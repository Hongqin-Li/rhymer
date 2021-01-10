use crate::{
    acl::Acl,
    database::{self, Database as _},
    error::{self, internal_server_error},
    server::{Context, Request},
    user::UserKind,
    validator::ClassName,
};
use error::{bad_request, not_found};
use mongodb::bson::{doc, Document};
use std::{result::Result, sync::Arc};
use warp::{Rejection, Reply};

pub struct Object {
    class: String,
    id: Option<String>,
    doc: Document,
    acl: Acl,
    ctx: Arc<Context>,
    user: UserKind,
}

impl Object {
    pub(crate) fn from(ctx: Arc<Context>, user: UserKind) -> Self {
        Object {
            ctx,
            user,
            id: None,
            class: "".to_string(),
            doc: Document::default(),
            acl: Acl::default(),
        }
    }

    pub fn set_class(&mut self, name: impl Into<String>) {
        self.class = name.into()
    }
    pub fn set_id(&mut self, id: impl Into<String>) {
        self.id = Some(id.into());
    }
    pub fn set_doc(&mut self, doc: Document) {
        self.doc = doc;
    }

    pub fn get_acl(&self) -> Acl {
        self.acl.clone()
    }

    /// Update if id is provided, else create a new one.
    pub async fn save(&mut self) -> Result<Document, Rejection> {
        if let Some(id) = self.id.clone() {
            (*self.ctx)
                .db
                .update(&self.class, &id, self.doc.clone(), self.user.clone())
                .await
        // Database guarantees the invariance of id, thus no need to update self.id
        } else {
            (*self.ctx)
                .db
                .create(&self.class, self.doc.clone(), self.user.clone())
                .await
                .map(|d| {
                    let id = d
                        .get(database::OBJECT_ID)
                        .expect("create should return objectId");
                    self.id = Some(id.to_string());
                    d
                })
        }
    }

    pub async fn destroy(&mut self) -> Result<Document, Rejection> {
        if let Some(id) = self.id.clone() {
            (*self.ctx)
                .db
                .delete(&self.class, &id, self.user.clone())
                .await
                .map(|d| {
                    self.id = None;
                    d
                })
        } else {
            not_found("Destroy without ID.")
        }
    }
}

pub async fn create(
    class: ClassName,
    mut req: Request,
    ctx: Arc<Context>,
) -> Result<impl Reply, Rejection> {
    let class = class.as_str();
    if let Some(f) = ctx.before_save.get(class) {
        trace!("before save(create): {}", class);
        f(&mut req, ctx.clone()).await?;
    };
    if let Some(ref body) = req.body {
        let mut obj = Object::from(ctx.clone(), req.user.clone());
        obj.set_class(class);
        obj.set_doc(body.clone()); // FIXME: maybe after hook do not need body?

        let result = obj.save().await.and_then(|d| {
            serde_json::to_string(&d)
                .map_or_else(|_e| internal_server_error("Serialization error"), |s| Ok(s))
        })?;
        if let Some(f) = ctx.after_save.get(class) {
            trace!("after save(create): {}", class);
            f(&mut req, ctx.clone()).await?;
        };
        Ok(warp::reply::with_status(
            result,
            warp::http::StatusCode::CREATED,
        ))
    } else {
        bad_request("Body not found")
    }
}

pub async fn retrieve_by_filter(
    class: ClassName,
    filter: Document,
    req: Request,
    ctx: Arc<Context>,
) -> Result<impl Reply, Rejection> {
    let result = ctx.db.retrieve(class.as_str(), filter, req.user).await;
    result.and_then(|v| {
        serde_json::to_string(&v)
            .map_or_else(|_e| internal_server_error("Serialization error"), |s| Ok(s))
    })
}

pub async fn retrieve(
    class: ClassName,
    id: String,
    req: Request,
    ctx: Arc<Context>,
) -> Result<impl Reply, Rejection> {
    let filter = doc! {database::OBJECT_ID: id };
    let result = ctx.db.retrieve(class.as_str(), filter, req.user).await;
    result.map_or_else(
        |e| Err(e),
        |v| {
            if let Some(v) = v.first() {
                serde_json::to_string(v)
                    .map_or_else(|_e| internal_server_error("Serialization error"), |s| Ok(s))
            } else if v.len() == 0 {
                not_found("")
            } else {
                internal_server_error("Id not unique")
            }
        },
    )
}

