use std::sync::Arc;

use crate::{
    database::{self, Database},
    error::{self, internal_server_error, unauthorized},
    server::{Context, Request},
    validator::{UserName, UserPassword},
};
use error::bad_request;
use mongodb::bson::{doc, Document};
use serde::{Deserialize, Serialize};
use warp::{Rejection, Reply};

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
            .map_or_else(
                |e| Err(e),
                |v| {
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
                },
            )
    }
}

pub fn encode_token(t: &ClientToken, key: &str) -> Result<String, Rejection> {
    jsonwebtoken::encode(
        &Header::default(),
        &t,
        &EncodingKey::from_secret(key.as_bytes()),
    )
    .map_or_else(
        |_e| error::internal_server_error("Error when encoding JWT"),
        |s| Ok(s),
    )
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
pub struct LoginQuery {
    username: String,
    password: String,
}

pub async fn signup(req: Request, ctx: Arc<Context>) -> Result<impl Reply, Rejection> {
    if let Some(body) = req.body {
        if let (Some(name), Some(pwd)) = (body.get("username"), body.get("password")) {
            let name = name.to_string().parse::<UserName>();
            let pwd = pwd.to_string().parse::<UserPassword>();
            match (name, pwd) {
                (Ok(name), Ok(pwd)) => {
                    let mut user = User::with_context(ctx);
                    user.signup(name.get_name(), pwd.get_name())
                        .await
                        .and_then(|d| {
                            serde_json::to_string(&d).map_or_else(
                                |_e| internal_server_error("Serialization error"),
                                |s| Ok(s),
                            )
                        })
                }
                (Ok(_), Err(_)) => error::bad_request("Password invalid"),
                (Err(_), _) => error::bad_request("User name invalid"),
            }
        } else {
            bad_request("User name or password not found")
        }
    } else {
        bad_request("Body not found")
    }
}

pub async fn login(
    _req: Request,
    ctx: Arc<Context>,
    q: LoginQuery,
) -> Result<impl Reply, Rejection> {
    let secret = ctx.config.secret.clone();
    let mut user = User::with_context(ctx);
    let result = user.login(&q.username, &q.password).await.map_or_else(
        |_e| unauthorized("User not found or password error"),
        |t| encode_token(&t, &secret),
    );
    result
}
