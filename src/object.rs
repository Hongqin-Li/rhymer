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
    pub fn from(ctx: Arc<Context>, user: UserKind) -> Self {
        Object {
            ctx,
            user,
            id: None,
            class: "".to_string(),
            doc: Document::default(),
            acl: Acl::default(),
        }
    }

    pub fn class(&mut self, name: String) {
        self.class = name
    }
    pub fn get_acl(&self) -> Acl {
        self.acl.clone()
    }

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
                .create(&self.class, self.doc.clone(), crate::user::UserKind::Master)
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
    req: Request,
    ctx: Arc<Context>,
    class: ClassName,
) -> Result<impl Reply, Rejection> {
    if let Some(body) = req.body {
        ctx.db
            .create(class.get_name(), body, req.user)
            .await
            .and_then(|d| {
                serde_json::to_string(&d)
                    .map_or_else(|_e| internal_server_error("Serialization error"), |s| Ok(s))
            })
    } else {
        bad_request("Body not found")
    }
}

pub async fn retrieve_by_filter(
    req: Request,
    ctx: Arc<Context>,
    class: ClassName,
    filter: Document,
) -> Result<impl Reply, Rejection> {
    let result = ctx.db.retrieve(class.get_name(), filter, req.user).await;
    result.and_then(|v| {
        serde_json::to_string(&v)
            .map_or_else(|_e| internal_server_error("Serialization error"), |s| Ok(s))
    })
}

pub async fn retrieve(
    req: Request,
    ctx: Arc<Context>,
    class: ClassName,
    id: String,
) -> Result<impl Reply, Rejection> {
    let filter = doc! {database::OBJECT_ID: id };
    let result = ctx.db.retrieve(class.get_name(), filter, req.user).await;
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
    req: Request,
    ctx: Arc<Context>,
    class: ClassName,
    id: String,
) -> Result<impl Reply, Rejection> {
    if let Some(body) = req.body {
        ctx.db
            .update(class.get_name(), &id, body, req.user)
            .await
            .and_then(|d| {
                serde_json::to_string(&d)
                    .map_or_else(|_e| internal_server_error("Serialization error"), |s| Ok(s))
            })
    } else {
        bad_request("Body not found")
    }
}

pub async fn delete(
    req: Request,
    ctx: Arc<Context>,
    class: ClassName,
    id: String,
) -> Result<impl Reply, Rejection> {
    ctx.db
        .delete(class.get_name(), &id, req.user)
        .await
        .and_then(|d| {
            serde_json::to_string(&d)
                .map_or_else(|_e| internal_server_error("Serialization error"), |s| Ok(s))
        })
}
