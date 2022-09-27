#![warn(clippy::all)]
//! Data formats for JSON output from the scanner
//!
//! Data format should now follow semantic versioning - a major version change is incompatible, a minor version change is backward compatible, a patch version is mostly around bug fixes.

use serde::Serialize;
use uuid::Uuid;

use crate::{
    coupling::CouplingMetadata, flare::FlareTreeNode, git_user_dictionary::GitUserDictionary,
    FeatureFlags,
};

pub static DATA_FILE_VERSION: &str = "1.0.2";

#[derive(Debug, Serialize)]
pub struct GitMetadata {
    pub users: GitUserDictionary,
}
#[derive(Debug, Serialize, Default)]
pub struct IndicatorMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<GitMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coupling: Option<CouplingMetadata>,
}

#[derive(Debug, Serialize)]
pub struct PolyglotData {
    version: String,
    name: String,
    id: String,
    tree: FlareTreeNode,
    metadata: IndicatorMetadata,
    features: FeatureFlags,
}

impl PolyglotData {
    pub fn new(name: &str, id: Option<&str>, tree: FlareTreeNode, features: FeatureFlags) -> Self {
        let id = id.map_or_else(
            || Uuid::new_v4().as_hyphenated().to_string(),
            std::string::ToString::to_string,
        );
        PolyglotData {
            version: DATA_FILE_VERSION.to_string(),
            name: name.to_string(),
            id,
            tree,
            metadata: IndicatorMetadata::default(),
            features,
        }
    }
    pub fn tree(&self) -> &FlareTreeNode {
        &self.tree
    }
    pub fn tree_mut(&mut self) -> &mut FlareTreeNode {
        &mut self.tree
    }

    pub fn metadata(&mut self) -> &mut IndicatorMetadata {
        &mut self.metadata
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;
    #[test]
    fn can_build_data_tree() {
        let root = FlareTreeNode::dir("root");
        let tree: PolyglotData = PolyglotData::new(
            "test",
            Some("test-id"),
            root.clone(),
            FeatureFlags::default(),
        );

        let expected = PolyglotData {
            name: "test".to_string(),
            id: "test-id".to_string(),
            version: DATA_FILE_VERSION.to_string(),
            tree: root,
            metadata: IndicatorMetadata::default(),
            features: FeatureFlags::default(),
        };

        assert_eq!(tree.name, expected.name);
        assert_eq!(tree.tree, expected.tree);
    }

    #[test]
    fn data_without_id_has_uuid() {
        let root = FlareTreeNode::dir("root");
        let tree1: PolyglotData =
            PolyglotData::new("test", None, root.clone(), FeatureFlags::default());
        let tree2: PolyglotData = PolyglotData::new("test", None, root, FeatureFlags::default());
        // really just asserting IDs are different!
        assert_ne!(tree1.id, tree2.id);
    }

    // TODO: removed serializing metadata test as it no longer made sense. Do we depend on just e2e tests?
}
