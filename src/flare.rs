#[derive(PartialEq, PartialOrd, Debug)]
pub enum FileOrDir {
    FlareDir { children: Vec<FlareNode> },
    FlareFile {},
}

#[derive(PartialEq, PartialOrd, Debug)]
pub struct FlareNode {
    name: String,
    value: FileOrDir,
}

impl FlareNode {
    fn from_file(name: String) -> FlareNode {
        FlareNode {
            name: name,
            value: FileOrDir::FlareFile {},
        }
    }
    fn from_dir(name: String) -> FlareNode {
        FlareNode {
            name: name,
            value: FileOrDir::FlareDir {
                children: Vec::new(),
            },
        }
    }
    fn append_child(&mut self, child:FlareNode) {
        match self.value {
            FileOrDir::FlareDir { ref mut children } => {
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
                value: FileOrDir::FlareDir {
                    children: vec![FlareNode {
                        name: String::from("child"),
                        value: FileOrDir::FlareFile {},
                    }]
                },
            })
    }
}
