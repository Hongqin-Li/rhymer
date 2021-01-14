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

use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};

/// JWT token of a verified user.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ClientToken {
    pub(crate) sub: String,
    pub(crate) exp: i64,

    /// Verified id of this user.
    pub id: String,

    /// Verified name of this user.
    pub name: String,
}

/// User kinds.
///
#[derive(Debug, Clone)]
pub enum UserKind {
    /// `Master` represents client request with Master Key.
    Master,
    /// `Client` is logged-in client.
    Client(ClientToken),
    /// `Guest` is non-logged in client.
    Guest,
}

/// User instance.
#[derive(Clone)]
pub struct User {
    kind: UserKind,
    ctx: Arc<Context>,
}

impl User {
    /// Create a user instance with server context.
    pub fn with_context(ctx: Arc<Context>) -> Self {
        User {
            kind: UserKind::Guest,
            ctx,
        }
    }

    /// Sign up with username and password, updating this user instance.
    ///
    /// Note that the uniqueness of username should be guaranteed by `scripts/init-db.js`
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

    /// Log in with username and password, updating this user instance.
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
                exp: chrono::Utc::now().timestamp() + 900, // Expire after 15min.
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

/// Encode a token struct `t` by key.
pub fn encode_token(t: &ClientToken, key: &str) -> Result<String, Rejection> {
    jsonwebtoken::encode(
        &Header::default(),
        &t,
        &EncodingKey::from_secret(key.as_ref()),
    )
    .map_or_else(
        |_e| error::internal_server_error("Error when encoding JWT"),
        |s| Ok(s),
    )
}

/// Decode token string `s` by key.
///
/// Return error if token invalid or expire.
pub fn decode_token(s: &str, key: &str) -> Result<ClientToken, Rejection> {
    trace!("decode token: {} by key {}", s, key);
    match jsonwebtoken::decode::<ClientToken>(
        &s,
        &DecodingKey::from_secret(key.as_ref()),
        &Validation::new(Algorithm::HS256),
    ) {
        Ok(t) => Ok(t.claims),
        Err(e) => {
            error!(
                "user token expired or wrong: {}, server may be under attack",
                s
            );
            bad_request("Token invalid, maybe expired")
        }
    }
}

/// Login query struct.
#[derive(Deserialize, Serialize)]
pub struct LoginQuery {
    username: String,
    password: String,
}

pub(crate) async fn signup(req: Request, ctx: Arc<Context>) -> Result<impl Reply, Rejection> {
    if let Some(body) = req.body {
        trace!("signup: {}", body);
        if let (Ok(name), Ok(pwd)) = (body.get_str("username"), body.get_str("password")) {
            let name = name.to_string().parse::<UserName>();
            let pwd = pwd.to_string().parse::<UserPassword>();
            match (name, pwd) {
                (Ok(name), Ok(pwd)) => {
                    let mut user = User::with_context(ctx.clone());
                    user.signup(name.as_str(), pwd.as_str()).await.map_or_else(
                        |e| conflict("User exists"),
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

pub(crate) async fn login(
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
pub(crate) mod tests {
    use std::sync::Arc;

    use serde_json::{json, Value};
    use warp::hyper::StatusCode;

    use crate::tests::TEST_SERVER_KEY;

    use super::{super::tests::test_api, decode_token, encode_token, ClientToken};

    /// Create a temp user token by user ID `uid` with expiration time of 10000s.
    pub fn tmp_token(uid: impl Into<String>) -> String {
        let now = chrono::Utc::now().timestamp();
        let id: String = uid.into();
        let t = crate::user::ClientToken {
            sub: id.clone(),
            id: id.clone(),
            name: "whatever".to_string(),
            exp: now + 10000,
        };
        encode_token(&t, crate::tests::TEST_SERVER_KEY).expect("error when encoding")
    }

    /// Helper macro for testing user log in.
    #[macro_export]
    macro_rules! login1 {
        ($api:expr, $name:expr, $pwd:expr) => {
            warp::test::request()
                .method("GET")
                .path(&format!("/login?username={}&password={}", $name, $pwd))
                .reply($api)
                .await
        };
    }

    /// Helper macro for testing user sign up.
    #[macro_export]
    macro_rules! signup1 {
        ($api:expr, $name:expr, $pwd:expr) => {
            warp::test::request()
            .method("POST")
            .path("/users")
            .json(&serde_json::json!({
                "username": $name,
                "password": $pwd
            }))
            .reply($api)
            .await
        };
    }

    #[test]
    fn test_jwt() {
        let now = chrono::Utc::now().timestamp();

        let t = ClientToken {
            sub: "x".to_string(),
            id: "x".to_string(),
            name: "foo".to_string(),
            exp: now + 100,
        };
        let s = encode_token(&t, TEST_SERVER_KEY).expect("error when encoding");
        let dt = decode_token(&s, TEST_SERVER_KEY).expect("error when decoding");
        assert_eq!(t.sub, dt.sub);
        assert_eq!(t.exp, dt.exp);
        assert_eq!(t.name, dt.name);
    }

    #[test]
    fn test_jwt_expired() {
        let now = chrono::Utc::now().timestamp();
        let t = ClientToken {
            sub: "x".to_string(),
            id: "x".to_string(),
            name: "foo".to_string(),
            exp: now - 1,
        };
        let s = encode_token(&t, TEST_SERVER_KEY).expect("error when encoding");
        assert!(decode_token(&s, TEST_SERVER_KEY).is_err());
    }

    #[tokio::test]
    async fn test_login_signup() {
        let api = test_api().await;

        // User not exists.
        let resp = login1!(&api, "foobar", "123");
        debug!("resp: {:?} body: {:?}", resp, resp.body());
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // User name validation.
        let resp = signup1!(&api, "fooA", "123");
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let resp = signup1!(&api, "foo1", "123");
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let resp = signup1!(&api, "foobar&", "123");
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let resp = signup1!(&api, "foo-1", "12345");
        assert_eq!(resp.status(), StatusCode::CREATED);
        let resp = signup1!(&api, "foo-A", "12345");
        assert_eq!(resp.status(), StatusCode::CREATED);

        // User register successfully.
        let resp = signup1!(&api, "foobar", "12345");
        let body: Value = serde_json::from_slice(&resp.body()[..]).unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        assert_eq!(body.get("username").unwrap(), "foobar");
        assert_eq!(body.get("password").unwrap(), "12345");
        assert!(body.get("createdAt").is_some());
        assert!(body.get("updatedAt").is_some());
        assert!(body.get("sessionToken").is_some());

        // User password error.
        let resp = login1!(&api, "foobar", "123456");
        debug!("resp: {:?} body: {:?}", resp, resp.body());
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // User login successfully.
        let resp = login1!(&api, "foobar", "12345");
        let body: Value = serde_json::from_slice(&resp.body()[..]).unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body.get("username").unwrap(), "foobar");
        assert_eq!(body.get("password").unwrap(), "12345");
        assert!(body.get("createdAt").is_some());
        assert!(body.get("updatedAt").is_some());
        assert!(body.get("sessionToken").is_some());
        let token = body
            .get("sessionToken")
            .expect("token not found")
            .as_str()
            .unwrap();
        let uid = body
            .get("objectId")
            .expect("user objectId not found")
            .as_str()
            .unwrap();

        let token = decode_token(token, TEST_SERVER_KEY).expect("token invalid");
        assert_eq!(token.id, uid);
        assert_eq!(token.name, "foobar");

        // User registration failed with name conflict.
        let resp = signup1!(&api, "foobar", "abcdef");
        assert_eq!(resp.status(), StatusCode::CONFLICT);
        debug!("resp {:?}", resp);
    }
}
