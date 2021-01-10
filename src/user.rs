use std::sync::Arc;

use crate::{
    database::{self, Database},
    error::{self, conflict, internal_server_error, unauthorized},
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

#[derive(Clone)]
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

    // NOTE: uniqueness of username is guaranteed by `scripts/init-db.js`
    pub async fn signup(
        &mut self,
        name: &str,
        pwd: &str,
    ) -> Result<(Document, ClientToken), Rejection> {
        let doc = doc! { "username": name, "password": pwd};
        trace!("signup: name {}, pwd {}, doc {}", name, pwd, doc);
        let result = (*self.ctx).db.create("_User", doc, UserKind::Master).await;
        result.map(|d| {
            let id = d.get_str(database::OBJECT_ID).unwrap().to_string();
            let token = ClientToken {
                sub: id.clone(),
                exp: 10, // FIXME: use context config
                id,
                name: name.to_owned(),
            };
            self.kind = UserKind::Client(token.clone());
            (d.to_owned(), token)
        })
    }

    pub async fn login(
        &mut self,
        name: &str,
        pwd: &str,
    ) -> Result<(Document, ClientToken), Rejection> {
        let filter = doc! {"username": name, "password": pwd};
        trace!("login filter: {:?}", filter);
        let mut v = self
            .ctx
            .db
            .retrieve("_User", filter, UserKind::Master)
            .await?;

        if let Some(d) = v.first() {
            let id = d.get_str(database::OBJECT_ID).unwrap().to_string();
            let token = ClientToken {
                sub: id.clone(),
                exp: 10, // FIXME: use context config
                id,
                name: name.to_owned(),
            };
            self.kind = UserKind::Client(token.clone());
            Ok((d.to_owned(), token))
        } else {
            unauthorized("")
        }
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
        trace!("signup: {}", body);
        if let (Ok(name), Ok(pwd)) = (body.get_str("username"), body.get_str("password")) {
            let name = name.to_string().parse::<UserName>();
            let pwd = pwd.to_string().parse::<UserPassword>();
            match (name, pwd) {
                (Ok(name), Ok(pwd)) => {
                    let mut user = User::with_context(ctx.clone());
                    user.signup(name.as_str(), pwd.as_str()).await.map_or_else(
                        |e| conflict("user exists"),
                        |(mut d, t)| {
                            let secret = ctx.config.secret.clone();
                            d.insert("sessionToken", encode_token(&t, &secret)?);
                            serde_json::to_string(&d).map_or_else(
                                |_e| internal_server_error("Serialization error"),
                                |s| {
                                    Ok(warp::reply::with_status(s, warp::http::StatusCode::CREATED))
                                },
                            )
                        },
                    )
                }
                (Ok(_), Err(_)) => error::bad_request("Password invalid"),
                (Err(_), _) => error::bad_request("User name invalid"),
            }
        } else {
            bad_request("User name or password not provided in request body")
        }
    } else {
        bad_request("Body not found")
    }
}

pub async fn login(
    q: LoginQuery,
    _req: Request,
    ctx: Arc<Context>,
) -> Result<impl Reply, Rejection> {
    let secret = ctx.config.secret.clone();
    let mut user = User::with_context(ctx);
    let result = user.login(&q.username, &q.password).await.map_or_else(
        |_e| unauthorized("User not found or password error"),
        |(mut d, t)| {
            d.insert("sessionToken", encode_token(&t, &secret)?);
            serde_json::to_string(&d)
                .map_or_else(|e| internal_server_error("Serialization error"), |s| Ok(s))
        },
    );
    result
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::{json, Value};
    use warp::hyper::StatusCode;

    use super::super::tests::test_api;

    #[tokio::test]
    async fn test_login_signup() {
        let api = test_api().await;

        let login1 = async move |api, name, pwd| {
            warp::test::request()
                .method("GET")
                .path(&format!("/login?username={}&password={}", name, pwd))
                .reply(api)
                .await
        };

        let signup1 = async move |api, name, pwd| {
            warp::test::request()
                .method("POST")
                .path("/users")
                .json(&json!({
                    "username": name,
                    "password": pwd
                }))
                .reply(api)
                .await
        };

        // User not exists.
        let resp = login1(&api, "foobar", "123").await;
        debug!("resp: {:?} body: {:?}", resp, resp.body());
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // User name validation.
        let resp = signup1(&api, "fooA", "123").await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let resp = signup1(&api, "foo1", "123").await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let resp = signup1(&api, "foobar&", "123").await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let resp = signup1(&api, "foo-1", "12345").await;
        assert_eq!(resp.status(), StatusCode::CREATED);
        let resp = signup1(&api, "foo-A", "12345").await;
        assert_eq!(resp.status(), StatusCode::CREATED);

        // User register successfully.
        let resp = signup1(&api, "foobar", "12345").await;
        let body: Value = serde_json::from_slice(&resp.body()[..]).unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        assert_eq!(body.get("username").unwrap(), "foobar");
        assert_eq!(body.get("password").unwrap(), "12345");
        assert!(body.get("createdAt").is_some());
        assert!(body.get("updatedAt").is_some());
        assert!(body.get("sessionToken").is_some());

        // User password error.
        let resp = login1(&api, "foobar", "123456").await;
        debug!("resp: {:?} body: {:?}", resp, resp.body());
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // User login successfully.
        let resp = login1(&api, "foobar", "12345").await;
        let body: Value = serde_json::from_slice(&resp.body()[..]).unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body.get("username").unwrap(), "foobar");
        assert_eq!(body.get("password").unwrap(), "12345");
        assert!(body.get("createdAt").is_some());
        assert!(body.get("updatedAt").is_some());
        assert!(body.get("sessionToken").is_some());

        // User registration failed with name conflict.
        let resp = signup1(&api, "foobar", "abcdef").await;
        assert_eq!(resp.status(), StatusCode::CONFLICT);
        debug!("resp {:?}", resp);
    }
}
