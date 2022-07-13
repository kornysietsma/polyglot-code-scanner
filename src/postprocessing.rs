use crate::{flare::FlareTreeNode, ScannerConfig};
use failure::Error;
use std::collections::hash_map::Entry;

fn remove_details(node: &mut FlareTreeNode, key: &str, value: &str) -> Result<(), Error> {
    if let Entry::Occupied(mut entry) = node.get_data_entry(key.to_string()) {
        if let Some(map) = entry.get_mut().as_object_mut() {
            map.remove_entry(value);
        }
    }
    for child in node.get_children_mut() {
        remove_details(child, key, value)?;
    }
    Ok(())
}

pub fn postprocess_tree(tree: &mut FlareTreeNode, config: ScannerConfig) -> Result<(), Error> {
    info!("Postprocessing tree before persisting");
    if !config.detailed {
        remove_details(tree, "git", "details")?;
    }
    // TODO: remove per node, this is traversing the tree twice!
    remove_details(tree, "git", "activity")?;
    Ok(())
}
