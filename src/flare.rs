#![warn(clippy::all)]
#![allow(dead_code)]

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use std::collections::HashMap;
use std::ffi::OsString;

#[derive(Debug, PartialEq)]
pub enum NodeValue {
    Dir {
        children: Vec<FlareTree>,
    },
    File {
        data: HashMap<String, serde_json::Value>,
    },
}

#[derive(Debug, PartialEq)]
pub struct FlareTree {
    name: OsString,
    value: NodeValue,
}

impl FlareTree {
    pub fn name(&self) -> &OsString {
        &self.name
    }

    pub fn data_entry<S: Into<String>>(&self, key: S) -> Option<&serde_json::Value> {
        if let NodeValue::File { ref data } = self.value {
            return data.get(&key.into());
        }
        None
    }

    pub fn from_file<S: Into<OsString>>(name: S) -> FlareTree {
        FlareTree {
            name: name.into(),
            value: NodeValue::File {
                data: HashMap::new(),
            },
        }
    }

    pub fn from_dir<S: Into<OsString>>(name: S) -> FlareTree {
        FlareTree {
            name: name.into(),
            value: NodeValue::Dir {
                children: Vec::new(),
            },
        }
    }

    pub fn add_file_data_as_value<S: Into<String>>(&mut self, key: S, value: serde_json::Value) {
        // TODO: error handling if not file
        //  or rethink structure
        if let NodeValue::File { ref mut data } = self.value {
            data.insert(key.into(), value);
        }
    }

    pub fn add_file_data<T: Serialize, S: Into<String>>(&mut self, key: S, value: &T) {
        // TODO: error handling if not file
        //  or rethink structure
        if let NodeValue::File { ref mut data } = self.value {
            data.insert(key.into(), serde_json::to_value(value).unwrap());
        }
    }

    pub fn append_child(&mut self, child: FlareTree) {
        // TODO: error handling if not dir
        //  or rethink structure
        if let NodeValue::Dir { ref mut children } = self.value {
            children.push(child);
        } // TODO: error handling!
    }

    pub fn get_in(&self, path: &mut std::path::Components) -> Option<&FlareTree> {
        match path.next() {
            Some(first_name) => {
                let dir_name = first_name.as_os_str();
                if let NodeValue::Dir { ref children } = self.value {
                    let first_match = children.iter().find(|c| dir_name == c.name)?;
                    return first_match.get_in(path);
                }
                None
            }
            None => Some(self),
        }
    }

    pub fn get_in_mut(&mut self, path: &mut std::path::Components) -> Option<&mut FlareTree> {
        match path.next() {
            Some(first_name) => {
                let dir_name = first_name.as_os_str();
                if let NodeValue::Dir { ref mut children } = self.value {
                    let first_match = children.iter_mut().find(|c| dir_name == c.name)?;
                    return first_match.get_in_mut(path);
                }
                None
            }
            None => Some(self),
        }
    }
}

impl Serialize for FlareTree {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("FlareTree", 2)?;
        let name_str = self.name.to_str().expect("Can't serialize!"); // TODO: how to convert to error result?
        state.serialize_field("name", &name_str)?;
        match &self.value {
            NodeValue::Dir { children } => state.serialize_field("children", children)?,
            NodeValue::File { data } => state.serialize_field("data", data)?,
        }

        state.end()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use regex::Regex;
    use serde_json::json;
    use serde_json::Value;
    use std::path::Path;

    #[test]
    fn can_build_tree() {
        let mut root = FlareTree::from_dir("root");
        root.append_child(FlareTree::from_file("child"));

        assert_eq!(
            root,
            FlareTree {
                name: OsString::from("root"),
                value: NodeValue::Dir {
                    children: vec![FlareTree {
                        name: OsString::from("child"),
                        value: NodeValue::File {
                            data: HashMap::new()
                        },
                    }]
                },
            }
        )
    }

    fn build_test_tree() -> FlareTree {
        let mut root = FlareTree::from_dir("root");
        root.append_child(FlareTree::from_file("root_file_1.txt"));
        root.append_child(FlareTree::from_file("root_file_2.txt"));
        let mut child1 = FlareTree::from_dir("child1");
        child1.append_child(FlareTree::from_file("child1_file_1.txt"));
        let mut grand_child = FlareTree::from_dir("grandchild");
        grand_child.append_child(FlareTree::from_file("grandchild_file.txt"));
        child1.append_child(grand_child);
        child1.append_child(FlareTree::from_file("child1_file_2.txt"));
        let mut child2 = FlareTree::from_dir("child2");
        let mut child2_file = FlareTree::from_file("child2_file.txt");
        let widget_data = json!({
            "sprockets": 7,
            "flanges": ["Nigel, Sarah"]
        });
        child2_file.add_file_data_as_value("widgets", widget_data);
        child2.append_child(child2_file);
        root.append_child(child1);
        root.append_child(child2);
        root
    }

