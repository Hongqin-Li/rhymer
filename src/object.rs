use crate::{
    acl::Acl,
    database::{self, Database as _, Mongodb as Database},
    error::{self, internal_server_error},
    rhymer::{Context, Request},
    user::UserKind,
    validator::ClassName,
};
use error::not_found;
use mongodb::bson::{doc, Document};
use std::{result::Result, sync::Arc};
use tokio::stream::StreamExt;
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
    class: ClassName,
    doc: Document,
    req: Request,
    ctx: Arc<Context>,
) -> Result<impl Reply, Rejection> {
    ctx.db
        .create(class.get_name(), doc, req.user)
        .await
        .and_then(|d| {
            serde_json::to_string(&d).map_or_else(|e| internal_server_error(""), |s| Ok(s))
        })
}

pub async fn retrieve_by_filter(
    class: ClassName,
    filter: Document,
    req: Request,
    ctx: Arc<Context>,
) -> Result<impl Reply, Rejection> {
    let result = ctx.db.retrieve(class.get_name(), filter, req.user).await;
    result.and_then(|v| {
        serde_json::to_string(&v).map_or_else(|e| internal_server_error(""), |s| Ok(s))
    })
}

pub async fn retrieve(
    class: ClassName,
    id: String,
    req: Request,
    ctx: Arc<Context>,
) -> Result<impl Reply, Rejection> {
    let filter = doc! {database::OBJECT_ID: id };
    let result = ctx.db.retrieve(class.get_name(), filter, req.user).await;
    result.map_or_else(
        |e| Err(e),
        |v| {
            if let Some(v) = v.first() {
                serde_json::to_string(v).map_or_else(|e| internal_server_error(""), |s| Ok(s))
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
    doc: Document,
    req: Request,
    ctx: Arc<Context>,
) -> Result<impl Reply, Rejection> {
    ctx.db
        .update(class.get_name(), &id, doc, req.user)
        .await
        .and_then(|d| {
            serde_json::to_string(&d).map_or_else(|e| internal_server_error(""), |s| Ok(s))
        })
}

pub async fn delete(
    class: ClassName,
    id: String,
    req: Request,
    ctx: Arc<Context>,
) -> Result<impl Reply, Rejection> {
    ctx.db
        .delete(class.get_name(), &id, req.user)
        .await
        .and_then(|d| {
            serde_json::to_string(&d).map_or_else(|e| internal_server_error(""), |s| Ok(s))
        })
}
