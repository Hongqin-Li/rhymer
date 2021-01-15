use std::{
    collections::HashMap,
    convert::{Infallible, TryFrom},
    sync::Arc,
};

use mongodb::bson::Document;
use serde_json::{Map, Value};
use user::decode_token;
use warp::hyper::{HeaderMap, Method};
use warp::{Filter, Rejection};

use crate::{
    database::{Database as _, Mongodb as Database},
    error, file,
    function::{self, FileHook, FuncMap, Function, HookFunc, HookMap},
    object::{self, Object, ObjectTrait},
    user::{self, User, UserKind},
    validator::ClassName,
};

/// Per-request data passed to Rhymer.
#[derive(Debug, Clone)]
pub struct Request {
    /// Headers of user request.
    pub headers: warp::http::HeaderMap,
    /// Body of user request.
    pub body: Option<Document>,
    /// Extracted from headers
    pub user: UserKind,
}

/// Per-server data passed on to Rhymer.
#[derive(Clone)]
pub struct Context {
    /// Server config.
    pub config: Config,

    /// Database connection instance.
    pub db: Database,

    /// Before save hook functions.
    pub before_save: HookMap,
    /// After save hook functions.
    pub after_save: HookMap,
    /// Before destroy hook functions.
    pub before_destroy: HookMap,
    /// After destory hook functions.
    pub after_destroy: HookMap,

    /// Function to trigger before saving a file.
    pub before_save_file: Option<FileHook>,
    /// Function to trigger after saving a file.
    pub after_save_file: Option<FileHook>,
    /// Function to trigger before deleting a file.
    pub before_delete_file: Option<FileHook>,
    /// Function to trigger after deleting a file.
    pub after_delete_file: Option<FileHook>,

    /// Functions.
    pub function: FuncMap,
}

// Helper functions, which is passed on to server-side hooks and functions.
impl Context {
    /// Login with username and password.
    pub async fn login(self: Arc<Self>, name: &str, pwd: &str) -> Result<User, ()> {
        let mut u = User::from_context(self, UserKind::Master);
        u.login(name, pwd).await.map_or_else(|e| Err(()), |t| Ok(u))
    }

    /// Create an object in specific class.
    pub fn object(self: Arc<Self>, class: &str) -> Object {
        let mut obj = Object::from_context(self, UserKind::Master);
        obj.set_class(class);
        obj
    }

    /// Create a user instance by id.
    pub fn user(self: Arc<Self>, id: impl Into<String>) -> User {
        let mut u = User::from_context(self, UserKind::Master);
        u.set_id(id);
        u
    }

    /// Run a function by name.
    pub fn f(self: Arc<Self>, name: &str) -> Function {
        unimplemented!();
    }
}

fn with_context(
    ctx: Arc<Context>,
) -> impl Filter<Extract = (Arc<Context>,), Error = Infallible> + Clone {
    warp::any().map(move || ctx.clone())
}

fn parse_user(headers: &HeaderMap, key: &str) -> Result<UserKind, Rejection> {
    let hdr_token = headers
        .get("x-parse-session-token")
        .map_or(None, |h| h.to_str().map_or(None, |s| Some(s)));

    let token = if let Some(s) = hdr_token {
        Some(decode_token(s, &key)?)
    } else {
        None
    };
    let master = headers.get("x-parse-master-key").map_or(false, |k| {
        k.to_str()
            .map_or(false, |k| if k == key { true } else { false })
    });
    trace!("header: {:?}", headers);

    Ok(if master {
        UserKind::Master
    } else if let Some(t) = token {
        UserKind::Client(t)
    } else {
        UserKind::Guest
    })
}

fn with_req(
    key: impl Into<String>,
    body_limit: u64,
) -> impl Filter<Extract = (Request,), Error = Rejection> + Clone {
    let key: String = key.into();
    warp::header::headers_cloned()
        .and(warp::body::content_length_limit(body_limit))
        .and(
            warp::body::json()
                .map(move |body: Map<String, Value>| {
                    // See https://github.com/mongodb/bson-rust/issues/189
                    let body = Document::try_from(body).map_or_else(
                        |e| {
                            warn!("request body deserialized failed");
                            None
                        },
                        |d| Some(d),
                    );
                    body
                })
                // .or(warp::any().map(|| None))
                // .unify(),
        )
        .and(warp::any().map(move || key.clone()))
        .and_then(
            async move |headers: HeaderMap,
                        body: Option<Document>,
                        key: String|
                        -> Result<Request, Rejection> {
                let user = parse_user(&headers, &key)?;
                Ok(Request {
                    headers,
                    body,
                    user,
                })
            },
        )
}

fn with_req_without_body(
    key: impl Into<String>,
) -> impl Filter<Extract = (Request,), Error = Rejection> + Clone {
    let key: String = key.into();
    warp::header::headers_cloned()
        .and(warp::any().map(move || key.clone()))
        .and_then(
            async move |headers: HeaderMap, key: String| -> Result<Request, Rejection> {
                let user = parse_user(&headers, &key)?;
                Ok(Request {
                    headers,
                    body: None,
                    user,
                })
            },
        )
}

