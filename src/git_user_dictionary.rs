#![warn(clippy::all)]
use crate::git_logger::User;
use serde::ser::SerializeSeq;
use serde::{Serialize, Serializer};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct GitUserDictionary {
    next_id: usize,
    users: HashMap<User, usize>,
}

impl GitUserDictionary {
    pub fn new() -> Self {
        GitUserDictionary {
            next_id: 0,
            users: HashMap::new(),
        }
    }
    pub fn register(&mut self, user: &User) -> usize {
        match self.users.get(user) {
            Some(id) => *id,
            None => {
                let result = self.next_id;
                self.users.insert(user.clone(), result);
                self.next_id += 1;
                result
            }
        }
    }
    #[allow(dead_code)]
    pub fn user_count(&self) -> usize {
        self.next_id
    }
    #[allow(dead_code)]
    pub fn user_id(&self, user: &User) -> Option<&usize> {
        self.users.get(user)
    }
}

/// We store, rather redundantly, the user ID in the JSON, even though users are output as an array.
/// This makes it easier for humans to correlate users with data without counting from 0
/// It also will make it easier later to alias users to other users.
#[derive(Debug, PartialEq, Serialize)]
struct UserKey<'a> {
    id: usize,
    user: &'a User,
}

impl Serialize for GitUserDictionary {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut reverse_index: HashMap<usize, &User> = HashMap::new();
        for (user, id) in self.users.iter() {
            reverse_index.insert(*id, user);
        }
        let mut seq = serializer.serialize_seq(Some(self.next_id))?;
        for id in 0..self.next_id {
            let user = reverse_index.get(&id).unwrap();
            seq.serialize_element(&UserKey { id, user })?;
        }
        seq.end()
    }
}
