use std::sync::Arc;

use crate::{
    database::{self, Database},
    error::{internal_server_error, unauthorized},
    object,
    rhymer::{Context, Request},
    validator::ClassName,
    Server,
};
use mongodb::bson::{doc, Document};
use serde::{Deserialize, Serialize};
use warp::{Rejection, Reply, reject::Reject};

use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ClientToken {
    sub: String,
    exp: usize,

    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub enum UserKind {
    Master,
    Client(ClientToken),
    Guest,
}

#[derive(Debug, Clone)]
pub struct User {
    kind: UserKind,
    ctx: Arc<Context>,
}

impl User {
    pub fn with_context(ctx: Arc<Context>) -> Self {
        User {
            kind: UserKind::Guest,
            ctx,
        }
    }

    pub async fn signup(&mut self, name: &str, pwd: &str) -> Result<Document, Rejection> {
        let doc = doc! { "username": name, "password": pwd};
        let result = (*self.ctx).db.create("_User", doc, UserKind::Master).await;
        result
    }

    pub async fn login(&mut self, name: &str, pwd: &str) -> Result<ClientToken, Rejection> {
        let filter = doc! {"username": name, "password": pwd};
        (*self.ctx)
            .db
            .retrieve("_User", filter, UserKind::Master)
            .await
            .map_or_else(|e| Err(e), |v| {
                if let Some(d) = v.first() {
                    if let Ok(id) = d.get_object_id(database::OBJECT_ID) {
                        let id = id.to_string();
                        let token = ClientToken {
                            sub: id.clone(),
                            exp: 10, // FIXME: use context config
                            id,
                            name: name.to_owned(),
                        };
                        self.kind = UserKind::Client(token.clone());
                        Ok(token)
                    } else {
                        unauthorized("")
                    }
                } else {
                    unauthorized("")
                }
            })
    }
}

pub fn encode_token(t: &ClientToken, key: &str) -> String {
    jsonwebtoken::encode(
        &Header::default(),
        &t,
        &EncodingKey::from_secret(key.as_bytes()),
    )
    .expect("should not failed when encode jwt")
}

pub fn decode_token(s: &str, key: &str) -> Option<ClientToken> {
    match jsonwebtoken::decode::<ClientToken>(
        &s,
        &DecodingKey::from_secret(key.as_bytes()),
        &Validation::default(),
    ) {
        Ok(t) => Some(t.claims),
        Err(_) => None,
    }
}

#[derive(Deserialize, Serialize)]
pub struct SignupQuery {
    username: String,
    password: String,
    email: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct LoginQuery {
    username: String,
    password: String,
}

pub async fn signup(
    q: SignupQuery,
    req: Request,
    ctx: Arc<Context>,
) -> Result<impl Reply, Rejection> {
    let mut user = User::with_context(ctx);
    user.signup(&q.username, &q.password).await.and_then(|d| {
        serde_json::to_string(&d).map_or_else(
            |e| internal_server_error("Cannot serialize document"),
            |s| Ok(s),
        )
    })
}

pub async fn login(
    q: LoginQuery,
    req: Request,
    ctx: Arc<Context>,
) -> Result<impl Reply, Rejection> {
    let mut user = User::with_context(ctx);
    let token_result = user.login(&q.username, &q.password).await.map_or_else(
        |e| unauthorized("User not found or password error"),
        |t| {
            serde_json::to_string(&t).map_or_else(
                |e| internal_server_error("Cannot serialize token"),
                |s| Ok(s),
            )
        },
    );
    token_result
}
