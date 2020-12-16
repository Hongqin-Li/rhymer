use crate::{
    acl::Acl,
    database::{Database as _, Mongodb as Database},
    error::{self, internal_server_error},
    rhymer::{Context, Request},
    validator::ClassName,
};
use chrono::Utc;
use chrono::{DateTime, SecondsFormat};
use error::not_found;
use mongodb::bson::{doc, Document};
use std::{result::Result, sync::Arc};
use tokio::stream::StreamExt;
use warp::{Rejection, Reply};

pub struct Object {
    class: String,
    doc: Document,
    acl: Acl,
    ctx: Arc<Context>,
}

impl Object {
    pub fn with_context(ctx: Arc<Context>) -> Self {
        // FIXME:
        Object {
            ctx,
            class: "".to_string(),
            doc: Document::default(),
            acl: Acl::default(),
        }
    }

    pub fn class(&mut self, name: String) {
        self.class = name
    }

    pub async fn save(acl: Option<Document>) {}

    pub async fn destroy(opt: Option<u32>) {}
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
    result.and_then(
        |v| {
            serde_json::to_string(&v).map_or_else(|e| internal_server_error(""), |s| Ok(s))
        },
    )
}

pub async fn retrieve(
    class: ClassName,
    id: String,
    req: Request,
    ctx: Arc<Context>,
) -> Result<impl Reply, Rejection> {
    let filter = doc! { "ObjectId": id };
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