pub async fn update(
    class: ClassName,
    id: String,
    mut req: Request,
    ctx: Arc<Context>,
) -> Result<impl Reply, Rejection> {
    let class = class.as_str();
    if let Some(f) = ctx.before_save.get(class) {
        trace!("before save(update): {}", class);
        f(&mut req, ctx.clone()).await?;
    };
    if let Some(ref body) = req.body {
        let mut obj = Object::from(ctx.clone(), req.user.clone());
        obj.set_class(class);
        obj.set_id(id);
        obj.set_doc(body.clone());

        let result = obj.save().await.and_then(|d| {
            serde_json::to_string(&d)
                .map_or_else(|_e| internal_server_error("Serialization error"), |s| Ok(s))
        })?;

        if let Some(f) = ctx.after_save.get(class) {
            trace!("after save(update): {}", class);
            f(&mut req, ctx.clone()).await?;
        };
        Ok(result)
    } else {
        bad_request("Body not found")
    }
}

pub async fn delete(
    class: ClassName,
    id: String,
    mut req: Request,
    ctx: Arc<Context>,
) -> Result<impl Reply, Rejection> {
    let class = class.as_str();
    if let Some(f) = ctx.before_destroy.get(class) {
        trace!("before destroy: {}", class);
        f(&mut req, ctx.clone()).await?;
    };

    let mut obj = Object::from(ctx.clone(), req.user.clone());
    obj.set_class(class);
    obj.set_id(id);
    let result = obj.destroy().await.and_then(|d| {
        serde_json::to_string(&d)
            .map_or_else(|_e| internal_server_error("Serialization error"), |s| Ok(s))
    })?;

    if let Some(f) = ctx.after_destroy.get(class) {
        trace!("after destroy: {}", class);
        f(&mut req, ctx.clone()).await?;
    };
    Ok(result)
}

#[cfg(test)]
mod test {
    use std::{collections::HashMap, convert::TryFrom, sync::Arc};

    use mongodb::bson::Document;
    use serde_json::{json, Map, Value};
    use warp::hyper::StatusCode;

    use super::super::tests::test_api;

    #[tokio::test]
    async fn test_create() {
        let api = test_api().await;

        let create1 = async move |api, class, body| {
            warp::test::request()
                .method("POST")
                .path(&format!("/classes/{}", class))
                .json(&body)
                .reply(api)
                .await
        };
        // Test class name validation.
        let resp = create1(&api, "_foo", json!({"a": "1"})).await;
        debug!("resp: {:?}", resp);
        assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);

        // Test create object data.
        let resp = create1(
            &api,
            "foo",
            json!({
                "test-string": "a",
                "test-int": 1,
                "test-bool": true,
                "test-array": [-1, 1000000000000 as i64, "b", false],
            }),
        )
        .await;
        debug!("resp: {:?}", resp);

        let body = String::from_utf8(resp.body()[..].into()).unwrap();
        debug!("resp.body(): {:?}", body);
        let value: Map<String, Value> = serde_json::from_str(&body).unwrap();
        let body = Document::try_from(value).unwrap();

        assert_eq!(resp.status(), StatusCode::CREATED);
        assert!(body.get("objectId").is_some());
        assert!(body.get("createdAt").is_some());
        assert!(body.get("updatedAt").is_some());
        assert!(body.get_i64("test-int").is_err());
        assert_eq!(body.get_i32("test-int").unwrap(), 1);
        assert_eq!(body.get_str("test-string").unwrap(), "a");
        assert_eq!(body.get_bool("test-bool").unwrap(), true);
        let a = body.get_array("test-array").unwrap();
        assert_eq!(a.get(0).unwrap().as_i32().unwrap(), -1);
        assert_eq!(a.get(1).unwrap().as_i64().unwrap(), 1000000000000);
        assert_eq!(a.get(2).unwrap().as_str().unwrap(), "b");
        assert_eq!(a.get(3).unwrap().as_bool().unwrap(), false);



    }

    #[tokio::test]
    async fn test_acl() {
    }

}
