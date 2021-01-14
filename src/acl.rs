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

/// Access control of each object.
#[derive(Debug, Default, Clone)]
pub struct Acl {
    /// Per-user access control, key is user_id
    pub user: HashMap<String, AclKind>,
    /// Access control on other users
    pub other: AclKind,
}

impl Acl {
    /// Create a new access control flag, default to read-write by all users.
    pub fn new() -> Self {
        Self::default()
    }

    // pub fn from_readers(user_ids: Vec<String>) -> Self {
    //     let mut x = Self::new();
    //     for i in user_ids {
    //         x.add_reader(i);
    //     }
    //     x
    // }
    // pub fn from_writers(user_ids: Vec<String>) -> Self {
    //     let mut x = Self::new();
    //     for i in user_ids {
    //         x.add_writer(i);
    //     }
    //     x
    // }
    // pub fn add_reader(&mut self, uid: String) {
    //     self.user.insert(uid, AclKind::ReadOnly);
    // }
    // pub fn add_writer(&mut self, uid: String) {
    //     self.user.insert(uid, AclKind::ReadWrite);
    // }

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
    /// Check if writable by user with user_id
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
    /// Set read-only by user.
    pub fn set_readonly(&mut self, user_id: impl Into<String>) {
        self.user.insert(user_id.into(), AclKind::ReadOnly);
    }
    /// Set invisiable by user.
    pub fn set_invisiable(&mut self, user_id: impl Into<String>) {
        self.user.insert(user_id.into(), AclKind::Invisible);
    }
    /// Set writable by user.
    pub fn set_writable(&mut self, user_id: impl Into<String>) {
        self.user.insert(user_id.into(), AclKind::ReadWrite);
    }

    /// Set writable by other users.
    pub fn set_public_readonly(&mut self) {
        self.other = AclKind::ReadOnly;
    }

    /// Set invisiable by other users.
    pub fn set_public_invisiable(&mut self) {
        self.other = AclKind::Invisible;
    }

