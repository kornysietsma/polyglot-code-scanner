use crate::flare::FlareTreeNode;
use failure::Error;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use serde_json::Value;
use std::collections::hash_map::Iter;
use std::collections::{HashMap, HashSet};
use std::path::{Components, Path, PathBuf};
use std::rc::Rc;

struct DailyStats {
    /// all files changed for a given day (in secs since epoch)
    stats: HashMap<u64, Vec<Rc<PathBuf>>>,
}

impl DailyStats {
    pub fn new(root: &FlareTreeNode) -> Result<Self, Error> {
        let mut stats: HashMap<u64, Vec<Rc<PathBuf>>> = HashMap::new();
        DailyStats::accumulate_stats(&mut stats, root, Rc::new(PathBuf::new()))?;
        Ok(DailyStats { stats })
    }
    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }
    pub fn earliest(&self) -> u64 {
        *self.stats.keys().min().unwrap()
    }
    pub fn latest(&self) -> u64 {
        *self.stats.keys().max().unwrap()
    }
    pub fn iter(&self) -> Iter<'_, u64, Vec<Rc<PathBuf>>> {
        self.stats.iter()
    }
    pub fn get(&self, day: u64) -> Option<&Vec<Rc<PathBuf>>> {
        self.stats.get(&day)
    }

    fn accumulate_stats(
        stats: &mut HashMap<u64, Vec<Rc<PathBuf>>>,
        node: &FlareTreeNode,
        path: Rc<PathBuf>,
    ) -> Result<(), Error> {
        let lines = node
            .get_data("loc")
            .and_then(|loc| loc.get("code"))
            .and_then(|x| x.as_u64())
            .unwrap_or(0);
        if lines > 0 {
            if let Some(Value::Object(value)) = node.get_data("git") {
                if let Some(Value::Array(details)) = value.get("details") {
                    for detail_value in details {
                        if let Some(commit_day) =
                            detail_value.pointer("/commit_day").and_then(|x| x.as_u64())
                        {
                            let daily_stat = stats.entry(commit_day).or_insert_with(Vec::new);
                            (*daily_stat).push(path.clone());
                        }
                    }
                }
            }
        };

        for child in node.get_children() {
            let mut child_path = Rc::clone(&path);
            (*Rc::make_mut(&mut child_path)).push(child.name());
            DailyStats::accumulate_stats(stats, &child, child_path)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct FileStats {
    name: Rc<PathBuf>,
    commits: u64,
    coupled_files: HashMap<Rc<PathBuf>, u64>,
}

impl FileStats {
    fn new(name: Rc<PathBuf>) -> Self {
        FileStats {
            name,
            commits: 0,
            coupled_files: HashMap::new(),
        }
    }
    fn add_file(&mut self, file: Rc<PathBuf>) {
        if file != self.name {
            let count = self.coupled_files.entry(file).or_insert(0);
            *count += 1;
        }
    }
    fn add_files(&mut self, files: Vec<Rc<PathBuf>>) {
        for file in files {
            self.add_file(file)
        }
        self.commits += 1;
    }
    fn filter_by_ratio(&self, min_coupling_ratio: f64) -> FileStats {
        let commits = self.commits as f64;
        FileStats {
            name: self.name.clone(),
            commits: self.commits,
            coupled_files: self
                .coupled_files
                .iter()
                .filter(|(_file, days)| **days as f64 / commits >= min_coupling_ratio)
                .map(|(file, days)| (file.clone(), *days))
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
struct CouplingBucket {
    bucket_start: u64,
    bucket_size: u64,
    file_stats: HashMap<Rc<PathBuf>, FileStats>,
}

impl CouplingBucket {
    fn new(bucket_start: u64, bucket_size: u64) -> Self {
        CouplingBucket {
            bucket_start,
            bucket_size,
            file_stats: HashMap::new(),
        }
    }
    fn add_files(
        &mut self,
        files: Vec<Rc<PathBuf>>,
        previous_day: Option<Vec<Rc<PathBuf>>>,
        next_day: Option<Vec<Rc<PathBuf>>>,
    ) {
        // source is just today's files
        // destination is any unique files in yesterday, today or tomorow
        let mut all_destinations = files.clone();
        if let Some(mut prev) = previous_day {
            all_destinations.append(&mut prev);
        }
        if let Some(mut next) = next_day {
            all_destinations.append(&mut next);
        }
        all_destinations.sort();
        all_destinations.dedup();
        for file in files.iter() {
            let entry = self
                .file_stats
                .entry(file.clone())
                .or_insert_with(|| FileStats::new(file.clone()));
            (*entry).add_files(all_destinations.clone());
        }
    }
    /// filter the bucket to remove noise
    /// min_source_days is the minimum number of days a file should have existed for it to be included
    /// min_coupling_ratio is the overall ratio of dest days / source days for the destination to be included.
    fn filter_by(&mut self, min_source_days: u64, min_coupling_ratio: f64) {
        self.file_stats = self
            .file_stats
            .drain()
            .filter(|(_file, file_stats)| file_stats.commits >= min_source_days)
            .map(|(file, file_stats)| (file, file_stats.filter_by_ratio(min_coupling_ratio)))
            .collect();
    }
}

struct CouplingBuckets {
    config: BucketingConfig,
    buckets: Vec<(u64, CouplingBucket)>,
}

impl CouplingBuckets {
    fn new(
        config: CouplingConfig,
        daily_stats: &DailyStats,
        bucketing_config: BucketingConfig,
    ) -> Self {
        let bucket_size = bucketing_config.bucket_size;
        CouplingBuckets {
            config: bucketing_config,
            buckets: (0..bucketing_config.bucket_count)
                .map(|bucket| {
                    let bucket_start: u64 = bucketing_config.bucket_start(bucket);
                    info!(
                        "Processing bucket {} of {}",
                        bucket, bucketing_config.bucket_count
                    );
                    let mut coupling_bucket = CouplingBucket::new(bucket_start, bucket_size);
                    daily_stats
                        .iter()
                        .filter(|(date, _files)| {
                            **date >= bucket_start && **date <= (bucket_start + bucket_size - 1)
                        })
                        .for_each(|(date, files)| {
                            let previous_day = daily_stats.get(*date - (24 * 60 * 60));
                            let next_day = daily_stats.get(*date + (24 * 60 * 60));
                            coupling_bucket.add_files(
                                files.clone(),
                                previous_day.cloned(),
                                next_day.cloned(),
                            );
                        });
                    coupling_bucket.filter_by(config.min_source_days, config.min_coupling_ratio);
                    (bucket, coupling_bucket)
                })
                .filter(|(_, coupling_bucket)| !coupling_bucket.file_stats.is_empty())
                .collect(),
        }
    }
    fn all_files(&self) -> HashSet<Rc<PathBuf>> {
        self.buckets
            .iter()
            .flat_map(|(_, coupling_bucket)| coupling_bucket.file_stats.keys().cloned())
            .collect()
    }
    fn file_coupling_data(&self, file: Rc<PathBuf>) -> SerializableCouplingData {
        SerializableCouplingData::new(
            self.buckets
                .iter()
                .filter(|(_bucket, coupling_bucket)| {
                    coupling_bucket.file_stats.contains_key(&file.clone())
                })
                .map(|(bucket, coupling_bucket)| {
                    let bucket_start =
                        self.config.first_bucket_start + bucket * self.config.bucket_size;
                    let bucket_end = bucket_start + self.config.bucket_size - 1;
                    let stats = coupling_bucket.file_stats.get(&file.to_owned()).unwrap();
                    let commit_days = stats.commits;
                    let coupled_files = stats
                        .coupled_files
                        .iter()
                        .map(|(file, count)| (file.clone(), *count))
                        .collect();
                    SerializableCouplingBucketData {
                        bucket_start,
                        bucket_end,
                        commit_days,
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
    pub commit_days: u64,
    pub coupled_files: Vec<(Rc<PathBuf>, u64)>,
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
    min_source_days: u64,
    // ignore if commits(to) / commits(from) is less than this - so if A is committed 100 days in a bucket, and B is on 20 of the same days, it would pass with a 0.2 ratio or higher
    min_coupling_ratio: f64,
}

impl CouplingConfig {
    pub fn new(bucket_days: u64, min_source_days: u64, min_coupling_ratio: f64) -> Self {
        CouplingConfig {
            bucket_days,
            min_source_days,
            min_coupling_ratio,
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
            min_source_days: 10,
            min_coupling_ratio: 0.25,
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

fn daily_stats_to_buckets(
    tree: &FlareTreeNode,
    config: CouplingConfig,
) -> Result<Option<(BucketingConfig, CouplingBuckets)>, Error> {
    let mut daily_stats = DailyStats::new(&tree)?;

    if daily_stats.is_empty() {
        warn!("No stats found, no coupling data processed");
        return Ok(None);
    }

    info!("Gathering coupling stats - building buckets");

    let earliest = daily_stats.earliest();
    let latest = daily_stats.latest();

    let bucketing_config = BucketingConfig::new(config, earliest, latest);

    let filtered_buckets = CouplingBuckets::new(config, &daily_stats, bucketing_config);
    Ok(Some((bucketing_config, filtered_buckets)))
}

pub fn gather_coupling(tree: &mut FlareTreeNode, config: CouplingConfig) -> Result<(), Error> {
    info!("Gathering coupling stats - accumulating daily counts");
    let bucket_info = daily_stats_to_buckets(tree, config)?;

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

// TODO:
// add stats to tree root metadata

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::ffi::OsString;
    use std::path::Path;
    use test_shared::*;

    const DAY_SIZE: u64 = 24 * 60 * 60;
    const TEST_START: u64 = (2020 - 1970) * 365 * 24 * 60 * 60;
    const DAY1: u64 = TEST_START + DAY_SIZE;
    const DAY21: u64 = TEST_START + 21 * DAY_SIZE;
    const DAY22: u64 = TEST_START + 22 * DAY_SIZE;
    const DAY23: u64 = TEST_START + 23 * DAY_SIZE;
    const DAY24: u64 = TEST_START + 24 * DAY_SIZE;
    const DAY27: u64 = TEST_START + 27 * DAY_SIZE;
    const DAY29: u64 = TEST_START + 29 * DAY_SIZE;

    #[derive(Debug, PartialEq, Serialize)]
    struct FakeGitDetails {
        commit_day: u64,
    }

    #[derive(Debug, PartialEq, Serialize)]
    struct FakeGitData {
        details: Option<Vec<FakeGitDetails>>,
    }

    impl FakeGitData {
        fn new(days: &Vec<u64>) -> Self {
            FakeGitData {
                details: Some(
                    days.iter()
                        .map(|day| FakeGitDetails { commit_day: *day })
                        .collect(),
                ),
            }
        }
        fn to_json(&self) -> Value {
            serde_json::value::to_value(self).unwrap()
        }
    }

    fn build_test_tree() -> FlareTreeNode {
        let mut root = FlareTreeNode::dir("root");
        let mut root_file_1 = FlareTreeNode::file("root_file_1.txt");
        root_file_1.add_data("git", FakeGitData::new(&vec![DAY1, DAY21]).to_json());
        root_file_1.add_data("loc", json!({"code": 12}));
        root.append_child(root_file_1);
        root.append_child(FlareTreeNode::file("root_file_2.txt"));
        let mut child1 = FlareTreeNode::dir("child1");
        let mut child1_file1 = FlareTreeNode::file("child1_file_1.txt");
        child1_file1.add_data("git", FakeGitData::new(&vec![DAY21, DAY22]).to_json());
        child1_file1.add_data("loc", json!({"code": 122}));
        child1.append_child(child1_file1);
        let mut child1_file2 = FlareTreeNode::file("binary_file.zip");
        child1_file2.add_data(
            "git",
            FakeGitData::new(&vec![DAY21, DAY22, DAY23]).to_json(),
        );
        child1.append_child(child1_file2);
        root.append_child(child1);
        root
    }

    #[test]
    fn can_convert_tree_to_daily_stats() {
        let tree = build_test_tree();
        let stats = DailyStats::new(&tree).unwrap();
        assert_eq!(stats.is_empty(), false);

        let mut expected: HashMap<u64, Vec<Rc<PathBuf>>> = HashMap::new();
        expected.insert(
            DAY1,
            vec![Rc::new(Path::new("root_file_1.txt").to_path_buf())],
        );
        expected.insert(
            DAY21,
            vec![
                Rc::new(Path::new("root_file_1.txt").to_path_buf()),
                Rc::new(Path::new("child1/child1_file_1.txt").to_path_buf()),
            ],
        );
        expected.insert(
            DAY22,
            vec![Rc::new(Path::new("child1/child1_file_1.txt").to_path_buf())],
        );

        assert_eq!(expected, stats.stats)
    }

    #[test]
    fn can_get_daily_stats_early_late() {
        let tree = build_test_tree();
        let stats = DailyStats::new(&tree).unwrap();
        assert_eq!(stats.earliest(), DAY1);
        assert_eq!(stats.latest(), DAY22);
    }

    #[test]
    fn can_get_bucket_sizes_from_config() {
        let config = CouplingConfig {
            bucket_days: 20,
            min_source_days: 1,
            min_coupling_ratio: 0.001,
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

    fn make_test_daily_stats(data: Vec<(u64, Vec<&str>)>) -> DailyStats {
        let stats_inner: HashMap<u64, Vec<Rc<PathBuf>>> = data
            .iter()
            .map(|(day, namelist)| {
                let paths: Vec<Rc<PathBuf>> = namelist
                    .iter()
                    .map(|name| Rc::new(Path::new(name).to_path_buf()))
                    .collect();
                (*day, paths)
            })
            .collect();
        DailyStats { stats: stats_inner }
    }

    #[test]
    fn can_build_coupling_buckets_from_stats() {
        let stats = make_test_daily_stats(vec![
            (DAY1, vec!["foo", "bar"]),
            (DAY21, vec!["foo", "baz"]),
        ]);
        // config is effectively not filtering anything
        let config = CouplingConfig {
            bucket_days: 20,
            min_source_days: 1,
            min_coupling_ratio: 0.001,
        };
        let bucketing_config = BucketingConfig::new(config, DAY1, DAY22);

        let coupling_buckets = CouplingBuckets::new(config, &stats, bucketing_config);

        let mut files: Vec<OsString> = coupling_buckets
            .all_files()
            .iter()
            .map(|f| f.as_os_str().to_os_string())
            .collect();
        files.sort();

        assert_eq!(files, vec!["bar", "baz", "foo"]);

        // if I were doing this properly I'd test the coupling data - but this is a side project,
        // I'll leave it as a TODO item ...
        // and test by serializing - more of an integration test than a unit test...
        let bar_data = coupling_buckets.file_coupling_data(Rc::new(Path::new("bar").to_path_buf()));
        let bar_value = serde_json::value::to_value(bar_data).expect("Can't serialize!");
        let bar_expected = json!({
          "buckets": [
          {
            "bucket_start": bucketing_config.bucket_start(0),
            "bucket_end": bucketing_config.bucket_start(0) + bucketing_config.bucket_size - 1,
            "commit_days": 1,
            "coupled_files": [["foo", 1]]
          }
          ]
        });
        assert_eq!(bar_value, bar_expected);

        let baz_data = coupling_buckets.file_coupling_data(Rc::new(Path::new("baz").to_path_buf()));
        let baz_value = serde_json::value::to_value(baz_data).expect("Can't serialize!");
        let baz_expected = json!({
          "buckets": [
          {
            "bucket_start": bucketing_config.bucket_start(1),
            "bucket_end": bucketing_config.bucket_start(1) + bucketing_config.bucket_size - 1,
            "commit_days": 1,
            "coupled_files": [["foo", 1]]
          }
          ]
        });
        assert_eq!(baz_value, baz_expected);

        let foo_data = coupling_buckets.file_coupling_data(Rc::new(Path::new("foo").to_path_buf()));
        let foo_value = serde_json::value::to_value(foo_data).expect("Can't serialize!");
        let foo_expected = json!({
          "buckets": [
          {
            "bucket_start": bucketing_config.bucket_start(0),
            "bucket_end": bucketing_config.bucket_start(0) + bucketing_config.bucket_size - 1,
            "commit_days": 1,
            "coupled_files": [["bar", 1]]
          },
          {
            "bucket_start": bucketing_config.bucket_start(1),
            "bucket_end": bucketing_config.bucket_start(1) + bucketing_config.bucket_size - 1,
            "commit_days": 1,
            "coupled_files": [["baz", 1]]
          }
          ]
        });
        assert_eq!(foo_value, foo_expected);
    }

    #[test]
    fn coupling_is_filtered_and_calculated_as_ratio_of_commits_to_others() {
        // test setup - bucket 1 has enough data for "foo" to have coupling,
        //  bucket 0 doesn't.
        // bucket1 shows you can get commits from days before or after.
        // foo should be coupled to baz, as it has 2 commits, and both of them share a day with baz
        // foo isn't coupled to bar as it only has one commit in common
        let stats = make_test_daily_stats(vec![
            (DAY1, vec!["foo", "bar"]),
            (DAY21, vec!["baz"]),
            (DAY22, vec!["foo"]),
            (DAY23, vec!["foo", "baz"]),
            (DAY24, vec!["bar"]),
            (DAY27, vec!["foo"]),
            (DAY29, vec!["foo"]),
        ]);

        let config = CouplingConfig {
            bucket_days: 20,
            min_source_days: 2,
            min_coupling_ratio: 0.5,
        };
        let bucketing_config = BucketingConfig::new(config, DAY1, DAY29);

        let coupling_buckets = CouplingBuckets::new(config, &stats, bucketing_config);

        let mut files: Vec<OsString> = coupling_buckets
            .all_files()
            .iter()
            .map(|f| f.as_os_str().to_os_string())
            .collect();
        files.sort();

        assert_eq!(files, vec!["baz", "foo"]);

        let foo_data = coupling_buckets.file_coupling_data(Rc::new(Path::new("foo").to_path_buf()));
        let foo_value = serde_json::value::to_value(foo_data).expect("Can't serialize!");
        let foo_expected = json!({
          "buckets": [
          {
            "bucket_start": bucketing_config.bucket_start(1),
            "bucket_end": bucketing_config.bucket_start(1) + bucketing_config.bucket_size - 1,
            "commit_days": 4,
            "coupled_files": [["baz", 2]]
          }
          ]
        });
        assert_eq!(foo_value, foo_expected);
    }
}
