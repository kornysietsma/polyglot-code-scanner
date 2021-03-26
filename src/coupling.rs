use crate::{flare::FlareTreeNode, git::GitActivity};
use failure::Error;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Every file change we've seen - only in source code, and only where actual lines of code changed
/// Stored two ways redundantly for speed of lookup:
/// * by timestamp, in a BTreeMap so it's easy to access ranges
/// * by filename, with a BTreeSet of timestamps so again we can get ranges out easily
struct FileChangeTimestamps {
    /// all files changed by timestamp - must actually have lines changed!
    timestamps: BTreeMap<u64, HashSet<Rc<Path>>>,
    file_changes: HashMap<Rc<Path>, BTreeSet<u64>>,
}

impl FileChangeTimestamps {
    pub fn new(root: &FlareTreeNode) -> Result<Self, Error> {
        let mut timestamps: BTreeMap<u64, HashSet<Rc<Path>>> = BTreeMap::new();
        let mut file_changes: HashMap<Rc<Path>, BTreeSet<u64>> = HashMap::new();
        FileChangeTimestamps::accumulate_files(
            &mut timestamps,
            &mut file_changes,
            root,
            Rc::from(PathBuf::new()),
        )?;
        Ok(FileChangeTimestamps {
            timestamps,
            file_changes,
        })
    }

    fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }
    fn earliest(&self) -> Option<&u64> {
        self.timestamps.range(..).next().map(|x| x.0)
    }
    fn latest(&self) -> Option<&u64> {
        self.timestamps.range(..).next_back().map(|x| x.0)
    }

    fn accumulate_files(
        timestamps: &mut BTreeMap<u64, HashSet<Rc<Path>>>,
        file_changes: &mut HashMap<Rc<Path>, BTreeSet<u64>>,
        node: &FlareTreeNode,
        path: Rc<Path>,
    ) -> Result<(), Error> {
        let lines = node
            .get_data("loc")
            .and_then(|loc| loc.get("code"))
            .and_then(|x| x.as_u64())
            .unwrap_or(0);
        if lines > 0 {
            if let Some(Value::Object(value)) = node.get_data("git") {
                if let Some(Value::Array(activity)) = value.get("activity") {
                    for activity_value in activity {
                        let activity: GitActivity = serde_json::from_value(activity_value.clone())?;
                        if activity.lines_deleted > 0 || activity.lines_added > 0 {
                            let ts_entry = timestamps
                                .entry(activity.commit_time)
                                .or_insert_with(HashSet::new);
                            (*ts_entry).insert(path.clone());
                            let fs_entry = file_changes
                                .entry(path.clone())
                                .or_insert_with(BTreeSet::new);
                            (*fs_entry).insert(activity.commit_time);
                        }
                    }
                }
            }
        };

        for child in node.get_children() {
            let mut child_path = path.to_path_buf();
            child_path.push(child.name());
            FileChangeTimestamps::accumulate_files(
                timestamps,
                file_changes,
                &child,
                child_path.into(),
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ActivityBurst {
    // the start of the burst
    pub start: u64,
    // the end of the burst - inclusive!
    pub end: u64,
    // how many events we met in this burst - 1 or more
    pub event_count: u64,
}

impl ActivityBurst {
    fn from_events(events: &BTreeSet<u64>, min_activity_gap: u64) -> Vec<ActivityBurst> {
        let mut results: Vec<ActivityBurst> = Vec::new();
        let mut first_event: Option<u64> = None;
        let mut last_event: Option<u64> = None;
        let mut event_count: u64 = 0;
        for event in events {
            match last_event {
                None => {
                    first_event = Some(*event);
                    last_event = Some(*event);
                    event_count = 1;
                }
                Some(last) => {
                    if (event - last) <= min_activity_gap {
                        last_event = Some(*event);
                        event_count += 1;
                    } else {
                        results.push(ActivityBurst {
                            start: first_event.unwrap(),
                            end: last,
                            event_count,
                        });
                        first_event = Some(*event);
                        last_event = Some(*event);
                        event_count = 1;
                    }
                }
            }
        }
        if let Some(last_event) = last_event {
            results.push(ActivityBurst {
                start: first_event.unwrap(),
                end: last_event,
                event_count,
            });
        }
        results
    }
}

/// The basic coupling data - for each file in some time period, how often did it change and how often did
/// another file change at roughly the same time
#[derive(Debug, Clone, PartialEq, Eq)]
struct Coupling {
    name: Rc<Path>,
    activity_bursts: u64,
    coupled_files: HashMap<Rc<Path>, u64>,
}

impl Coupling {
    fn new(name: Rc<Path>) -> Self {
        Coupling {
            name,
            activity_bursts: 0,
            coupled_files: HashMap::new(),
        }
    }
    fn add_file(&mut self, file: Rc<Path>) {
        if file != self.name {
            let count = self.coupled_files.entry(file).or_insert(0);
            *count += 1;
        }
    }
    fn add_files<T>(&mut self, files: T)
    where
        T: IntoIterator<Item = Rc<Path>>,
    {
        for file in files {
            self.add_file(file)
        }
        self.activity_bursts += 1;
    }
    fn filter_by_ratio(&self, min_coupling_ratio: f64) -> Coupling {
        let bursts = self.activity_bursts as f64;
        Coupling {
            name: self.name.clone(),
            activity_bursts: self.activity_bursts,
            coupled_files: self
                .coupled_files
                .iter()
                .filter(|(_file, other_bursts)| {
                    **other_bursts as f64 / bursts >= min_coupling_ratio
                })
                .map(|(file, other_bursts)| (file.clone(), *other_bursts))
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
struct CouplingBucket {
    bucket_start: u64,
    bucket_size: u64,
    couplings: HashMap<Rc<Path>, Coupling>,
}

impl CouplingBucket {
    fn new(bucket_start: u64, bucket_size: u64) -> Self {
        CouplingBucket {
            bucket_start,
            bucket_size,
            couplings: HashMap::new(),
        }
    }

    fn add_files<T>(&mut self, from: Rc<Path>, to: T)
    where
        T: IntoIterator<Item = Rc<Path>>,
    {
        let stats = self
            .couplings
            .entry(from.clone())
            .or_insert_with(|| Coupling::new(from));
        (*stats).add_files(to);
    }

    /// filter the bucket to remove noise
    /// min_source_days is the minimum number of days a file should have existed for it to be included
    /// min_coupling_ratio is the overall ratio of dest days / source days for the destination to be included.
    fn filter_by(&mut self, min_bursts: u64, min_coupling_ratio: f64) {
        self.couplings = self
            .couplings
            .drain()
            .filter(|(_file, file_stats)| file_stats.activity_bursts >= min_bursts)
            .map(|(file, file_stats)| (file, file_stats.filter_by_ratio(min_coupling_ratio)))
            .collect();
    }
}

struct CouplingBuckets {
    buckets: Vec<CouplingBucket>,
}

impl CouplingBuckets {
    fn new(
        config: CouplingConfig,
        file_change_timestamps: &FileChangeTimestamps,
        bucketing_config: BucketingConfig,
    ) -> Self {
        let bucket_size = bucketing_config.bucket_size;
        let mut buckets: Vec<CouplingBucket> = (0..bucketing_config.bucket_count)
            .map(|bucket| {
                let bucket_start: u64 = bucketing_config.bucket_start(bucket);
                CouplingBucket::new(bucket_start, bucket_size)
            })
            .collect();
        for (file, timestamps) in file_change_timestamps.file_changes.iter() {
            for burst in ActivityBurst::from_events(timestamps, config.min_activity_gap) {
                let window_start = burst.start - config.coupling_time_distance;
                let window_end = burst.end + config.coupling_time_distance;
                let bucket_number = bucketing_config.bucket_for(burst.start).unwrap();
                let mut unique_files: HashSet<Rc<Path>> = HashSet::new();
                for (_coupled_time, coupled_files) in file_change_timestamps
                    .timestamps
                    .range(window_start..window_end)
                {
                    unique_files.extend(coupled_files.iter().cloned());
                }
                buckets[bucket_number].add_files(file.clone(), unique_files);
            }
        }
        for bucket in &mut buckets {
            bucket.filter_by(config.min_bursts, config.min_coupling_ratio);
        }
        CouplingBuckets { buckets }
    }

    fn all_files(&self) -> HashSet<Rc<Path>> {
        self.buckets
            .iter()
            .flat_map(|coupling_bucket| coupling_bucket.couplings.keys().cloned())
            .collect()
    }

    fn file_coupling_data(&self, file: Rc<Path>) -> SerializableCouplingData {
        SerializableCouplingData::new(
            self.buckets
                .iter()
                .filter(|coupling_bucket| coupling_bucket.couplings.contains_key(&file.clone()))
                .map(|coupling_bucket| {
                    let stats = coupling_bucket.couplings.get(&file.to_owned()).unwrap();
                    let activity_bursts = stats.activity_bursts;
                    let mut coupled_files: Vec<_> = stats
                        .coupled_files
                        .iter()
                        .map(|(file, count)| (file.clone(), *count))
                        .collect();
                    coupled_files.sort_by(|(path1, _count1), (path2, _count2)| {
                        path1.partial_cmp(path2).unwrap()
                    });
                    SerializableCouplingBucketData {
                        bucket_start: coupling_bucket.bucket_start,
                        bucket_end: coupling_bucket.bucket_start + coupling_bucket.bucket_size - 1,
                        activity_bursts,
                        coupled_files,
                    }
                })
                .collect(),
        )
    }
}

/// Individual bucket to save in the Json tree
#[derive(Debug, PartialEq, Serialize)]
struct SerializableCouplingBucketData {
    pub bucket_start: u64,
    pub bucket_end: u64,
    pub activity_bursts: u64,
    pub coupled_files: Vec<(Rc<Path>, u64)>,
}

/// Data to save in the Json tree for a file
#[derive(Debug, PartialEq, Serialize)]
struct SerializableCouplingData {
    pub buckets: Vec<SerializableCouplingBucketData>,
}

impl SerializableCouplingData {
    fn new(buckets: Vec<SerializableCouplingBucketData>) -> Self {
        SerializableCouplingData { buckets }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CouplingConfig {
    // number of days in a bucket
    bucket_days: u64,
    // ignore if a "from" file isn't changed this often in a bucket - avoid coincidental change noise
    min_bursts: u64,
    // ignore if commits(to) / commits(from) is less than this - so if A is committed 100 days in a bucket, and B is on 20 of the same days, it would pass with a 0.2 ratio or higher
    min_coupling_ratio: f64,
    /// how many seconds gap before we start a new activity
    min_activity_gap: u64,
    /// how many seconds before or after an activity count for coupling?
    coupling_time_distance: u64,
}

impl CouplingConfig {
    pub fn new(
        bucket_days: u64,
        min_bursts: u64,
        min_coupling_ratio: f64,
        min_activity_gap: u64,
        coupling_time_distance: u64,
    ) -> Self {
        CouplingConfig {
            bucket_days,
            min_bursts,
            min_coupling_ratio,
            min_activity_gap,
            coupling_time_distance,
        }
    }
    pub fn bucket_size(&self) -> u64 {
        self.bucket_days * 24 * 60 * 60
    }
    pub fn buckets_for(&self, earliest: u64, latest: u64) -> (u64, u64) {
        // want buckets that end with the last day of the last bucket the latest day
        let bucket_size = self.bucket_size();
        let bucket_count = ((latest - earliest) / bucket_size) + 1;
        let first_bucket_start = (latest - (bucket_size * bucket_count)) + 1;
        (bucket_count, first_bucket_start)
    }
}
impl Default for CouplingConfig {
    fn default() -> Self {
        CouplingConfig {
            bucket_days: 91, // roughly 1/4 of a year
            min_bursts: 10,  // 10 bursts of activity in a quarter or be considered inactive
            min_coupling_ratio: 0.25,
            min_activity_gap: 60 * 60 * 2,       // 2 hours
            coupling_time_distance: 60 * 60 * 1, // 1 hour
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BucketingConfig {
    earliest: u64,
    latest: u64,
    bucket_size: u64,
    bucket_count: u64,
    first_bucket_start: u64,
}

impl BucketingConfig {
    fn new(coupling_config: CouplingConfig, earliest: u64, latest: u64) -> Self {
        let bucket_size = coupling_config.bucket_size();
        let bucket_count = ((latest - earliest) / bucket_size) + 1;
        let first_bucket_start = (latest - (bucket_size * bucket_count)) + 1;
        BucketingConfig {
            earliest,
            latest,
            bucket_size,
            bucket_count,
            first_bucket_start,
        }
    }
    fn bucket_start(&self, bucket: u64) -> u64 {
        self.first_bucket_start + bucket * self.bucket_size
    }
    fn bucket_for(&self, timestamp: u64) -> Option<usize> {
        if timestamp < self.first_bucket_start {
            return None;
        }
        let relative_ts = timestamp - self.first_bucket_start;
        if relative_ts >= self.bucket_count * self.bucket_size {
            return None;
        }
        Some((relative_ts / self.bucket_size) as usize)
    }
}

impl Serialize for BucketingConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("CouplingBuckets", 3)?;
        state.serialize_field("bucket_size", &self.bucket_size)?;
        state.serialize_field("bucket_count", &self.bucket_count)?;
        state.serialize_field("first_bucket_start", &self.first_bucket_start)?;
        state.end()
    }
}

fn file_changes_to_coupling_buckets(
    tree: &FlareTreeNode,
    config: CouplingConfig,
) -> Result<Option<(BucketingConfig, CouplingBuckets)>, Error> {
    let timestamps = FileChangeTimestamps::new(&tree)?;

    if timestamps.is_empty() {
        warn!("No timestamps found, no coupling data processed");
        return Ok(None);
    }

    info!("Gathering coupling stats - building buckets");

    let earliest = timestamps.earliest().unwrap();
    let latest = timestamps.latest().unwrap();

    let bucketing_config = BucketingConfig::new(config, *earliest, *latest);

    let filtered_buckets = CouplingBuckets::new(config, &timestamps, bucketing_config);
    Ok(Some((bucketing_config, filtered_buckets)))
}

pub fn gather_coupling(tree: &mut FlareTreeNode, config: CouplingConfig) -> Result<(), Error> {
    info!("Gathering coupling stats - accumulating daily counts");
    let bucket_info = file_changes_to_coupling_buckets(tree, config)?;

    let (bucketing_config, filtered_buckets) = match bucket_info {
        Some(result) => result,
        None => return Ok(()),
    };

    info!("Gathering coupling stats - applying buckets to JSON tree");

    for file in filtered_buckets.all_files() {
        if let Some(tree_node) = tree.get_in_mut(&mut file.components().clone()) {
            let coupling_data = filtered_buckets.file_coupling_data(file);
            tree_node.add_data(
                "coupling",
                serde_json::value::to_value(coupling_data)
                    .expect("Serializable object couldn't be serialized to JSON"),
            );
        } else {
            // TODO: return an error
            error!("Can't find {:?} in tree!", &file);
        };
    }

    tree.add_data(
        "coupling_meta",
        serde_json::value::to_value(bucketing_config).expect("Can't serialize bucketing config"),
    );
    info!("Gathering coupling stats - done");
    Ok(())
}

#[cfg(test)]
mod test {
    use crate::git_logger::CommitChange;

    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::path::Path;
    #[allow(unused_imports)] // boilerplate for getting shared test logic
    use test_shared::*;

    // TODO: better constants - days aren't really what we want any more
    const DAY_SIZE: u64 = 24 * 60 * 60;
    const TEST_START: u64 = (2020 - 1970) * 365 * DAY_SIZE;
    const DAY1: u64 = TEST_START + DAY_SIZE;
    const DAY2: u64 = TEST_START + 2 * DAY_SIZE;
    const DAY3: u64 = TEST_START + 3 * DAY_SIZE;
    const DAY4: u64 = TEST_START + 4 * DAY_SIZE;
    const DAY21: u64 = TEST_START + 21 * DAY_SIZE;
    const DAY22: u64 = TEST_START + 22 * DAY_SIZE;
    const DAY23: u64 = TEST_START + 23 * DAY_SIZE;
    const DAY29: u64 = TEST_START + 29 * DAY_SIZE;

    #[derive(Debug, PartialEq, Serialize)]
    struct FakeGitDetails {
        commit_day: u64,
    }

    #[derive(Debug, PartialEq, Serialize)]
    struct FakeGitData {
        activity: Option<Vec<GitActivity>>,
    }

    fn fake_git_activity(timestamp: u64) -> GitActivity {
        GitActivity {
            author_time: timestamp,
            commit_time: timestamp,
            users: HashSet::new(),
            change: CommitChange::Add,
            lines_added: 1,
            lines_deleted: 0,
        }
    }

    impl FakeGitData {
        fn new(timestamps: &[u64]) -> Self {
            FakeGitData {
                activity: Some(timestamps.iter().map(|t| fake_git_activity(*t)).collect()),
            }
        }
        fn to_json(&self) -> Value {
            serde_json::value::to_value(self).unwrap()
        }
    }

    fn build_test_tree() -> FlareTreeNode {
        let mut root = FlareTreeNode::dir("root");
        let mut root_file_1 = FlareTreeNode::file("root_file_1.txt");
        root_file_1.add_data("git", FakeGitData::new(&[DAY1, DAY21]).to_json());
        root_file_1.add_data("loc", json!({"code": 12}));
        root.append_child(root_file_1);
        root.append_child(FlareTreeNode::file("root_file_2.txt"));
        let mut child1 = FlareTreeNode::dir("child1");
        let mut child1_file1 = FlareTreeNode::file("child1_file_1.txt");
        child1_file1.add_data("git", FakeGitData::new(&[DAY21, DAY22]).to_json());
        child1_file1.add_data("loc", json!({"code": 122}));
        child1.append_child(child1_file1);
        let mut child1_file2 = FlareTreeNode::file("binary_file.zip");
        child1_file2.add_data("git", FakeGitData::new(&[DAY21, DAY22, DAY23]).to_json());
        child1.append_child(child1_file2);
        root.append_child(child1);
        root
    }
    #[test]
    fn can_convert_tree_to_daily_stats() {
        let tree = build_test_tree();
        let stats = FileChangeTimestamps::new(&tree).unwrap();
        assert_eq!(stats.is_empty(), false);

        let mut expected_timestamps: BTreeMap<u64, HashSet<Rc<Path>>> = BTreeMap::new();
        let root_file_1: Rc<Path> = Rc::from(Path::new("root_file_1.txt").to_owned());
        let child_file_1: Rc<Path> = Rc::from(Path::new("child1/child1_file_1.txt").to_owned());
        expected_timestamps.insert(DAY1, [root_file_1.clone()].iter().cloned().collect());
        expected_timestamps.insert(
            DAY21,
            [root_file_1.clone(), child_file_1.clone()]
                .iter()
                .cloned()
                .collect(),
        );
        expected_timestamps.insert(DAY22, [child_file_1.clone()].iter().cloned().collect());

        let mut expected_file_changes: HashMap<Rc<Path>, BTreeSet<u64>> = HashMap::new();
        expected_file_changes.insert(root_file_1.clone(), [DAY1, DAY21].iter().cloned().collect());
        expected_file_changes.insert(
            child_file_1.clone(),
            [DAY21, DAY22].iter().cloned().collect(),
        );

        assert_eq!(expected_timestamps, stats.timestamps);
        assert_eq!(expected_file_changes, stats.file_changes);
    }

    #[test]
    fn can_get_daily_stats_early_late() {
        let tree = build_test_tree();
        let stats = FileChangeTimestamps::new(&tree).unwrap();
        assert_eq!(stats.earliest().unwrap(), &DAY1);
        assert_eq!(stats.latest().unwrap(), &DAY22);
    }

    #[test]
    fn single_event_creates_a_single_activity_burst() {
        let events = [DAY1].iter().cloned().collect();
        let results = ActivityBurst::from_events(&events, 60);
        assert_eq!(results.len(), 1);
        let res1 = results.first().unwrap();
        assert_eq!(res1.start, DAY1);
        assert_eq!(res1.end, DAY1);
        assert_eq!(res1.event_count, 1);
    }
    #[test]
    fn empty_events_creates_no_activity_burst() {
        let events = BTreeSet::new();
        let results = ActivityBurst::from_events(&events, 60);
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn events_with_no_gap_return_a_single_activity_burst() {
        let events = [DAY1, DAY1 + 10, DAY1 + 20].iter().cloned().collect();
        let results = ActivityBurst::from_events(&events, 60);
        assert_eq!(results.len(), 1);
        let res1 = results.first().unwrap();
        assert_eq!(res1.start, DAY1);
        assert_eq!(res1.end, DAY1 + 20);
        assert_eq!(res1.event_count, 3);
    }

    #[test]
    fn events_with_gaps_return_multiple_activity_bursts() {
        let events: BTreeSet<u64> = [
            DAY1,
            DAY1 + 10,
            DAY1 + 20,
            DAY1 + 80,
            DAY1 + 90,
            DAY2,
            DAY3,
            DAY3 + 20,
        ]
        .iter()
        .cloned()
        .collect();

        let results = ActivityBurst::from_events(&events, 59);
        assert_eq!(
            results,
            vec![
                ActivityBurst {
                    start: DAY1,
                    end: DAY1 + 20,
                    event_count: 3
                },
                ActivityBurst {
                    start: DAY1 + 80,
                    end: DAY1 + 90,
                    event_count: 2
                },
                ActivityBurst {
                    start: DAY2,
                    end: DAY2,
                    event_count: 1
                },
                ActivityBurst {
                    start: DAY3,
                    end: DAY3 + 20,
                    event_count: 2
                },
            ]
        );
    }

    #[test]
    fn can_get_bucket_sizes_from_config() {
        let config = CouplingConfig {
            bucket_days: 20,
            min_bursts: 1,
            min_coupling_ratio: 0.001,
            min_activity_gap: 60,
            coupling_time_distance: 100,
        };
        let (bucket_count, first_bucket_start) = config.buckets_for(DAY1, DAY22);
        assert_eq!(bucket_count, 2);

        // first day is in first bucket
        assert_eq!(
            DAY1 > first_bucket_start && DAY1 < (first_bucket_start + config.bucket_size()),
            true
        );
        // last day is the last second of the last bucket (should it be midnight of the next day?)
        assert_eq!(
            DAY22,
            first_bucket_start + bucket_count * config.bucket_size() - 1
        );
    }

    #[test]
    fn can_find_bucket_for_timestamp() {
        let coupling_config = CouplingConfig {
            bucket_days: 20,
            min_bursts: 1,
            min_coupling_ratio: 0.001,
            min_activity_gap: 60,
            coupling_time_distance: 100,
        };
        let config = BucketingConfig::new(coupling_config, DAY1, DAY29);
        assert_eq!(config.first_bucket_start, DAY29 - (40 * DAY_SIZE) + 1);
        assert_eq!(config.bucket_count, 2);
        assert_eq!(config.bucket_for(DAY1), Some(0));
        assert_eq!(config.bucket_for(DAY29), Some(1));
        assert_eq!(config.bucket_for(DAY29 + 1), None);
        assert_eq!(config.bucket_for(config.first_bucket_start - 1), None);
    }

    fn make_test_timestamps(data: Vec<(u64, Vec<&str>)>) -> FileChangeTimestamps {
        let timestamps: BTreeMap<u64, HashSet<Rc<Path>>> = data
            .iter()
            .map(|(day, namelist)| {
                let paths: HashSet<Rc<Path>> = namelist
                    .iter()
                    .map(|name| Rc::from(Path::new(name).to_path_buf()))
                    .collect();
                (*day, paths)
            })
            .collect();
        let mut file_changes: HashMap<Rc<Path>, BTreeSet<u64>> = HashMap::new();
        for (timestamp, files) in timestamps.clone() {
            for file in files {
                let fs_entry = file_changes
                    .entry(file.clone())
                    .or_insert_with(BTreeSet::new);
                (*fs_entry).insert(timestamp);
            }
        }
        FileChangeTimestamps {
            timestamps,
            file_changes,
        }
    }

    fn rc_pb(name: &str) -> Rc<Path> {
        Rc::from(Path::new(name).to_path_buf())
    }

    #[test]
    fn single_file_change_produces_trivial_coupling_data() {
        // simple scenario:
        //  'foo' changes with 'bar' only
        let timestamps = make_test_timestamps(vec![(DAY1, vec!["foo", "bar"])]);
        // config is effectively not filtering anything
        let config = CouplingConfig {
            bucket_days: 20,
            min_bursts: 1,
            min_coupling_ratio: 0.001,
            min_activity_gap: 60 * 60,
            coupling_time_distance: 60 * 60,
        };
        let bucketing_config = BucketingConfig::new(config, DAY1, DAY1);

        let coupling_buckets = CouplingBuckets::new(config, &timestamps, bucketing_config);

        assert_eq!(coupling_buckets.buckets.len(), 1);
        let first_bucket = coupling_buckets.buckets.get(0).unwrap();
        // this really repeats an earlier test - buckets are right-aligned on date range so DAY1 is last timestamp in bucket
        assert_eq!(first_bucket.bucket_start, DAY1 - (20 * DAY_SIZE) + 1);
        assert_eq!(first_bucket.bucket_size, 20 * DAY_SIZE);

        let mut expected_stats: HashMap<Rc<Path>, Coupling> = HashMap::new();
        let mut foo_coupling: HashMap<Rc<Path>, u64> = HashMap::new();
        foo_coupling.insert(rc_pb("foo"), 1);
        let mut bar_coupling: HashMap<Rc<Path>, u64> = HashMap::new();
        bar_coupling.insert(rc_pb("bar"), 1);
        expected_stats.insert(
            rc_pb("foo"),
            Coupling {
                name: rc_pb("foo"),
                activity_bursts: 1,
                coupled_files: bar_coupling,
            },
        );
        expected_stats.insert(
            rc_pb("bar"),
            Coupling {
                name: rc_pb("bar"),
                activity_bursts: 1,
                coupled_files: foo_coupling,
            },
        );

        assert_eq!(first_bucket.couplings, expected_stats);
    }

    #[test]
    fn can_build_coupling_data_from_timestamps() {
        // a more real scenario, with a few more detailed coupling stats
        let timestamps = make_test_timestamps(vec![
            (DAY1, vec!["foo", "bar"]),
            (DAY1 + 60, vec!["foo"]),
            (DAY1 + 90, vec!["baz"]),
            (DAY21, vec!["foo", "baz", "bat"]),
            (DAY22, vec!["foo"]),
            (DAY22 + 200, vec!["foo"]),
            (DAY22 + 400, vec!["foo"]),
            (DAY22 + 500, vec!["bat"]),
        ]);
        // config is effectively not filtering anything
        let config = CouplingConfig {
            bucket_days: 20,
            min_bursts: 1,
            min_coupling_ratio: 0.001,
            min_activity_gap: 60 * 60,
            coupling_time_distance: 60 * 60,
        };
        let bucketing_config = BucketingConfig::new(config, DAY1, DAY22 + 500);

        let coupling_buckets = CouplingBuckets::new(config, &timestamps, bucketing_config);

        // there should be 2 buckets (as each one is 20 days long)
        assert_eq!(coupling_buckets.buckets.len(), 2);
        let first_bucket = coupling_buckets.buckets.get(0).unwrap();
        let second_bucket = coupling_buckets.buckets.get(1).unwrap();

        // first bucket should have file_stats for foo, bar and baz
        //  easier to dig out specific test cases than build the whole structure for equality testing
        let foo_stats = first_bucket.couplings.get(&rc_pb("foo")).unwrap();
        assert_eq!(foo_stats.name, rc_pb("foo")); // redundant!
        assert_eq!(foo_stats.activity_bursts, 1); // actually activity bursts not commits - and there is only one
        let foo_coupling: HashMap<Rc<Path>, u64> = [(rc_pb("bar"), 1), (rc_pb("baz"), 1)]
            .iter()
            .cloned()
            .collect();
        assert_eq!(foo_stats.coupled_files, foo_coupling);

        // second bucket, foo has two bursts, one coupled with baz, one with bat
        let foo_stats_b2 = second_bucket.couplings.get(&rc_pb("foo")).unwrap();
        assert_eq!(foo_stats_b2.activity_bursts, 2);
        let foo_coupling_b2: HashMap<Rc<Path>, u64> = [(rc_pb("baz"), 1), (rc_pb("bat"), 2)]
            .iter()
            .cloned()
            .collect();
        assert_eq!(foo_stats_b2.coupled_files, foo_coupling_b2);
    }

    #[test]
    fn can_build_serializable_coupling_data_from_timestamps() {
        // same scenario as above, but testing the ability to turn it into serializable data
        let timestamps = make_test_timestamps(vec![
            (DAY1, vec!["foo", "bar"]),
            (DAY1 + 60, vec!["foo"]),
            (DAY1 + 90, vec!["baz"]),
            (DAY21, vec!["foo", "baz", "bat"]),
            (DAY22, vec!["foo"]),
            (DAY22 + 200, vec!["foo"]),
            (DAY22 + 400, vec!["foo"]),
            (DAY22 + 500, vec!["bat"]),
        ]);
        // config is effectively not filtering anything
        let config = CouplingConfig {
            bucket_days: 20,
            min_bursts: 1,
            min_coupling_ratio: 0.001,
            min_activity_gap: 60 * 60,
            coupling_time_distance: 60 * 60,
        };
        let bucketing_config = BucketingConfig::new(config, DAY1, DAY22 + 500);

        let coupling_buckets = CouplingBuckets::new(config, &timestamps, bucketing_config);

        let foo_coupling = coupling_buckets.file_coupling_data(rc_pb("foo"));

        // cheat - as it's serializable, we can test by comparing JSON instead of constructing objects
        let foo_json = serde_json::value::to_value(foo_coupling).expect("Can't serialize!");
        let foo_expected = json!({
          "buckets": [
          {
            "bucket_start": bucketing_config.bucket_start(0),
            "bucket_end": bucketing_config.bucket_start(0) + bucketing_config.bucket_size - 1,
            "activity_bursts": 1,
            "coupled_files": [["bar", 1],["baz",1]]
          },
          {
            "bucket_start": bucketing_config.bucket_start(1),
            "bucket_end": bucketing_config.bucket_start(1) + bucketing_config.bucket_size - 1,
            "activity_bursts": 2,
            "coupled_files": [["bat", 2],["baz",1]]
          }

          ]
        });
        assert_eq!(foo_json, foo_expected);
    }

    #[test]
    fn coupling_is_filtered_and_calculated_as_ratio_of_commits_to_others() {
        // test setup - filter out files with 1 burst per bucket,
        // and coupling below 50%
        let config = CouplingConfig {
            bucket_days: 20,
            min_bursts: 2,
            min_coupling_ratio: 0.5,
            min_activity_gap: 60 * 60,
            coupling_time_distance: 60 * 60,
        };
        // test times should check these:
        // foo -> bar is in as it's 100%
        // foo -> baz is in just as it's 50%
        // foo -> bat is out as it's 25%
        // bat -> foo is out as there is only one bat
        // baz -> foo is in as there are two baz's
        let timestamps = make_test_timestamps(vec![
            (DAY1, vec!["foo", "bar"]),
            (DAY2, vec!["foo", "bar"]),
            (DAY3, vec!["foo", "bar", "baz"]),
            (DAY4, vec!["foo", "bar", "baz", "bat"]),
        ]);
        let bucketing_config = BucketingConfig::new(config, DAY1, DAY29);

        let coupling_buckets = CouplingBuckets::new(config, &timestamps, bucketing_config);

        let foo_coupling = coupling_buckets.file_coupling_data(rc_pb("foo"));
        assert_eq!(foo_coupling.buckets.len(), 1);
        let foo_coupling = &foo_coupling.buckets[0];
        assert_eq!(foo_coupling.activity_bursts, 4);
        assert_eq!(
            foo_coupling.coupled_files,
            vec![(rc_pb("bar"), 4), (rc_pb("baz"), 2)]
        );

        let bat_coupling = coupling_buckets.file_coupling_data(rc_pb("bat"));
        assert_eq!(bat_coupling.buckets.len(), 0);

        let baz_coupling = coupling_buckets.file_coupling_data(rc_pb("baz"));
        assert_eq!(baz_coupling.buckets.len(), 1);
        let baz_coupling = &baz_coupling.buckets[0];
        assert_eq!(baz_coupling.activity_bursts, 2);
        assert_eq!(
            baz_coupling.coupled_files,
            vec![(rc_pb("bar"), 2), (rc_pb("bat"), 1), (rc_pb("foo"), 2)]
        );
    }
}
