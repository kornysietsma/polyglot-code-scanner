use crate::polyglot_data::PolyglotData;
use crate::{flare::FlareTreeNode, git::GitActivity};
use failure::Error;
use indicatif::{ProgressBar, ProgressStyle};
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, PathBuf};
use std::rc::Rc;
use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
};

/// a path-like owned structure, for efficient creation and tracking of relative paths
/// (originally I just used Rc<Path> but needed to split them into Components over and over)
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Clone)]
struct PathVec {
    components: Vec<OsString>,
}

impl PathVec {
    fn new() -> Self {
        PathVec {
            components: Vec::new(),
        }
    }
    fn to_path_buf(&self) -> PathBuf {
        self.components.iter().collect()
    }
    fn push<T>(&mut self, path: T)
    where
        T: Into<OsString>,
    {
        self.components.push(path.into())
    }
}

impl Serialize for PathVec {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_path_buf().to_string_lossy().as_ref())
    }
}

impl<P> From<P> for PathVec
where
    P: Into<PathBuf>,
{
    fn from(source: P) -> Self {
        let components: Vec<OsString> = source
            .into()
            .components()
            .map(|component| {
                if let Component::Normal(text) = component {
                    text.to_owned()
                } else {
                    panic!("Unsupported path component '{:?}'", component);
                }
            })
            .collect();
        PathVec { components }
    }
}

/// Every file change we've seen - only in source code, and only where actual lines of code changed
/// Stored two ways redundantly for speed of lookup:
/// * by timestamp, in a BTreeMap so it's easy to access ranges
/// * by filename, with a BTreeSet of timestamps so again we can get ranges out easily
struct FileChangeTimestamps {
    /// all files changed by timestamp - must actually have lines changed!
    timestamps: BTreeMap<u64, HashSet<Rc<PathVec>>>,
    file_changes: HashMap<Rc<PathVec>, BTreeSet<u64>>,
}

