use std::{fs, path::Path};

use anyhow::Error;
use filetime::FileTime;
use serde::Serialize;

use crate::{
    flare::FlareTreeNode, polyglot_data::IndicatorMetadata,
    toxicity_indicator_calculator::ToxicityIndicatorCalculator,
};

/// File creation and modification times, in seconds since unix epoch
/// using the filetime crate so Windows times are converted to unix times!
#[derive(Debug, PartialEq, Clone, Serialize, Default)]
pub struct FileStats {
    created: i64,
    modified: i64,
}

impl FileStats {
    fn new(path: &Path) -> Result<Self, Error> {
        let metadata = fs::metadata(path)?;
        let ctime = FileTime::from_creation_time(&metadata);
        let mtime = FileTime::from_last_modification_time(&metadata);
        match (ctime, mtime) {
            (Some(ctime), mtime) => Ok(FileStats {
                created: ctime.unix_seconds(),
                modified: mtime.unix_seconds(),
            }),
            (None, mtime) => {
                warn!("File has no ctime - using mtime");
                Ok(FileStats {
                    created: mtime.unix_seconds(),
                    modified: mtime.unix_seconds(),
                })
            }
        }
    }
}
#[derive(Debug)]
pub struct FileStatsCalculator {}

impl ToxicityIndicatorCalculator for FileStatsCalculator {
    fn name(&self) -> String {
        "file_stats".to_string()
    }

    fn visit_node(&mut self, node: &mut FlareTreeNode, path: &Path) -> Result<(), Error> {
        node.indicators_mut().file_stats = Some(FileStats::new(path)?);

        Ok(())
    }

    fn apply_metadata(&self, _metadata: &mut IndicatorMetadata) -> Result<(), Error> {
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::time::SystemTime;

    use super::*;
    use std::time::UNIX_EPOCH;
    use tempfile::{NamedTempFile, TempDir};

    #[test]
    fn can_get_stats_for_a_file() -> Result<(), Error> {
        let newfile = NamedTempFile::new()?;

        let stats = FileStats::new(newfile.path())?;
        let now: i64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs()
            .try_into()?;

        assert!(stats.created > now - 1 && stats.created < now + 1);
        assert!(stats.modified > now - 1 && stats.modified < now + 1);

        Ok(())
    }
    #[test]
    fn can_get_stats_for_a_dir() -> Result<(), Error> {
        let newdir = TempDir::new()?;

        let stats = FileStats::new(newdir.path())?;
        let now: i64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs()
            .try_into()?;

        assert!(stats.created > now - 1 && stats.created < now + 1);
        assert!(stats.modified > now - 1 && stats.modified < now + 1);

        Ok(())
    }
}
