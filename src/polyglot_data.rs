#![warn(clippy::all)]
//! Data formats for JSON output from the scanner
//!
//! Data format should now follow semantic versioning - a major version change is incompatible, a minor version change is backward compatible, a patch version is mostly around bug fixes.

use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::flare::FlareTreeNode;

pub static DATA_FILE_VERSION: &str = "1.0.0";

#[derive(Debug, PartialEq, Serialize)]
pub struct PolyglotData {
    version: String,
    name: String,
    id: String,
    tree: FlareTreeNode,
    // TODO: more strongly typed data!  Value is just JSON - we should have an enum of valid data.
    metadata: HashMap<String, Value>,
}

impl PolyglotData {
    pub fn new(name: &str, id: Option<&str>, tree: FlareTreeNode) -> Self {
        let id = id
            .map(|i| i.to_string())
            .unwrap_or_else(|| Uuid::new_v4().as_hyphenated().to_string());
        PolyglotData {
            version: DATA_FILE_VERSION.to_string(),
            name: name.to_string(),
            id,
            tree,
            metadata: HashMap::new(),
        }
    }
    pub fn tree(&self) -> &FlareTreeNode {
        &self.tree
    }
    pub fn tree_mut(&mut self) -> &mut FlareTreeNode {
        &mut self.tree
    }

    pub fn add_metadata<S: Into<String>>(&mut self, key: S, value: Value) {
        self.metadata.insert(key.into(), value);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use test_shared::*;
    #[test]
    fn can_build_data_tree() {
        let root = FlareTreeNode::dir("root");
        let tree: PolyglotData = PolyglotData::new("test", Some("test-id"), root.clone());

        assert_eq!(
            tree,
            PolyglotData {
                name: "test".to_string(),
                id: "test-id".to_string(),
                version: DATA_FILE_VERSION.to_string(),
                tree: root,
                metadata: HashMap::new()
            }
        )
    }

    #[test]
    fn data_without_id_has_uuid() {
        let mut root = FlareTreeNode::dir("root");
        let tree1: PolyglotData = PolyglotData::new("test", None, root.clone());
        let tree2: PolyglotData = PolyglotData::new("test", None, root);
        // really just asserting IDs are different!
        assert_ne!(tree1.id, tree2.id);
    }

    #[test]
    fn can_serialize_file_with_metadata_value_to_json() {
        let root = FlareTreeNode::file("foo.txt");
        let mut tree: PolyglotData = PolyglotData::new("test", Some("test-id"), root);
        let value = json!({"foo": ["bar", "baz", 123]});
        tree.add_metadata("bat", value);

        assert_eq_json_str(
            &tree,
            r#"{
                    "name":"test",
                    "id":"test-id",
                    "version":"1.0.0",
                    "tree": {
                      "name":"foo.txt"
                    },
                    "metadata": {"bat": {"foo": ["bar", "baz", 123]}}
                }"#,
        )
    }
}