impl FileChangeTimestamps {
    pub fn new(root: &FlareTreeNode) -> Result<Self, Error> {
        let mut timestamps: BTreeMap<u64, HashSet<Rc<PathVec>>> = BTreeMap::new();
        let mut file_changes: HashMap<Rc<PathVec>, BTreeSet<u64>> = HashMap::new();
        FileChangeTimestamps::accumulate_files(
            &mut timestamps,
            &mut file_changes,
            root,
            Rc::from(PathVec::new()),
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
        timestamps: &mut BTreeMap<u64, HashSet<Rc<PathVec>>>,
        file_changes: &mut HashMap<Rc<PathVec>, BTreeSet<u64>>,
        node: &FlareTreeNode,
        path: Rc<PathVec>,
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
            let mut child_path = (*path).clone();
            child_path.push(child.name());
            FileChangeTimestamps::accumulate_files(
                timestamps,
                file_changes,
                child,
                Rc::new(child_path),
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
    name: Rc<PathVec>,
    activity_bursts: u64,
    coupled_files: HashMap<Rc<PathVec>, u64>,
}

impl Coupling {
    fn new(name: Rc<PathVec>) -> Self {
        Coupling {
            name,
            activity_bursts: 0,
            coupled_files: HashMap::new(),
        }
    }
    fn add_file(&mut self, file: Rc<PathVec>) {
        if file != self.name {
            let count = self.coupled_files.entry(file).or_insert(0);
            *count += 1;
        }
    }
    fn add_files<T>(&mut self, files: T)
    where
        T: IntoIterator<Item = Rc<PathVec>>,
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
    couplings: HashMap<Rc<PathVec>, Coupling>,
}

impl CouplingBucket {
    fn new(bucket_start: u64, bucket_size: u64) -> Self {
        CouplingBucket {
            bucket_start,
            bucket_size,
            couplings: HashMap::new(),
        }
    }

    fn add_files<T>(&mut self, from: Rc<PathVec>, to: T)
    where
        T: IntoIterator<Item = Rc<PathVec>>,
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
        let bar = ProgressBar::new(file_change_timestamps.file_changes.len() as u64);
        bar.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
                .expect("Invalid template in CouplingBuckets::new!")
                .progress_chars("##-"),
        );
        for (file, timestamps) in file_change_timestamps.file_changes.iter() {
            bar.inc(1);
            for burst in ActivityBurst::from_events(timestamps, config.min_activity_gap) {
                let window_start = burst.start - config.coupling_time_distance;
                let window_end = burst.end + config.coupling_time_distance;
                let bucket_number = bucketing_config.bucket_for(burst.start).unwrap();
                let mut unique_files: HashSet<Rc<PathVec>> = HashSet::new();
                for (_coupled_time, coupled_files) in file_change_timestamps
                    .timestamps
                    .range(window_start..window_end)
                {
                    unique_files.extend(
                        coupled_files
                            .iter()
                            .filter(|&dest_file| {
                                filter_file(
                                    config.min_distance,
                                    config.max_common_roots,
                                    file,
                                    dest_file,
                                )
                            })
                            .cloned(),
                    );
                }
                buckets[bucket_number].add_files(file.clone(), unique_files);
            }
        }
        bar.finish();
        info!("Gathering coupling stats - filtering buckets");

        for bucket in &mut buckets {
            bucket.filter_by(config.min_bursts, config.min_coupling_ratio);
        }
        CouplingBuckets { buckets }
    }

    fn all_files(&self) -> HashSet<Rc<PathVec>> {
        self.buckets
            .iter()
            .flat_map(|coupling_bucket| coupling_bucket.couplings.keys().cloned())
            .collect()
    }

    fn file_coupling_data(&self, file: Rc<PathVec>) -> SerializableCouplingData {
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
    pub coupled_files: Vec<(Rc<PathVec>, u64)>,
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

#[derive(Debug, Clone, Copy, Serialize)]
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
    /// distance between nodes must be at least this, where 1 is siblings, 2 cousins, etc
    min_distance: usize,
    /// nodes must have no more than this many roots in common
    /// eg if 0, they must have different top-level folders.
    /// This is combined with min_distance (and maybe I'll ditch one?)
    max_common_roots: Option<usize>,
}

impl CouplingConfig {
    pub fn new(
        bucket_days: u64,
        min_bursts: u64,
        min_coupling_ratio: f64,
        min_activity_gap: u64,
        coupling_time_distance: u64,
        min_distance: usize,
        max_common_roots: Option<usize>,
    ) -> Self {
        CouplingConfig {
            bucket_days,
            min_bursts,
            min_coupling_ratio,
            min_activity_gap,
            coupling_time_distance,
            min_distance,
            max_common_roots,
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

#[derive(Debug, Clone, Copy)]
pub struct BucketingConfig {
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

/// count roots in common.
/// NOTE: this only nicely handles paths like I am using here,
/// which never start with '/' and never have '.' or '..' in them!
fn common_roots(path1: &PathVec, path2: &PathVec) -> usize {
    let mut components1 = path1.components.iter();
    let mut components2 = path2.components.iter();
    let mut common = 0;
    while let (Some(comp1), Some(comp2)) = (components1.next(), components2.next()) {
        if comp1 == comp2 {
            common += 1;
        } else {
            break;
        }
    }
    common
}

/// relationship distance:
/// equal, distance 0
/// siblings, e.g. same parent, distance 1
/// cousins, e.g. same grandparent, distance 2
/// uncles/aunts/neices/nephews, e.g. same grandparent but different depths, still distance 2
/// No relation returns None
#[cfg(test)]
fn relationship_distance(path1: &PathVec, path2: &PathVec) -> Option<usize> {
    let common_root_count = common_roots(path1, path2);
    relationship_distance_with_common_precalculated(path1, path2, common_root_count)
}

/// as relationship_distance but avoids duplicate calculation of common_roots()
fn relationship_distance_with_common_precalculated(
    path1: &PathVec,
    path2: &PathVec,
    common_root_count: usize,
) -> Option<usize> {
    if common_root_count == 0 {
        return None;
    }
    let depth1 = path1.components.len();
    let depth2 = path2.components.len();
    if depth2 > depth1 {
        Some((depth2 - common_root_count) as usize)
    } else {
        Some((depth1 - common_root_count) as usize)
    }
}

fn filter_file(
    min_distance: usize,
    max_common_roots: Option<usize>,
    path1: &PathVec,
    path2: &PathVec,
) -> bool {
    let common_root_count = common_roots(path1, path2);
    // return false if file is filtered by either criterion
    if let Some(max_common_roots) = max_common_roots {
        if common_root_count > max_common_roots {
            return false;
        }
    }
    let distance = relationship_distance_with_common_precalculated(path1, path2, common_root_count);
    if let Some(distance) = distance {
        return distance >= min_distance;
    }
    true
}

fn file_changes_to_coupling_buckets(
    tree: &FlareTreeNode,
    config: CouplingConfig,
) -> Result<Option<(BucketingConfig, CouplingBuckets)>, Error> {
    info!("Gathering coupling stats - collecting timestamps");

    let timestamps = FileChangeTimestamps::new(tree)?;

    if timestamps.is_empty() {
        warn!("No timestamps found, no coupling data processed");
        return Ok(None);
    }

    info!(
        "Collected {} timestamps, touching {} files",
        timestamps.timestamps.len(),
        timestamps.file_changes.len()
    );

    info!("Gathering coupling stats - building buckets");

    let earliest = timestamps.earliest().unwrap();
    let latest = timestamps.latest().unwrap();

    let bucketing_config = BucketingConfig::new(config, *earliest, *latest);

    let filtered_buckets = CouplingBuckets::new(config, &timestamps, bucketing_config);
    Ok(Some((bucketing_config, filtered_buckets)))
}

pub fn gather_coupling(
    polyglot_data: &mut PolyglotData,
    config: CouplingConfig,
) -> Result<(), Error> {
    info!("Gathering coupling stats - accumulating timestamps");
    let bucket_info = file_changes_to_coupling_buckets(polyglot_data.tree(), config)?;

    let (bucketing_config, filtered_buckets) = match bucket_info {
        Some(result) => result,
        None => return Ok(()),
    };

    info!("Gathering coupling stats - applying buckets to JSON tree");

    for file in filtered_buckets.all_files() {
        // TODO: can we avoid converting to pathbuf?
        let file_buf: PathBuf = file.to_path_buf();
        if let Some(tree_node) = polyglot_data
            .tree_mut()
            .get_in_mut(&mut file_buf.components())
        {
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

    polyglot_data.add_metadata(
        "coupling",
        json!({"buckets": bucketing_config,
    "config": config}),
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

    fn simple_coupling_config() -> CouplingConfig {
        CouplingConfig {
            bucket_days: 20,
            min_bursts: 1,
            min_coupling_ratio: 0.001,
            min_activity_gap: 60 * 60,
            coupling_time_distance: 60 * 60,
            min_distance: 0,
            max_common_roots: None,
        }
    }

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
            users: BTreeSet::new(),
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
    fn can_make_pathvec_from_paths() {
        let path_vec: PathVec = PathVec::from("foo/bar/baz");
        assert_eq!(path_vec.to_path_buf(), PathBuf::from("foo/bar/baz"));
    }

    #[test]
    fn can_append_to_pathvecs() {
        let mut path_vec: PathVec = PathVec::from("foo/bar");
        path_vec.push("baz");
        assert_eq!(path_vec.to_path_buf(), PathBuf::from("foo/bar/baz"));
    }

    #[test]
    fn can_convert_tree_to_daily_stats() {
        let tree = build_test_tree();
        let stats = FileChangeTimestamps::new(&tree).unwrap();
        assert!(!stats.is_empty());

        let mut expected_timestamps: BTreeMap<u64, HashSet<Rc<PathVec>>> = BTreeMap::new();
        let root_file_1: Rc<PathVec> = Rc::from(PathVec::from("root_file_1.txt"));
        let child_file_1: Rc<PathVec> = Rc::from(PathVec::from("child1/child1_file_1.txt"));
        expected_timestamps.insert(DAY1, [root_file_1.clone()].iter().cloned().collect());
        expected_timestamps.insert(
            DAY21,
            [root_file_1.clone(), child_file_1.clone()]
                .iter()
                .cloned()
                .collect(),
        );
        expected_timestamps.insert(DAY22, [child_file_1.clone()].iter().cloned().collect());

        let mut expected_file_changes: HashMap<Rc<PathVec>, BTreeSet<u64>> = HashMap::new();
        expected_file_changes.insert(root_file_1, [DAY1, DAY21].iter().cloned().collect());
        expected_file_changes.insert(child_file_1, [DAY21, DAY22].iter().cloned().collect());

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
        let config = simple_coupling_config();
        let (bucket_count, first_bucket_start) = config.buckets_for(DAY1, DAY22);
        assert_eq!(bucket_count, 2);

        // first day is in first bucket
        assert!(DAY1 > first_bucket_start && DAY1 < (first_bucket_start + config.bucket_size()));
        // last day is the last second of the last bucket (should it be midnight of the next day?)
        assert_eq!(
            DAY22,
            first_bucket_start + bucket_count * config.bucket_size() - 1
        );
    }

    #[test]
    fn can_find_bucket_for_timestamp() {
        let coupling_config = simple_coupling_config();
        let config = BucketingConfig::new(coupling_config, DAY1, DAY29);
        assert_eq!(config.first_bucket_start, DAY29 - (40 * DAY_SIZE) + 1);
        assert_eq!(config.bucket_count, 2);
        assert_eq!(config.bucket_for(DAY1), Some(0));
        assert_eq!(config.bucket_for(DAY29), Some(1));
        assert_eq!(config.bucket_for(DAY29 + 1), None);
        assert_eq!(config.bucket_for(config.first_bucket_start - 1), None);
    }

    fn make_test_timestamps(data: Vec<(u64, Vec<&str>)>) -> FileChangeTimestamps {
        let timestamps: BTreeMap<u64, HashSet<Rc<PathVec>>> = data
            .iter()
            .map(|(day, namelist)| {
                let paths: HashSet<Rc<PathVec>> = namelist
                    .iter()
                    .map(|name| Rc::from(PathVec::from(name)))
                    .collect();
                (*day, paths)
            })
            .collect();
        let mut file_changes: HashMap<Rc<PathVec>, BTreeSet<u64>> = HashMap::new();
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

    fn rc_pb(name: &str) -> Rc<PathVec> {
        Rc::from(PathVec::from(name))
    }

    #[test]
    fn single_file_change_produces_trivial_coupling_data() {
        // simple scenario:
        //  'foo' changes with 'bar' only
        let timestamps = make_test_timestamps(vec![(DAY1, vec!["foo", "bar"])]);
        // config is effectively not filtering anything
        let config = simple_coupling_config();
        let bucketing_config = BucketingConfig::new(config, DAY1, DAY1);

        let coupling_buckets = CouplingBuckets::new(config, &timestamps, bucketing_config);

        assert_eq!(coupling_buckets.buckets.len(), 1);
        let first_bucket = coupling_buckets.buckets.get(0).unwrap();
        // this really repeats an earlier test - buckets are right-aligned on date range so DAY1 is last timestamp in bucket
        assert_eq!(first_bucket.bucket_start, DAY1 - (20 * DAY_SIZE) + 1);
        assert_eq!(first_bucket.bucket_size, 20 * DAY_SIZE);

        let mut expected_stats: HashMap<Rc<PathVec>, Coupling> = HashMap::new();
        let mut foo_coupling: HashMap<Rc<PathVec>, u64> = HashMap::new();
        foo_coupling.insert(rc_pb("foo"), 1);
        let mut bar_coupling: HashMap<Rc<PathVec>, u64> = HashMap::new();
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
        let config = simple_coupling_config();
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
        let foo_coupling: HashMap<Rc<PathVec>, u64> = [(rc_pb("bar"), 1), (rc_pb("baz"), 1)]
            .iter()
            .cloned()
            .collect();
        assert_eq!(foo_stats.coupled_files, foo_coupling);

        // second bucket, foo has two bursts, one coupled with baz, one with bat
        let foo_stats_b2 = second_bucket.couplings.get(&rc_pb("foo")).unwrap();
        assert_eq!(foo_stats_b2.activity_bursts, 2);
        let foo_coupling_b2: HashMap<Rc<PathVec>, u64> = [(rc_pb("baz"), 1), (rc_pb("bat"), 2)]
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
        let config = simple_coupling_config();
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
            min_distance: 0,
            max_common_roots: None,
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

    #[test]
    fn coupling_is_filtered_by_file_distance() {
        // test setup - filter out files with 1 burst per bucket,
        // and coupling below 50%
        let config = CouplingConfig {
            bucket_days: 20,
            min_bursts: 1,
            min_coupling_ratio: 0.01,
            min_activity_gap: 60 * 60,
            coupling_time_distance: 60 * 60,
            min_distance: 2,
            max_common_roots: Some(1),
        };
        // filtering here means:
        //  siblings are not included
        //  'foo/bar/*' won't match anything else under 'foo/bar' as that's two common roots.

        let timestamps = make_test_timestamps(vec![
            (DAY1, vec!["foo/bar.c", "foo/baz.c"]),         // siblings
            (DAY2, vec!["foo/bat/bar.c", "foo/baz/bar.c"]), // cousins
            (DAY3, vec!["foo/bar/baz/bat.c", "foo/bar/bat/bum.c"]), // two common roots
            (DAY4, vec!["foo/bum.c", "bar/foo.c"]),         // unrelated
        ]);
        let bucketing_config = BucketingConfig::new(config, DAY1, DAY29);

        let coupling_buckets = CouplingBuckets::new(config, &timestamps, bucketing_config);

        let day1_coupling = coupling_buckets.file_coupling_data(rc_pb("foo/bar.c"));
        assert_eq!(day1_coupling.buckets.len(), 1);
        assert_eq!(day1_coupling.buckets[0].coupled_files.len(), 0);
        let day2_coupling = coupling_buckets.file_coupling_data(rc_pb("foo/bat/bar.c"));
        assert_eq!(day2_coupling.buckets.len(), 1);
        let day2_coupling = &day2_coupling.buckets[0];
        assert_eq!(
            day2_coupling.coupled_files,
            vec![(rc_pb("foo/baz/bar.c"), 1)]
        );
        let day3_coupling = coupling_buckets.file_coupling_data(rc_pb("foo/bar/baz/bat.c"));
        assert_eq!(day3_coupling.buckets.len(), 1);
        assert_eq!(day3_coupling.buckets[0].coupled_files.len(), 0);

        let day4_coupling = coupling_buckets.file_coupling_data(rc_pb("foo/bum.c"));
        assert_eq!(day4_coupling.buckets.len(), 1);
        let day4_coupling = &day4_coupling.buckets[0];
        assert_eq!(day4_coupling.coupled_files, vec![(rc_pb("bar/foo.c"), 1)]);
    }

    #[test]
    fn common_roots_calculates_common_parts_of_paths() {
        assert_eq!(common_roots(&"foo".into(), &"bar".into()), 0);
        assert_eq!(common_roots(&"foo".into(), &"foo".into()), 1);
        assert_eq!(common_roots(&"foo".into(), &"foo/bar".into()), 1);
        assert_eq!(common_roots(&"foo/baz".into(), &"foo/bar".into()), 1);
        assert_eq!(common_roots(&"foo/baz".into(), &"foo/baz".into()), 2);
        assert_eq!(common_roots(&"foo/baz/a".into(), &"foo/baz/b".into()), 2);
    }

    #[test]
    fn unrelated_paths_have_no_relationship() {
        assert_eq!(relationship_distance(&"foo".into(), &"bar".into()), None);
        assert_eq!(
            relationship_distance(&"foo/baz".into(), &"bar/baz".into()),
            None
        );
    }
    #[test]
    fn can_count_relationship_distance_for_simple_cases() {
        assert_eq!(
            relationship_distance(&"foo/bar".into(), &"foo/bar".into()),
            Some(0)
        );
        assert_eq!(
            relationship_distance(&"foo/bar".into(), &"foo/baz".into()),
            Some(1)
        );
        assert_eq!(
            relationship_distance(&"foo/bar/baz".into(), &"foo/baz/bat".into()),
            Some(2)
        );
    }
    #[test]
    fn uncles_and_nieces_and_other_strange_relationships_work() {
        assert_eq!(
            relationship_distance(&"foo/bam".into(), &"foo/bar/baz".into()),
            Some(2)
        );
        assert_eq!(
            relationship_distance(&"foo/bar".into(), &"foo/bar/baz".into()),
            Some(1)
        );
        assert_eq!(
            relationship_distance(&"foo/bag".into(), &"foo/bar/bat/baz".into()),
            Some(3)
        );
    }
}
