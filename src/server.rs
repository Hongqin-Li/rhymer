use std::{
    collections::HashMap,
    convert::{Infallible, TryFrom},
    pin::Pin,
    sync::{Arc, Mutex},
};

use error::{bad_request, internal_server_error};
use mongodb::bson::Document;
use serde::{de::DeserializeOwned, private::de::IdentifierDeserializer};
use serde_json::{Map, Value};
use user::{decode_token, ClientToken};
use warp::{
    body::aggregate,
    hyper::HeaderMap,
    reject::{self, Reject},
    Buf, Future,
};
use warp::{Filter, Rejection};

use crate::{
    database::{Database as _, Mongodb as Database},
    error,
    function::{self, FuncMap, Function, HookFunc, HookMap},
    object::{self, Object},
    user::{self, User, UserKind},
    validator::ClassName,
};

/// Per-request data passed to Rhymer.
#[derive(Debug, Clone)]
pub struct Request {
    pub headers: warp::http::HeaderMap,
    pub body: Option<Document>,

    // Extracted from headers
    pub user: UserKind,
}

/// Per-server data passed on to Rhymer.
#[derive(Clone)]
pub struct Context {
    pub config: ServerConfig,
    pub db: Database,
    pub before_save: HookMap,
    pub after_save: HookMap,
    pub before_destroy: HookMap,
    pub after_destroy: HookMap,
    pub function: FuncMap,
}

// Helper functions, which is passed on to server-side hooks and functions.
impl Context {
    /// Login with username and password
    pub async fn login(self: Arc<Self>, name: &str, pwd: &str) -> Result<User, ()> {
        let mut u = User::with_context(self);
        u.login(name, pwd).await.map_or_else(|e| Err(()), |t| Ok(u))
    }

    /// Create an object in specific class
    pub fn object(self: Arc<Self>, class: &str) -> Object {
        let mut obj = Object::from(self, UserKind::Master);
        obj.set_class(class);
        obj
    }

    /// Run a function by name
    pub fn f(self: Arc<Self>, name: &str) -> Function {
        unimplemented!();
    }
}

fn with_context(
    ctx: Arc<Context>,
) -> impl Filter<Extract = (Arc<Context>,), Error = Infallible> + Clone {
    warp::any().map(move || ctx.clone())
}

fn parse_user(headers: &HeaderMap, key: &str) -> UserKind {
    let token = headers
        .get("X-Parse-Session-Token")
        .map_or(None, |h| h.to_str().map_or(None, |s| decode_token(s, &key)));
    let master = headers.get("X-Parse-Master-Key").map_or(false, |k| {
        k.to_str()
            .map_or(false, |k| if k == key { true } else { false })
    });

    if master {
        UserKind::Master
    } else if let Some(t) = token {
        UserKind::Client(t)
    } else {
        UserKind::Guest
    }
}

fn with_req(
    key: impl Into<String>,
) -> impl Filter<Extract = (Request,), Error = Rejection> + Clone {
    let key: String = key.into();
    warp::header::headers_cloned().and(warp::body::json()).map(
        move |headers: HeaderMap, body: Map<String, Value>| {
            // See https://github.com/mongodb/bson-rust/issues/189
            let body = Document::try_from(body).map_or_else(
                |e| {
                    warn!("request body deserialized failed");
                    None
                },
                |d| Some(d),
            );
            let user = parse_user(&headers, &key);
            Request {
                headers,
                body,
                user,
            }
        },
    )
}

fn with_req_without_body(
    key: impl Into<String>,
) -> impl Filter<Extract = (Request,), Error = Infallible> + Clone {
    let key: String = key.into();
    warp::header::headers_cloned().map(move |headers: HeaderMap| {
        let user = parse_user(&headers, &key);
        Request {
            headers,
            body: None,
            user,
        }
    })
}

/// Server configuration
#[derive(Debug, Clone, Default)]
pub struct ServerConfig {
    /// port
    pub port: u16,

    /// The secret will be used to create JWT and should not
    /// be available to client.
    pub secret: String,
    /// MongoDB url of form `mongodb://USERNAME:PASSWORD@localhost:27017/DATABASE_NAME`
    pub database_url: String,

