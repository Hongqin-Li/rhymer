use std::sync::Arc;

use crate::{
    database::{self, Database},
    error::{self, conflict, internal_server_error, unauthorized},
    object::ObjectTrait,
    server::{Context, Request},
    validator::{UserName, UserPassword},
    Acl,
};
use error::bad_request;
use mongodb::bson::{doc, Bson, Document};
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
    /// Data of current user.
    pub data: Document,

    id: Option<String>,

    /// User that owns this user object.
    user: UserKind,
    ctx: Arc<Context>,
}

impl User {
    const NAME: &'static str = "username";
    const PWD: &'static str = "password";

    /// Set value by key of data of this user.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<Bson>) {
        self.data.insert(key, value);
    }

    /// Sign up with username and password, updating this user instance.
    ///
    /// Note that the uniqueness of username should be guaranteed by `scripts/init-db.js`
    pub async fn signup(
        &mut self,
        name: &str,
        pwd: &str,
    ) -> Result<(Document, ClientToken), Rejection> {
        let doc = doc! { Self::NAME : name, Self::PWD : pwd};
        trace!("signup: name {}, pwd {}, doc {}", name, pwd, doc);
        self.data = doc;
        let d = self.save().await?;

        let id = d.get_str(database::OBJECT_ID).unwrap().to_string();
        let token = ClientToken {
            sub: id.clone(),
            exp: chrono::Utc::now().timestamp() + 900, // Expire after 15min.
            id,
            name: name.to_owned(),
        };
        Ok((d.to_owned(), token))
    }

    /// Log in with username and password, updating this user instance.
    pub async fn login(
        &mut self,
        name: &str,
        pwd: &str,
    ) -> Result<(Document, ClientToken), Rejection> {
        let filter = doc! {Self::NAME: name, Self::PWD : pwd};
        trace!("login filter: {:?}", filter);
        let v = self
            .ctx
            .db
            .retrieve("_User", filter, UserKind::Master)
            .await?;

        if let Some(d) = v.first() {
            if v.len() != 1 {
                return internal_server_error("User ID not unique");
            }
            let id = d.get_str(database::OBJECT_ID).unwrap().to_string();
            let token = ClientToken {
                sub: id.clone(),
                exp: chrono::Utc::now().timestamp() + 900, // Expire after 15min.
                id,
                name: name.to_owned(),
            };
            self.data = d.clone();
            // self.kind = UserKind::Client(token.clone());
            Ok((d.to_owned(), token))
        } else {
            unauthorized("")
        }
    }
}

#[async_trait::async_trait]
impl ObjectTrait for User {
    fn from_context(ctx: Arc<Context>, user: UserKind) -> Self {
        User {
            data: Document::default(),
            id: None,
            user,
            ctx,
        }
    }

    fn set_id(&mut self, id: impl Into<String>) {
        self.id = Some(id.into());
    }
    fn set_data(&mut self, data: impl Into<Document>) {
        self.data = data.into();
    }

    async fn get(&mut self, id: String) -> Result<Document, Rejection> {
        todo!();
    }
    async fn save(&mut self) -> Result<Document, Rejection> {
        // Validate name and password.
        let mut name = None;
        let mut pwd = None;
        if let Ok(s) = self.data.get_str(Self::NAME) {
            if let Err(s) = s.to_string().parse::<UserName>() {
                return bad_request("Cannot update with invalid username");
            } else {
                name = Some(s);
            }
        };
        if let Ok(s) = self.data.get_str(Self::PWD) {
            if let Err(s) = s.to_string().parse::<UserPassword>() {
                return bad_request("Cannot update with invalid password");
            } else {
                pwd = Some(s)
            }
        };

        if let Some(ref id) = self.id {
            // Update
            let result = match self.user.clone() {
                UserKind::Client(t) => {
                    if &t.id == id {
                        // Client can update itself.
                        (*self.ctx)
                            .db
                            .update("_User", id, self.data.clone(), self.user.clone())
                            .await?
                    } else {
                        return unauthorized("Client is not allowed to update other user's data");
                    }
                }
                // Guest cannot update username and password.
                UserKind::Guest => {
                    return unauthorized("Client is not allowed to update other user's data");
                }
                // Master can update anything
                UserKind::Master => {
                    (*self.ctx)
                        .db
                        .update("_User", id, self.data.clone(), self.user.clone())
                        .await?
                }
            };
            Ok(result)
        } else {
            // Create
            if let (Some(name), Some(pwd)) = (name, pwd) {
                let doc = (*self.ctx)
                    .db
                    .create("_User", self.data.clone(), self.user.clone())
                    .await
                    .map_or_else(|d| conflict("User exists"), |d| Ok(d))?;
                self.data = doc.clone();
                Ok(doc)
            } else {
                bad_request("Cannot create user without username or password")
            }
        }
    }

