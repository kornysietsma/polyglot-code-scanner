#![warn(clippy::all)]
use crate::git_logger::User;
use serde::ser::SerializeSeq;
use serde::{Serialize, Serializer};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GitUserDedupMode {
    #[default]
    Legacy,
    Email,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum UserIdentityKey {
    EmailLower(String),
    LegacyLowerUser(User),
}

#[derive(Debug, Clone, Default)]
pub struct GitUserDictionary {
    next_id: usize,
    identity_to_id: HashMap<UserIdentityKey, usize>,
    users: Vec<User>,
    last_name_update_by_id: Vec<u64>,
    dedup_mode: GitUserDedupMode,
}

impl GitUserDictionary {
    pub fn new(dedup_mode: GitUserDedupMode) -> Self {
        Self {
            dedup_mode,
            ..Self::default()
        }
    }

    fn identity_key(&self, user: &User) -> UserIdentityKey {
        if self.dedup_mode == GitUserDedupMode::Email {
            if let Some(normalized_email) = user.normalized_email_if_valid() {
                return UserIdentityKey::EmailLower(normalized_email);
            }
        }
        UserIdentityKey::LegacyLowerUser(user.as_lower_case())
    }

    pub fn register(&mut self, user: &User, observed_at: u64) -> usize {
        let identity_key = self.identity_key(user);
        match self.identity_to_id.get(&identity_key) {
            Some(id) => *id,
            None => {
                let result = self.next_id;
                self.identity_to_id.insert(identity_key, result);
                self.users.push(user.clone());
                self.last_name_update_by_id.push(observed_at);
                self.next_id += 1;
                result
            }
        }
    }

    pub fn register_with_name_updates(&mut self, user: &User, observed_at: u64) -> usize {
        let identity_key = self.identity_key(user);
        if let Some(id) = self.identity_to_id.get(&identity_key).copied() {
            if let UserIdentityKey::EmailLower(_) = identity_key {
                let last_update = self
                    .last_name_update_by_id
                    .get(id)
                    .copied()
                    .unwrap_or_default();
                if user.has_non_blank_name() && observed_at >= last_update {
                    if let Some(existing_user) = self.users.get_mut(id) {
                        *existing_user = existing_user.with_name_from(user);
                    }
                    if let Some(last_name_update) = self.last_name_update_by_id.get_mut(id) {
                        *last_name_update = observed_at;
                    }
                }
            }
            return id;
        }

        self.register(user, observed_at)
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
        self.identity_to_id
            .get(&UserIdentityKey::LegacyLowerUser(user.as_lower_case()))
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
        let mut dict = GitUserDictionary::default();

        let jane = User::new(Some("Jane"), Some("JaneDoe@gmail.com"));
        let user0 = dict.register(&jane, 1);
        assert_eq!(user0, 0);
        assert_eq!(dict.user_by_id(user0), jane);

        let user1 = dict.register(&User::new(Some("Jane"), None), 2);
        assert_eq!(user1, 1);
        let user0again = dict.register(&User::new(Some("Jane"), Some("JaneDoe@gmail.com")), 3);
        assert_eq!(user0again, 0);
    }

    #[test]
    fn user_checks_are_case_insensitive_and_return_first_seen_user() {
        let mut dict = GitUserDictionary::default();

        let jane = User::new(Some("Jane"), Some("JaneDoe@gmail.com"));
        let lower_jane = User::new(Some("jane"), Some("janeDoe@gmail.com"));
        let user0 = dict.register(&jane, 1);
        assert_eq!(user0, 0);
        // there is only one user!
        assert_eq!(dict.user_count(), 1);

        let user1 = dict.register(&lower_jane, 2);
        assert_eq!(user1, 0);
        assert_eq!(dict.user_by_id(0), jane);
    }

    #[test]
    fn email_mode_collapses_same_email_and_keeps_latest_non_blank_name() {
        let mut dict = GitUserDictionary::new(GitUserDedupMode::Email);

        let old = User::new(Some("Old Name"), Some("JaneDoe@gmail.com"));
        let newer = User::new(Some("New Name"), Some("janedoe@GMAIL.com"));

        let id0 = dict.register_with_name_updates(&old, 10);
        let id1 = dict.register_with_name_updates(&newer, 20);

        assert_eq!(id0, id1);
        assert_eq!(dict.user_count(), 1);
        assert_eq!(
            dict.user_by_id(id0),
            User::new(Some("New Name"), Some("JaneDoe@gmail.com"))
        );
    }

    #[test]
    fn email_mode_blank_or_invalid_email_uses_legacy_identity() {
        let mut dict = GitUserDictionary::new(GitUserDedupMode::Email);

        let invalid_1 = User::new(Some("Jane"), Some("not-an-email"));
        let invalid_2 = User::new(Some("JANE"), Some("NOT-AN-EMAIL"));
        let missing = User::new(Some("Jane"), None);

        let id0 = dict.register_with_name_updates(&invalid_1, 1);
        let id1 = dict.register_with_name_updates(&invalid_2, 2);
        let id2 = dict.register_with_name_updates(&missing, 3);

        assert_eq!(id0, id1);
        assert_ne!(id0, id2);
    }
}
