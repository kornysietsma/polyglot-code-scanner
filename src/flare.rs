#[derive(PartialEq, PartialOrd, Debug)]
pub enum NodeValue {
    Dir { children: Vec<FlareNode> },
    File {},
}

#[derive(PartialEq, PartialOrd, Debug)]
pub struct FlareNode {
    name: String,
    value: NodeValue,
}

impl FlareNode {
    pub fn name(&self) -> &String {
        &self.name
    }
    pub fn from_file(name: String) -> FlareNode {
        FlareNode {
            name: name,
            value: NodeValue::File {},
        }
    }
    pub fn from_dir(name: String) -> FlareNode {
        FlareNode {
            name: name,
            value: NodeValue::Dir {
                children: Vec::new(),
            },
        }
    }
    pub fn append_child(&mut self, child:FlareNode) {
        match self.value {
            NodeValue::Dir { ref mut children } => {
                children.push(child);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn can_build_tree() {
        let mut root = FlareNode::from_dir(String::from("root"));
        root.append_child(FlareNode::from_file(String::from("child")));

        assert_eq!(root, FlareNode {
                name: String::from("root"),
                value: NodeValue::Dir {
                    children: vec![FlareNode {
                        name: String::from("child"),
                        value: NodeValue::File {},
                    }]
                },
            })
    }
}