    /// Set writable by other users.
    pub fn set_public_writable(&mut self) {
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
        d
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;
    use warp::{hyper::StatusCode, Rejection};

    use crate::{
        server::{Context, Request},
        tests::test_server,
        user::UserKind,
        with_user,
    };

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

    const ACL_TEST_USERS: &[&'static str] = &["a", "b"];

    async fn save_acl(
        req: Request,
        ctx: Arc<Context>,
        arg: HashMap<String, String>,
    ) -> Result<String, Rejection> {
        let mut obj = ctx.object("test");
        let mut acl = Acl::new();

        let public_acc = arg.get("public").unwrap();
        if public_acc == "0" {
            acl.set_public_invisiable();
        } else if public_acc == "1" {
            acl.set_public_readonly();
        } else if public_acc == "2" {
            acl.set_public_writable();
        } else {
            panic!("unexpected acl code {}", public_acc);
        }

        for uid in ACL_TEST_USERS.iter() {
            let uid = uid.to_string();
            let acc = arg.get(&uid).unwrap();
            if acc == "0" {
                acl.set_invisiable(&uid);
            } else if acc == "1" {
                acl.set_readonly(&uid);
            } else if acc == "2" {
                acl.set_writable(&uid);
            } else {
                panic!("unexpected acl code {}", public_acc);
            }
        }

        let data = doc! { "name": "a"};
        obj.set_acl(acl);
        obj.set_doc(data);
        let r = obj.save().await?;
        Ok(r.get_str("objectId").unwrap().to_string())
    }

    #[tokio::test]
    async fn test_acl() {
        let mut s = test_server().await;
        s.define(
            "testf",
            Box::new(|req, ctx, arg| Box::pin(save_acl(req, ctx, arg))),
        );

        let invoke1 = async move |api, name, query| {
            warp::test::request()
                .method("GET")
                .path(&format!("/functions/{}?{}", &name, &query))
                .reply(api)
                .await
        };

        let retrieve1 = async move |api, uid, class, id| {
            with_user!(uid, "GET")
                .path(&format!("/classes/{}/{}", class, id))
                .reply(api)
                .await
        };
        let retrieve1_public = async move |api, class, id| {
            warp::test::request()
                .method("GET")
                .path(&format!("/classes/{}/{}", class, id))
                .reply(api)
                .await
        };
        let update1 = async move |api, uid, class, id, body| {
            with_user!(uid, "PUT")
                .path(&format!("/classes/{}/{}", class, id))
                .json(&body)
                .reply(api)
                .await
        };
        let update1_public = async move |api, class, id, body| {
            warp::test::request()
                .method("PUT")
                .path(&format!("/classes/{}/{}", class, id))
                .json(&body)
                .reply(api)
                .await
        };

        let delete1 = async move |api, uid, class, id| {
            with_user!(uid, "DELETE")
                .path(&format!("/classes/{}/{}", class, id))
                .reply(api)
                .await
        };

        let delete1_public = async move |api, class, id| {
            warp::test::request()
                .method("DELETE")
                .path(&format!("/classes/{}/{}", class, id))
                .reply(api)
                .await
        };

        let api = s.routes().await;

        let max_code = 3u64.pow((ACL_TEST_USERS.len() + 1) as u32);
        for i in 0..max_code {
            let mut j = i;
            let public_code = j % 3;
            let mut query_str = format!("public={}", public_code);
            j /= 3;

            let mut user_code = vec![];
            for uid in ACL_TEST_USERS {
                let code = j % 3;
                user_code.push(code);
                query_str += &format!("&{}={}", &uid, &code);
                j /= 3;
            }
            let resp = invoke1(&api, "testf", query_str.clone()).await;
            assert_eq!(resp.status(), StatusCode::OK);
            let oid = String::from_utf8_lossy(&resp.body()[..]).to_string();

            // Public guest.
            let r = retrieve1_public(&api, "test", oid.clone()).await;
            let u = update1_public(&api, "test", oid.clone(), json!({"name": "whatever"})).await;
            if public_code == 0 {
                assert_eq!(r.status(), StatusCode::NOT_FOUND);
                assert_eq!(u.status(), StatusCode::NOT_FOUND);
            } else if public_code == 1 {
                assert_eq!(r.status(), StatusCode::OK);
                assert_eq!(u.status(), StatusCode::NOT_FOUND);
            } else {
                assert_eq!(r.status(), StatusCode::OK);
                assert_eq!(u.status(), StatusCode::OK);
            }

            // Client.
            for (i, code) in user_code.iter().enumerate() {
                let uid = ACL_TEST_USERS[i];
                let code = code.clone();
                let r = retrieve1(&api, uid, "test", oid.clone()).await;
                let u = update1(&api, uid, "test", oid.clone(), json!({"name": "whatever"})).await;

                if code == 0 {
                    assert_eq!(r.status(), StatusCode::NOT_FOUND);
                    assert_eq!(u.status(), StatusCode::NOT_FOUND);
                } else if code == 1 {
                    assert_eq!(r.status(), StatusCode::OK);
                    assert_eq!(u.status(), StatusCode::NOT_FOUND);
                } else {
                    assert_eq!(r.status(), StatusCode::OK);
                    assert_eq!(u.status(), StatusCode::OK);
                }
            }

            // Test deletion.
            let d = delete1_public(&api, "test", oid.clone()).await;
            if public_code == 2 {
                assert_eq!(d.status(), StatusCode::OK);
                continue;
            } else {
                assert_eq!(d.status(), StatusCode::NOT_FOUND);
            }
            for (i, code) in user_code.iter().enumerate() {
                let uid = ACL_TEST_USERS[i];
                let code = code.clone();
                let d = delete1(&api, uid, "test", oid.clone()).await;
                if code == 2 {
                    assert_eq!(d.status(), StatusCode::OK);
                    break;
                } else {
                    assert_eq!(d.status(), StatusCode::NOT_FOUND);
                }
            }
        }
    }
}