    #[test]
    fn can_get_elements_from_tree() {
        let tree = build_test_tree();

        let mut path = std::path::Path::new("child1/grandchild/grandchild_file.txt").components();
        let grandchild = tree.get_in(&mut path);
        assert_eq!(
            grandchild.expect("Grandchild not found!").name(),
            "grandchild_file.txt"
        );
    }

    #[test]
    fn cant_get_missing_elements_from_tree() {
        let tree = build_test_tree();
        let mut path = std::path::Path::new("child1/grandchild/nonesuch").components();
        let missing = tree.get_in(&mut path);
        assert_eq!(missing.is_none(), true);

        let mut path2 =
            Path::new("child1/grandchild/grandchild_file.txt/files_have_no_kids").components();
        let missing2 = tree.get_in(&mut path2);
        assert_eq!(missing2.is_none(), true);
    }

    #[test]
    fn can_get_mut_elements_from_tree() {
        let mut tree = build_test_tree();
        let grandchild = tree
            .get_in_mut(&mut Path::new("child1/grandchild/grandchild_file.txt").components())
            .expect("Grandchild not found!");
        assert_eq!(grandchild.name(), "grandchild_file.txt");
        grandchild.name = OsString::from("fish");
        let grandchild2 = tree.get_in_mut(&mut Path::new("child1/grandchild/fish").components());
        assert_eq!(grandchild2.expect("fish not found!").name(), "fish");

        let grandchild_dir = tree
            .get_in_mut(&mut Path::new("child1/grandchild").components())
            .expect("Grandchild dir not found!");
        assert_eq!(grandchild_dir.name(), "grandchild");
        grandchild_dir.append_child(FlareTree::from_file("new_kid_on_the_block.txt"));
        let new_kid = tree
            .get_in_mut(&mut Path::new("child1/grandchild/new_kid_on_the_block.txt").components())
            .expect("New kid not found!");
        assert_eq!(new_kid.name(), "new_kid_on_the_block.txt");
    }

    #[test]
    fn can_get_json_payloads_from_tree() {
        let tree = build_test_tree();
        let file = tree
            .get_in(&mut Path::new("child2/child2_file.txt").components())
            .unwrap();

        assert_eq!(file.name(), "child2_file.txt");

        let expected = json!({
            "sprockets": 7,
            "flanges": ["Nigel, Sarah"]
        });

        assert_eq!(file.data_entry("widgets".to_string()).unwrap(), &expected);
    }

    fn strip(string: &str) -> String {
        let re = Regex::new(r"\s+").unwrap();
        re.replace_all(string, "").to_string()
    }

    #[test]
    fn can_serialize_directory_to_json() {
        let root = FlareTree::from_dir("root");

        let serialized = serde_json::to_string(&root).unwrap();

        assert_eq!(
            serialized,
            strip(
                r#"{
                    "name":"root",
                    "children": []
                }"#
            )
        )
    }
    #[test]
    fn can_serialize_file_to_json() {
        let file = FlareTree::from_file("foo.txt");

        let serialized = serde_json::to_string(&file).unwrap();

        assert_eq!(
            serialized,
            strip(
                r#"{
                    "name":"foo.txt",
                    "data": {}
                }"#
            )
        )
    }

    #[test]
    fn can_serialize_file_with_data_to_json() {
        let mut file = FlareTree::from_file("foo.txt");
        file.add_file_data("wibble", &"fnord".to_string());

        let serialized = serde_json::to_string(&file).unwrap();

        assert_eq!(
            serialized,
            strip(
                r#"{
                    "name":"foo.txt",
                    "data": {"wibble":"fnord"}
                }"#
            )
        )
    }

    #[test]
    fn can_serialize_file_with_data_value_to_json() {
        let mut file = FlareTree::from_file("foo.txt");
        let value = json!({"foo": ["bar", "baz", 123]});
        file.add_file_data_as_value("bat", value);

        let serialized = serde_json::to_string(&file).unwrap();

        assert_eq!(
            serialized,
            strip(
                r#"{
                    "name":"foo.txt",
                    "data": {"bat": {"foo": ["bar", "baz", 123]}}
                }"#
            )
        )
    }

    #[test]
    fn can_serialize_simple_tree_to_json() {
        let mut root = FlareTree::from_dir("root");
        root.append_child(FlareTree::from_file("child.txt"));
        root.append_child(FlareTree::from_dir("child2"));

        let serialized = serde_json::to_string(&root).unwrap();
        let reparsed: Value = serde_json::from_str(&serialized).unwrap();

        assert_eq!(
            reparsed,
            json!({
                "name":"root",
                "children":[
                    {
                        "name": "child.txt",
                        "data": {}
                    },
                    {
                        "name":"child2",
                        "children":[]
                    }
                ]
            })
        )
    }
}
