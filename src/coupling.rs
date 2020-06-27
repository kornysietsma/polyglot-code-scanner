use super::git::{GitData, GitDetails};
use crate::flare::FlareTreeNode;
use failure::Error;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::default::Default;
use std::ffi::{OsStr, OsString};
use std::path::{Components, Path, PathBuf};

fn accumulate_stats(
    daily_stats: &mut HashMap<u64, Vec<PathBuf>>,
    node: &FlareTreeNode,
    path: PathBuf,
) -> Result<(), Error> {
    if let Some(Value::Object(value)) = node.get_data("git") {
        if let Some(Value::Array(details)) = value.get("details") {
            for detail_value in details {
                if let Some(commit_day) =
                    detail_value.pointer("/commit_day").and_then(|x| x.as_u64())
                {
                    let daily_stat = daily_stats.entry(commit_day).or_insert_with(Vec::new);
                    (*daily_stat).push(path.clone());
                }
            }
        }
    };

    for child in node.get_children() {
        let mut child_path = path.clone();
        child_path.push(child.name());
        accumulate_stats(daily_stats, &child, child_path)?;
    }
    Ok(())
}

#[derive(Debug)]
struct FileStats {
    name: PathBuf,
    commits: u64,
    coupled_files: HashMap<PathBuf, u64>,
}

impl FileStats {
    fn new(name: PathBuf) -> Self {
        FileStats {
            name,
            commits: 0,
            coupled_files: HashMap::new(),
        }
    }
    fn add_file(&mut self, file: PathBuf) {
        if file != self.name {
            let count = self.coupled_files.entry(file).or_insert(0);
            *count += 1;
        }
    }
    fn add_files(&mut self, files: Vec<PathBuf>) {
        for file in files {
            self.add_file(file)
        }
        self.commits += 1;
    }
    fn filter_by_ratio(&self, min_coupling_ratio: f64) -> FileStats {
        // println!("Filtering by ratio: {:?}", self);
        let commits = self.commits as f64;
        FileStats {
            name: self.name.to_owned(),
            commits: self.commits,
            coupled_files: self
                .coupled_files
                .iter()
                .filter(|(_file, days)| **days as f64 / commits >= min_coupling_ratio)
                .map(|(file, days)| (file.to_owned(), *days))
                .collect(),
        }
    }
}

#[derive(Debug)]
struct CouplingBucket {
    bucket_start: u64,
    bucket_size: u64,
    file_stats: HashMap<PathBuf, FileStats>,
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
        files: Vec<PathBuf>,
        previous_day: Option<Vec<PathBuf>>,
        next_day: Option<Vec<PathBuf>>,
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
                .entry(file.to_owned())
                .or_insert_with(|| FileStats::new(file.to_owned()));
            (*entry).add_files(all_destinations.clone());
        }
    }
    /// filter the bucket to remove noise
    /// min_source_days is the minimum number of days a file should have existed for it to be included
    /// min_coupling_ratio is the overall ratio of dest days / source days for the destination to be included.
    fn filter_by(&mut self, min_source_days: u64, min_coupling_ratio: f64) {
        // println!("Filtering bucket with {} files", self.file_stats.len());
        // println!("files: {:?}", self.file_stats);

        self.file_stats = self
            .file_stats
            .drain()
            .filter(|(_file, file_stats)| file_stats.commits >= min_source_days)
            .map(|(file, file_stats)| (file, file_stats.filter_by_ratio(min_coupling_ratio)))
            .collect();
    }

    // fn update_tree(&self, tree: &mut FlareTreeNode) {
    //     for (file, stats) in self.file_stats.iter() {
    //         let tree_node = tree.get_in_mut(&mut file.components().clone());
    //     }
    // }
}

#[derive(Debug, PartialEq, Serialize)]
struct CouplingBucketData {
    pub bucket_start: u64,
    pub bucket_end: u64,
    pub commit_days: u64,
    pub coupled_files: Vec<(PathBuf, u64)>,
}

