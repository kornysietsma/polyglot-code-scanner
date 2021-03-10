use crate::{flare::FlareTreeNode, CalculatorConfig};
use failure::Error;
use std::collections::hash_map::Entry;

fn remove_details(node: &mut FlareTreeNode) -> Result<(), Error> {
    if let Entry::Occupied(mut entry) = node.get_data_entry("git".to_string()) {
        if let Some(map) = entry.get_mut().as_object_mut() {
            map.remove_entry("details");
        }
    }
    for child in node.get_children_mut() {
        remove_details(child)?;
    }
    Ok(())
}

pub fn postprocess_tree(tree: &mut FlareTreeNode, config: CalculatorConfig) -> Result<(), Error> {
    info!("Postprocessing tree before persisting");
    if !config.detailed {
        remove_details(tree)?;
    }
    Ok(())
}
