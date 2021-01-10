use std::collections::HashMap;

use mongodb::bson::{doc, Document};

use crate::database;

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum AclKind {
    Invisible,
    ReadOnly,
    ReadWrite,
}

impl Default for AclKind {
    fn default() -> Self {
        AclKind::ReadWrite
    }
}

#[derive(Debug, Default)]
struct AclItem {
    pub uid: String,
    pub acl: AclKind,
}

#[derive(Debug, Default, Clone)]
pub struct Acl {
    /// Per-user access control, key is user_id
    pub user: HashMap<String, AclKind>,
    /// Access control on other users
    pub other: AclKind,
}

impl Acl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_readers(user_ids: Vec<String>) -> Self {
        let mut x = Self::new();
        for i in user_ids {
            x.add_reader(i);
        }
        x
    }
    pub fn from_writers(user_ids: Vec<String>) -> Self {
        let mut x = Self::new();
        for i in user_ids {
            x.add_writer(i);
        }
        x
    }
    pub fn add_reader(&mut self, uid: String) {
        self.user.insert(uid, AclKind::ReadOnly);
    }
    pub fn add_writer(&mut self, uid: String) {
        self.user.insert(uid, AclKind::ReadWrite);
    }

    /// Check if readable by user with user_id
    pub fn readable_by_user(&self, user_id: &str) -> bool {
        if let Some(k) = self.user.get(user_id) {
            match k {
                AclKind::Invisible => false,
                _ => true,
            }
        } else {
            self.readable_by_public()
        }
    }
    /// Check  if writable by user with user_id
    pub fn writable_by_user(&self, user_id: &str) -> bool {
        if let Some(k) = self.user.get(user_id) {
            match k {
                AclKind::ReadWrite => true,
                _ => false,
            }
        } else {
            self.writable_by_public()
        }
    }
    /// Check if readable by public (users not specified in ACL)
    pub fn readable_by_public(&self) -> bool {
        match self.other {
            AclKind::Invisible => false,
            _ => true,
        }
    }

    /// Check if writable by public (users not specified in ACL)
    pub fn writable_by_public(&self) -> bool {
        match self.other {
            AclKind::ReadWrite => true,
            _ => false,
        }
    }
    pub fn set_readonly(&mut self, user_id: impl Into<String>) {
        self.user.insert(user_id.into(), AclKind::ReadOnly);
    }
    pub fn set_invisiable(&mut self, user_id: impl Into<String>) {
        self.user.insert(user_id.into(), AclKind::Invisible);
    }
    pub fn set_writable(&mut self, user_id: impl Into<String>) {
        self.user.insert(user_id.into(), AclKind::ReadWrite);
    }
    pub fn set_public_readonly(&mut self) {
        self.other = AclKind::ReadOnly;
    }
    pub fn set_public_invisiable(&mut self) {
        self.other = AclKind::Invisible;
    }
    pub fn set_public_writable(&mut self, user_id: impl Into<String>) {
        self.other = AclKind::ReadWrite;
    }
}

impl Into<Document> for Acl {
    fn into(self) -> Document {
        let mut acl = self.user;
        acl.insert("*".to_string(), self.other);

        let mut d = Document::new();
        for (uid, acl) in acl.iter() {
            d.insert(
                uid,
                match acl {
                    AclKind::Invisible => "i",
                    AclKind::ReadOnly => "r",
                    AclKind::ReadWrite => "w",
                },
            );
        }
        doc! {database::ACL: d}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_acl_new() {
        let acl = Acl::new();
        assert!(acl.other == AclKind::ReadWrite);
        assert!(acl.user.len() == 0);
        assert!(acl.readable_by_public() && acl.writable_by_public());
        assert!(acl.readable_by_user("foo") && acl.writable_by_user("bar"));
    }

    #[test]
    fn test_setting_acl() {
        let mut acl = Acl::new();
        let uid = "foo";
        acl.set_readonly(uid);
        assert!(acl.readable_by_user(uid) && !acl.writable_by_user(uid));
        acl.set_invisiable(uid);
        assert!(!acl.readable_by_user(uid) && !acl.writable_by_user(uid));
        acl.set_writable(uid);
        assert!(acl.readable_by_user(uid) && acl.writable_by_user(uid));
    }
}
