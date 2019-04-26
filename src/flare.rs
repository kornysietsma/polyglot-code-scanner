#![warn(clippy::all)]
#![allow(dead_code)]

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use std::collections::HashMap;

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
    name: String,
    value: NodeValue,
}

impl FlareTree {
    pub fn name(&self) -> &String {
        &self.name
    }
    pub fn from_file(name: String) -> FlareTree {
        FlareTree {
            name: name,
            value: NodeValue::File {
                data: HashMap::new(),
            },
        }
    }

    pub fn from_dir(name: String) -> FlareTree {
        FlareTree {
            name: name,
            value: NodeValue::Dir {
                children: Vec::new(),
            },
        }
    }

    pub fn add_file_data_as_value(&mut self, key: String, value: serde_json::Value) {
        if let NodeValue::File { ref mut data } = self.value {
            data.insert(key, value);
        }
    }

    pub fn add_file_data<T: Serialize>(&mut self, key: String, value: &T) {
        if let NodeValue::File { ref mut data } = self.value {
            data.insert(key, serde_json::to_value(value).unwrap());
        }
    }

    pub fn append_child(&mut self, child: FlareTree) {
        if let NodeValue::Dir { ref mut children } = self.value {
            children.push(child);
        } // TODO: error handling!
    }

    pub fn get_in(&self, path: &[&str]) -> Option<&FlareTree> {
        let (first_name, remaining_names) = path.split_first()?;

        if let NodeValue::Dir { ref children } = self.value {
            let first_match = children.iter().find(|c| &c.name == first_name);
            let first_match = first_match?;
            if path.len() == 1 {
                return Some(first_match);
            } else {
                return first_match.get_in(remaining_names);
            }
        };
        None
    }

    pub fn get_in_mut(&mut self, path: &[&str]) -> Option<&mut FlareTree> {
        let (first_name, remaining_names) = path.split_first()?;

        if let NodeValue::Dir { ref mut children } = self.value {
            let first_match = children.iter_mut().find(|c| &c.name == first_name);
            let first_match = first_match?;
            if path.len() == 1 {
                return Some(first_match);
            } else {
                return first_match.get_in_mut(remaining_names);
            }
        };
        None
    }
}

impl Serialize for FlareTree {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("FlareTree", 2)?;
        state.serialize_field("name", &self.name)?;
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

    #[test]
    fn can_build_tree() {
        let mut root = FlareTree::from_dir(String::from("root"));
        root.append_child(FlareTree::from_file(String::from("child")));

        assert_eq!(
            root,
            FlareTree {
                name: String::from("root"),
                value: NodeValue::Dir {
                    children: vec![FlareTree {
                        name: String::from("child"),
                        value: NodeValue::File {
                            data: HashMap::new()
                        },
                    }]
                },
            }
        )
    }

    fn build_test_tree() -> FlareTree {
        let mut root = FlareTree::from_dir(String::from("root"));
        root.append_child(FlareTree::from_file(String::from("root_file_1.txt")));
        root.append_child(FlareTree::from_file(String::from("root_file_2.txt")));
        let mut child1 = FlareTree::from_dir(String::from("child1"));
        child1.append_child(FlareTree::from_file(String::from("child1_file_1.txt")));
        let mut grand_child = FlareTree::from_dir(String::from("grandchild"));
        grand_child.append_child(FlareTree::from_file(String::from("grandchild_file.txt")));
        child1.append_child(grand_child);
        child1.append_child(FlareTree::from_file(String::from("child1_file_2.txt")));
        let mut child2 = FlareTree::from_dir(String::from("child2"));
        let mut child2_file = FlareTree::from_file(String::from("child2_file.txt"));
        let widget_data = json!({
            "sprockets": 7,
            "flanges": ["Nigel, Sarah"]
        });
        child2_file.add_file_data_as_value("widgets".to_string(), widget_data);
        child2.append_child(child2_file);
        root.append_child(child1);
        root.append_child(child2);
        root
    }

    #[test]
    fn can_get_elements_from_tree() {
        let tree = build_test_tree();
        let grandchild = tree.get_in(&["child1", "grandchild", "grandchild_file.txt"]);
        assert_eq!(
            grandchild.expect("Grandchild not found!").name(),
            "grandchild_file.txt"
        );
    }

    #[test]
    fn cant_get_missing_elements_from_tree() {
        let tree = build_test_tree();
        let missing = tree.get_in(&["child1", "grandchild", "nonesuch"]);
        assert_eq!(missing.is_none(), true);
        let missing2 = tree.get_in(&[
            "child1",
            "grandchild",
            "grandchild_file.txt",
            "files have no kids",
        ]);
        assert_eq!(missing2.is_none(), true);
        let missing3 = tree.get_in(&[]);
        assert_eq!(missing3.is_none(), true);
    }

    #[test]
    fn can_get_mut_elements_from_tree() {
        let mut tree = build_test_tree();
        let grandchild = tree
            .get_in_mut(&["child1", "grandchild", "grandchild_file.txt"])
            .expect("Grandchild not found!");
        assert_eq!(grandchild.name(), "grandchild_file.txt");
        grandchild.name = String::from("fish");
        let grandchild2 = tree.get_in_mut(&["child1", "grandchild", "fish"]);
        assert_eq!(grandchild2.expect("fish not found!").name(), "fish");

        let grandchild_dir = tree
            .get_in_mut(&["child1", "grandchild"])
            .expect("Grandchild dir not found!");
        assert_eq!(grandchild_dir.name(), "grandchild");
        grandchild_dir.append_child(FlareTree::from_file(String::from(
            "new_kid_on_the_block.txt",
        )));
        let new_kid = tree
            .get_in_mut(&["child1", "grandchild", "new_kid_on_the_block.txt"])
            .expect("New kid not found!");
        assert_eq!(new_kid.name(), "new_kid_on_the_block.txt");
    }

    fn strip(string: &str) -> String {
        let re = Regex::new(r"\s+").unwrap();
        re.replace_all(string, "").to_string()
    }

    #[test]
    fn can_serialize_directory_to_json() {
        let root = FlareTree::from_dir(String::from("root"));

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
        let file = FlareTree::from_file(String::from("foo.txt"));

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
        let mut file = FlareTree::from_file(String::from("foo.txt"));
        file.add_file_data("wibble".to_string(), &"fnord".to_string());

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
        let mut file = FlareTree::from_file(String::from("foo.txt"));
        let value = json!({"foo": ["bar", "baz", 123]});
        file.add_file_data("bat".to_string(), &value);

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
        let mut root = FlareTree::from_dir(String::from("root"));
        root.append_child(FlareTree::from_file(String::from("child.txt")));
        root.append_child(FlareTree::from_dir(String::from("child2")));

        let serialized = serde_json::to_string(&root).unwrap();

        assert_eq!(
            serialized,
            strip(
                r#"{
                    "name":"root",
                    "children": [
                        {
                            "name": "child.txt",
                            "data": {}
                        },
                        {
                            "name": "child2",
                            "children": []
                        }
                    ]
                }"#
            )
        );
        // duplicate of above, but to show using Values not Strings:
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
