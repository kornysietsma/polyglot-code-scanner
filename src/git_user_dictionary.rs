#![warn(clippy::all)]
use crate::git_logger::User;
use serde::ser::SerializeSeq;
use serde::{Serialize, Serializer};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct GitUserDictionary {
    next_id: usize,
    lower_users: HashMap<User, usize>,
    users: Vec<User>,
}

impl GitUserDictionary {
    pub fn new() -> Self {
        GitUserDictionary {
            next_id: 0,
            lower_users: HashMap::new(),
            users: Vec::new(),
        }
    }
    pub fn register(&mut self, user: &User) -> usize {
        let lower_user = user.as_lower_case();
        match self.lower_users.get(&lower_user) {
            Some(id) => *id,
            None => {
                let result = self.next_id;
                self.lower_users.insert(lower_user, result);
                self.users.push(user.clone());
                self.next_id += 1;
                result
            }
        }
    }
    #[cfg(test)]
    pub fn user_by_id(&self, user_id: usize) -> User {
        self.users
            .get(user_id)
            .expect("No user found matching ID!")
            .clone()
    }
    #[cfg(test)]
    pub fn user_count(&self) -> usize {
        self.next_id
    }
    #[cfg(test)]
    pub fn user_id(&self, user: &User) -> Option<&usize> {
        self.lower_users.get(&user.as_lower_case())
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
        let mut seq = serializer.serialize_seq(Some(self.users.len()))?;
        for (id, user) in self.users.iter().enumerate() {
            seq.serialize_element(&UserKey { id, user })?;
        }
        seq.end()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[cfg(test)]
    use pretty_assertions::assert_eq;

    // use test_shared::*;

    #[test]
    fn users_receive_sequential_ids() {
        let mut dict = GitUserDictionary::new();

        let jane = User::new(Some("Jane"), Some("JaneDoe@gmail.com"));
        let user0 = dict.register(&jane);
        assert_eq!(user0, 0);
        assert_eq!(dict.user_by_id(user0), jane);

        let user1 = dict.register(&User::new(Some("Jane"), None));
        assert_eq!(user1, 1);
        let user0again = dict.register(&User::new(Some("Jane"), Some("JaneDoe@gmail.com")));
        assert_eq!(user0again, 0);
    }

    #[test]
    fn user_checks_are_case_insensitive_and_return_first_seen_user() {
        let mut dict = GitUserDictionary::new();

        let jane = User::new(Some("Jane"), Some("JaneDoe@gmail.com"));
        let lower_jane = User::new(Some("jane"), Some("janeDoe@gmail.com"));
        let user0 = dict.register(&jane);
        assert_eq!(user0, 0);
        // there is only one user!
        assert_eq!(dict.user_count(), 1);

        let user1 = dict.register(&lower_jane);
        assert_eq!(user1, 0);
        assert_eq!(dict.user_by_id(0), jane)
    }
}