#[derive(Debug, PartialEq, Serialize)]
struct CouplingData {
    pub buckets: Vec<CouplingBucketData>,
}

impl CouplingData {
    fn from(
        filtered_buckets: &[(u64, CouplingBucket)],
        bucket_size: u64,
        first_bucket_start: u64,
        file: &Path,
    ) -> Self {
        CouplingData {
            buckets: filtered_buckets
                .iter()
                .filter(|(_bucket, coupling_bucket)| {
                    coupling_bucket.file_stats.contains_key(&file.to_owned())
                })
                .map(|(bucket, coupling_bucket)| {
                    let bucket_start = first_bucket_start + bucket * bucket_size;
                    let bucket_end = bucket_start + bucket_size - 1;
                    let stats = coupling_bucket.file_stats.get(&file.to_owned()).unwrap();
                    let commit_days = stats.commits;
                    let coupled_files = stats
                        .coupled_files
                        .iter()
                        .map(|(file, count)| (file.clone(), *count))
                        .collect();
                    CouplingBucketData {
                        bucket_start,
                        bucket_end,
                        commit_days,
                        coupled_files,
                    }
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CouplingConfig {
    // number of days in a bucket
    pub bucket_days: u64,
    // ignore if a "from" file isn't changed this often in a bucket - avoid coincidental change noise
    pub min_source_days: u64,
    // ignore if commits(to) / commits(from) is less than this - so if A is committed 100 days in a bucket, and B is on 20 of the same days, it would pass with a 0.2 ratio or higher
    min_coupling_ratio: f64,
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

pub fn gather_coupling(tree: &mut FlareTreeNode, config: CouplingConfig) -> Result<(), Error> {
    println!("Gathering coupling stats - accumulating daily counts");
    let bucketsize = config.bucket_days * 24 * 60 * 60; // TODO - can we work in days not secs?
    let mut daily_stats: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    accumulate_stats(&mut daily_stats, &tree, PathBuf::new())?;
    println!("Gathering coupling stats - building buckets");
    // println!("{:#?}", daily_stats);
    if daily_stats.is_empty() {
        println!("No stats found, no coupling data processed");
        return Ok(());
    }
    let earliest = daily_stats.keys().min().unwrap();
    let latest = daily_stats.keys().max().unwrap();
    // want buckets that end with the last day of the last bucket the latest day
    let bucket_count = ((latest - earliest) / bucketsize) + 1;
    let first_bucket_start = (latest - (bucketsize * bucket_count)) + 1;
    // what I need:

    let filtered_buckets: Vec<(u64, CouplingBucket)> = (0..bucket_count)
        .map(|bucket| {
            println!("Bucket {} of {}", bucket, bucket_count);
            let bucket_start: u64 = first_bucket_start + bucket * bucketsize;
            let mut coupling_bucket = CouplingBucket::new(bucket_start, bucketsize);

            daily_stats
                .iter()
                .filter(|(date, _files)| {
                    **date >= bucket_start && **date <= (bucket_start + bucketsize - 1)
                })
                .for_each(|(date, files)| {
                    let previous_day = daily_stats.get(&(*date - (24 * 60 * 60)));
                    let next_day = daily_stats.get(&(*date + (24 * 60 * 60)));
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
        .collect();
    let all_files: HashSet<PathBuf> = filtered_buckets
        .iter()
        .flat_map(|(_, coupling_bucket)| coupling_bucket.file_stats.keys().cloned())
        .collect();

    for file in all_files {
        if let Some(tree_node) = tree.get_in_mut(&mut file.components().clone()) {
            let coupling_data =
                CouplingData::from(&filtered_buckets, bucketsize, first_bucket_start, &file);
            tree_node.add_data(
                "coupling",
                serde_json::value::to_value(coupling_data)
                    .expect("Serializable object couldn't be serialized to JSON"),
            );
        } else {
            println!("Can't find {:?} in tree!", &file);
        };
    }

    println!("Gathering coupling stats - done");
    Ok(())
}
