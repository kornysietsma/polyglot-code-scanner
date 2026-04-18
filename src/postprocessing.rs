use crate::git::GitNodeData;
use crate::{flare::FlareTreeNode, ScannerConfig};
use anyhow::Error;
use log::info;
use std::time::{SystemTime, UNIX_EPOCH};

fn remove_details(node: &mut FlareTreeNode, config: &ScannerConfig) -> Result<(), Error> {
    if let Some(GitNodeData::File { data }) = &mut node.indicators_mut().git {
        if !config.features.git_details {
            data.details = Vec::new();
        }
        data.activity = Vec::new();
    }
    for child in node.get_children_mut() {
        remove_details(child, config)?;
    }
    Ok(())
}

/// Check if a directory is a git root (has GitNodeData::Dir)
fn is_git_root(node: &FlareTreeNode) -> bool {
    matches!(node.indicators().git, Some(GitNodeData::Dir { .. }))
}

/// Check if a git root should be pruned based on inactivity cutoff
/// Returns true if the git root should be pruned (is inactive beyond the cutoff)
fn should_prune_git_root_by_date(node: &FlareTreeNode, prune_years: u64, now: u64) -> bool {
    if let Some(GitNodeData::Dir { data }) = &node.indicators().git {
        if let Some(latest_date) = data.latest_commit_date {
            let cutoff_timestamp = now.saturating_sub(prune_years * 365 * 86400);
            return latest_date < cutoff_timestamp;
        }
    }
    false
}

/// Remove directories that are inactive by date cutoff (directory-level pruning with git root awareness)
///
/// **Pruning Strategy:**
/// - Only prune git roots that are inactive (latest_commit_date older than cutoff)
/// - Keep all non-git directories (no structural anchor to identify pruning units)
/// - Keep all files (they belong to their parent directory)
///
/// **Rationale:**
/// - Git roots (with .git) are atomic units; pruning at this level makes sense
/// - Non-git directories have no git history to evaluate; without a clear semantic anchor,
///   keeping them is the conservative choice (avoids accidentally deleting legitimate content)
/// - This is a rare scenario anyway (most code is git-tracked)
///
/// **Behavior Examples:**
/// - Git root with recent commits → KEEP with all contents
/// - Git root with only old commits → PRUNE entirely
/// - Directory with no git data anywhere → KEEP (could be docs, resources, etc.)
/// - File inside any directory → KEEP with its parent directory
fn prune_inactive(node: &mut FlareTreeNode, config: &ScannerConfig) -> Result<(), Error> {
    if config.prune_inactive_years.is_none() || !config.features.git {
        return Ok(());
    }

    if node.is_file() {
        return Ok(()); // Don't prune individual files
    }

    let prune_years = config.prune_inactive_years.unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Filter children: only remove git roots that are inactive by date
    let children = node.get_children_mut();
    children.retain(|child| {
        // Keep files always (they belong to their parent directory)
        if child.is_file() {
            return true;
        }

        // Only prune if this is a git root AND it's inactive by date
        if is_git_root(child) && should_prune_git_root_by_date(child, prune_years, now) {
            info!(
                "Pruning git root {:?} - inactive for more than {} years",
                child.name(),
                prune_years
            );
            return false;
        }

        // Keep everything else:
        // - Active git roots
        // - All non-git directories (conservative: no way to determine if they should be pruned)
        true
    });

    // Recursively apply pruning only to non-git-root directories
    // Git roots keep all their contents once they pass the inactivity check
    for child in node.get_children_mut() {
        if !child.is_file() && !is_git_root(child) {
            prune_inactive(child, config)?;
        }
    }

    Ok(())
}

/// Recursively collect timestamp bounds from all files in a git root subtree
fn collect_file_timestamps(
    node: &FlareTreeNode,
    earliest: &mut Option<u64>,
    latest: &mut Option<u64>,
) {
    if let Some(GitNodeData::File { data }) = &node.indicators().git {
        // Update latest from last_update
        *latest = Some(latest.map_or(data.last_update, |l| l.max(data.last_update)));

        // Update earliest from creation_date if available, otherwise from last_update
        let file_earliest = data.creation_date.unwrap_or(data.last_update);
        *earliest = Some(earliest.map_or(file_earliest, |e| e.min(file_earliest)));
    }

    // Recurse into children
    for child in node.get_children() {
        collect_file_timestamps(child, earliest, latest);
    }
}

/// Populate earliest_commit_date and latest_commit_date for all git roots in the tree
fn populate_git_root_timestamps(node: &mut FlareTreeNode) -> Result<(), Error> {
    // First, check if this node is a git root and collect timestamps if so
    let is_git_root = matches!(node.indicators().git, Some(GitNodeData::Dir { .. }));

    if is_git_root {
        let mut earliest: Option<u64> = None;
        let mut latest: Option<u64> = None;

        // Collect timestamps from all descendant files
        for child in node.get_children() {
            collect_file_timestamps(child, &mut earliest, &mut latest);
        }

        // Update the GitInfo with the collected timestamps
        if let Some(GitNodeData::Dir { data }) = &mut node.indicators_mut().git {
            data.earliest_commit_date = earliest;
            data.latest_commit_date = latest;
        }
    }

    // Recursively process children (they might contain nested git roots)
    for child in node.get_children_mut() {
        if !child.is_file() {
            populate_git_root_timestamps(child)?;
        }
    }

    Ok(())
}

pub fn postprocess_tree(tree: &mut FlareTreeNode, config: &ScannerConfig) -> Result<(), Error> {
    info!("Postprocessing tree before persisting");
    populate_git_root_timestamps(tree)?;
    remove_details(tree, config)?;
    prune_inactive(tree, config)?;
    Ok(())
}
