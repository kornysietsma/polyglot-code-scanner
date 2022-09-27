use crate::{flare::FlareTreeNode, git::GitNodeData, ScannerConfig};
use anyhow::Error;

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

pub fn postprocess_tree(tree: &mut FlareTreeNode, config: &ScannerConfig) -> Result<(), Error> {
    info!("Postprocessing tree before persisting");
    remove_details(tree, config)?;
    Ok(())
}