    async fn destroy(&mut self) -> Result<Document, Rejection> {
        unimplemented!("Do not support destroying user")
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

/// Sign up with request, used by RESTFul API.
pub async fn signup(req: Request, ctx: Arc<Context>) -> Result<impl Reply, Rejection> {
    trace!("user signup");

    if let Some(d) = req.body {
        let mut user = User::from_context(ctx.clone(), req.user.clone());
        user.set_data(d);
        let mut d = user.save().await?;

        let id = d.get_str(database::OBJECT_ID).unwrap().to_string();
        let name = d.get_str(User::NAME).unwrap();

        let token = ClientToken {
            sub: id.clone(),
            exp: chrono::Utc::now().timestamp() + 900, // Expire after 15min.
            id,
            name: name.to_owned(),
        };

        encode_token(&token, &ctx.config.secret).map_or_else(
            |e| internal_server_error("Failed to encode JWT token"),
            |t| {
                d.insert("sessionToken", t);
                Ok(())
            },
        )?;

        let result = serde_json::to_string(&d)
            .map_or_else(|e| internal_server_error("Serialization error"), |s| Ok(s))?;
        Ok(warp::reply::with_status(
            result,
            warp::http::StatusCode::CREATED,
        ))
    } else {
        bad_request("Failed to sign up with empty body")
    }
}

/// Login with query of username and password, used by RESTFul API.
pub async fn login(
    q: LoginQuery,
    req: Request,
    ctx: Arc<Context>,
) -> Result<impl Reply, Rejection> {
    trace!("user login");

    let secret = ctx.config.secret.clone();
    let mut user = User::from_context(ctx, req.user.clone());
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

/// Update user by id, used by RESTFul API.
///
/// Master can update any user and Client can only update itself.
pub async fn update(id: String, req: Request, ctx: Arc<Context>) -> Result<impl Reply, Rejection> {
    trace!("user update {}", &id);
    if let Some(body) = req.body {
        let mut user = User::from_context(ctx, req.user.clone());
        user.set_id(id);
        user.set_data(body);

        let d = user.save().await?;
        serde_json::to_string(&d)
            .map_or_else(|e| internal_server_error("Serialization error"), |s| Ok(s))
    } else {
        bad_request("Cannot update user with empty body")
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Arc;

    use serde_json::{json, Value};
    use warp::hyper::StatusCode;

    use crate::{tests::TEST_SERVER_KEY, with_user};

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

    #[tokio::test]
    async fn test_update() {
        let api = test_api().await;

        let update1 = async move |api, id, data| {
            warp::test::request()
                .method("POST")
                .path(&format!("/users/{}", id))
                .json(&data)
                .reply(api)
                .await
        };
        let update1_with_client = async move |api, uid, id, data| {
            with_user!(uid, "POST")
                .path(&format!("/users/{}", id))
                .json(&data)
                .reply(api)
                .await
        };
        let update1_with_master = async move |api, id, data| {
            warp::test::request()
                .header("x-parse-master-key", TEST_SERVER_KEY)
                .method("POST")
                .path(&format!("/users/{}", id))
                .json(&data)
                .reply(api)
                .await
        };

        let resp = signup1!(&api, "foobar", "12345");
        let body: Value = serde_json::from_slice(&resp.body()[..]).unwrap();
        let uid = body
            .get("objectId")
            .expect("failed to find objectId in user")
            .as_str()
            .expect("failed to convert into str");
        assert_eq!(resp.status(), StatusCode::CREATED);

        // Guest cannot update user.
        let resp = update1(&api, uid, json!({"username": "abcdefg"})).await;
        let body: Value = dbg!(serde_json::from_slice(&resp.body()[..]).unwrap());

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // Client cannot update other user.
        let resp = update1_with_client(&api, uid, "other", json!({"username": "abcdefg"})).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // Client cannot update with invalid username or password.
        let resp = update1_with_client(&api, uid, uid, json!({"username": "a"})).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let resp = update1_with_client(&api, uid, uid, json!({"password": "a"})).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // Client can update itself.
        let resp = update1_with_client(
            &api,
            uid,
            uid,
            json!({"username": "abcdefg", "password": "123456", "arbitrary": "data"}),
        )
        .await;
        let body: Value = serde_json::from_slice(&resp.body()[..]).unwrap();
        dbg!(&body);
        // Should return value before update.
        assert_eq!(body.get("username").unwrap().as_str().unwrap(), "foobar");
        assert_eq!(body.get("objectId").unwrap().as_str().unwrap(), uid);
        assert_eq!(body.get("arbitrary"), None);
        assert_eq!(resp.status(), StatusCode::OK);

        // Master can update any user.
        let resp = update1_with_master(
            &api,
            uid,
            json!({"username": "123456", "password": "0123456"}),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body: Value = serde_json::from_slice(&resp.body()[..]).unwrap();
        dbg!(&body);
        assert_eq!(body.get("username").unwrap().as_str().unwrap(), "abcdefg");
        assert_eq!(body.get("password").unwrap().as_str().unwrap(), "123456");
        assert_eq!(body.get("arbitrary").unwrap().as_str().unwrap(), "data");
        assert_eq!(body.get("objectId").unwrap().as_str().unwrap(), uid);
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