    /// Maximum legal body size in bytes.
    pub body_limit: u64,
}

/// The server
#[derive(Clone, Default)]
pub struct Server {
    config: ServerConfig,
    before_save: HookMap,
    after_save: HookMap,
    before_destroy: HookMap,
    after_destroy: HookMap,
    function: FuncMap,
}

impl Server {
    /// Create a server from option
    pub async fn from_option(config: ServerConfig) -> Self {
        Self {
            config,
            ..Server::default()
        }
    }

    /// Register a before save hook function.
    pub fn before_save(&mut self, class_name: impl Into<String>, f: HookFunc) {
        self.before_save.insert(class_name.into(), f);
    }
    /// Register a after save hook function.
    pub fn after_save(&mut self, class_name: impl Into<String>, f: HookFunc) {
        self.after_save.insert(class_name.into(), f);
    }
    /// Register a before destroy hook function.
    pub fn before_destroy(&mut self, class_name: impl Into<String>, f: HookFunc) {
        self.before_destroy.insert(class_name.into(), f);
    }
    /// Register a after destroy hook function.
    pub fn after_destroy(&mut self, class_name: impl Into<String>, f: HookFunc) {
        self.after_destroy.insert(class_name.into(), f);
    }

    /// Register a function to be invoked by api.
    pub fn define(&mut self, name: impl Into<String>, f: Function) {
        self.function.insert(name.into(), f);
    }

    /// Warp's filters for routing.
    pub async fn routes(
        &mut self,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        let context = Arc::new(Context {
            db: Database::from_url(self.config.database_url.clone()).await,
            config: self.config.clone(),
            before_save: self.before_save.clone(),
            after_save: self.after_save.clone(),
            before_destroy: self.before_destroy.clone(),
            after_destroy: self.after_destroy.clone(),
            function: self.function.clone(),
        });

        // Body extraction must be at last to avoid multiple extraction.
        macro_rules! get {
            ($($e:expr), *) => {
                warp::get()
                    $(.and($e))*
                    .and(with_req_without_body(self.config.secret.clone()))
                    .and(with_context(context.clone()));
            }
        }

        macro_rules! post {
            ($($e:expr), *) => {
                warp::post()
                    $(.and($e))*
                    .and(with_req(self.config.secret.clone()))
                    .and(with_context(context.clone()));
            }
        }

        macro_rules! put {
            ($($e:expr), *) => {
                warp::put()
                    $(.and($e))*
                    .and(with_req(self.config.secret.clone()))
                    .and(with_context(context.clone()));
            }
        }

        macro_rules! delete {
            ($($e:expr), *) => {
                warp::delete()
                    $(.and($e))*
                    .and(with_req_without_body(self.config.secret.clone()))
                    .and(with_context(context.clone()));
            }
        }

        let signup = post!(warp::path("users")).and_then(user::signup);

        let login =
            get!(warp::path("login"), warp::query::<user::LoginQuery>()).and_then(user::login);

        let user_routes = signup.or(login);

        let create = post!(warp::path!("classes" / ClassName)).and_then(object::create);

        let retrieve = get!(warp::path!("classes" / ClassName / String)).and_then(object::retrieve);

        let retrieve_by_filter = get!(warp::path!("classes" / ClassName), warp::query())
            .and_then(object::retrieve_by_filter);

        let update = put!(warp::path!("classes" / ClassName / String)).and_then(object::update);

        let delete = delete!(warp::path!("classes" / ClassName / String)).and_then(object::delete);

        let object_routes = create
            .or(retrieve)
            .or(retrieve_by_filter)
            .or(update)
            .or(delete);

        let function_route =
            get!(warp::path!("functions" / String), warp::query()).and_then(function::run);

        let routes = user_routes
            .or(object_routes)
            .or(function_route)
            .recover(error::handle_rejection);

        routes
    }

    /// Run this `Server` forever on the current thread.
    pub async fn run(&mut self) {
        warp::serve(self.routes().await)
            .run(([127, 0, 0, 1], self.config.port))
            .await;
    }
}