/// Server configuration
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// port
    pub port: u16,

    /// The secret will be used to create JWT and should not
    /// be available to client.
    pub secret: String,
    /// MongoDB url of form `mongodb://USERNAME:PASSWORD@localhost:27017/DATABASE_NAME`
    pub database_url: String,

    /// Url of this server.
    ///
    /// This url will be used to generated links for uploaded files.
    pub server_url: String,

    /// Maximum legal body size in bytes.
    pub body_limit: u64,
}

/// The server
#[derive(Clone, Default)]
pub struct Server {
    config: Config,
    before_save: HookMap,
    after_save: HookMap,
    before_destroy: HookMap,
    after_destroy: HookMap,
    before_save_file: Option<FileHook>,
    after_save_file: Option<FileHook>,
    before_delete_file: Option<FileHook>,
    after_delete_file: Option<FileHook>,
    function: FuncMap,
}

impl Server {
    /// Create a server from option
    pub async fn from_option(config: Config) -> Self {
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

    /// Register a hook function triggered before saving a file.
    pub fn before_save_file(&mut self, f: FileHook) {
        self.before_save_file = Some(f);
    }
    /// Register a hook function triggered after saving a file.
    pub fn after_save_file(&mut self, f: FileHook) {
        self.after_save_file = Some(f);
    }
    /// Register a hook function triggered before deleting a file.
    pub fn before_delete_file(&mut self, f: FileHook) {
        self.before_delete_file = Some(f);
    }
    /// Register a hook function triggered after deleting a file.
    pub fn after_delete_file(&mut self, f: FileHook) {
        self.after_delete_file = Some(f);
    }

    /// Register a function to be invoked by api.
    pub fn define(&mut self, name: impl Into<String>, f: Function) {
        self.function.insert(name.into(), f);
    }

    /// Warp's filters for routing.
    pub async fn routes(
        &self,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        let context = Arc::new(Context {
            db: Database::from_url(self.config.database_url.clone()).await,
            config: self.config.clone(),
            before_save: self.before_save.clone(),
            after_save: self.after_save.clone(),
            before_destroy: self.before_destroy.clone(),
            after_destroy: self.after_destroy.clone(),
            before_save_file: self.before_save_file.clone(),
            after_save_file: self.after_save_file.clone(),
            before_delete_file: self.before_delete_file.clone(),
            after_delete_file: self.after_delete_file.clone(),
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
                    .and(with_req(self.config.secret.clone(), self.config.body_limit))
                    .and(with_context(context.clone()));
            }
        }

        macro_rules! put {
            ($($e:expr), *) => {
                warp::put()
                    $(.and($e))*
                    .and(with_req(self.config.secret.clone(), self.config.body_limit))
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

        // Be careful about the order to avoid consuming request body multiple times.
        let update_user = post!(warp::path!("users" / String)).and_then(user::update);

        let signup = post!(warp::path("users")).and_then(user::signup);

        let login =
            get!(warp::path("login"), warp::query::<user::LoginQuery>()).and_then(user::login);

        let user_routes = update_user.or(signup).or(login);

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

        let get_function =
            get!(warp::path!("functions" / String), warp::query()).and_then(function::run);

        let post_function = warp::post()
            .and(warp::path!("functions" / String))
            .and(warp::body::content_length_limit(self.config.body_limit))
            .and(
                warp::body::json()
                    .or(warp::any().map(|| HashMap::default()))
                    .unify(),
            )
            .and(with_req_without_body(self.config.secret.clone()))
            .and(with_context(context.clone()))
            .and_then(function::run_post);

        let function_route = get_function.or(post_function);

        // Upload file by application/x-www-form-urlencoded
        let create_file = warp::post()
            .and(warp::path!("files" / String))
            .and(warp::body::content_length_limit(self.config.body_limit))
            .and(warp::body::bytes())
            .and(with_req_without_body(self.config.secret.clone()))
            .and(with_context(context.clone()))
            .and_then(file::create);

        let retrieve_file =
            get!(warp::path("files"), warp::fs::dir("./files")).and_then(file::retrieve);

        let delete_file = delete!(warp::path!("files" / String / String)).and_then(file::delete);

        let file_routes = retrieve_file.or(create_file).or(delete_file);

        let cors = warp::cors()
            .allow_any_origin()
            .allow_headers(vec![
                "Content-Type",
                "X-Parse-Application-Id",
                "X-Parse-Revocable-Session",
                "X-Parse-Session-Token",
            ])
            .allow_methods(&[Method::GET, Method::POST, Method::DELETE, Method::PUT]);

        let routes = user_routes
            .or(object_routes)
            .or(function_route)
            .or(file_routes)
            .recover(error::handle_rejection)
            .with(cors);

        routes
    }

    /// Run this `Server` forever on the current thread.
    pub async fn run(&mut self) {
        warp::serve(self.routes().await)
            .run(([127, 0, 0, 1], self.config.port))
            .await;
    }
}
