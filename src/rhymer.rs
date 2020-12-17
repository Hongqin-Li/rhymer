use std::{convert::Infallible, sync::Arc};

use mongodb::bson::Document;
use user::{decode_token, ClientToken};
use warp::hyper::HeaderMap;
use warp::{Filter, Rejection};

use crate::{
    database::{Database as _, Mongodb as Database},
    error,
    function::{Function, Functions},
    object::{self, Object},
    user::{self, User, UserKind},
    validator::ClassName,
};

// Per-request data passed to Rhymer.

#[derive(Debug, Clone)]
pub struct Request {
    pub headers: warp::http::HeaderMap,
    pub body: Document,

    // Extracted from headers
    pub user: UserKind,
}

// Per-server data passed on to Rhymer.
#[derive(Debug, Clone)]
pub struct Context {
    pub config: ServerConfig,
    pub db: Database,
    // pub func: FunctionManager,
    // pub file: FileManeger,
}

fn with_context(
    ctx: Arc<Context>,
) -> impl Filter<Extract = (Arc<Context>,), Error = Infallible> + Clone {
    warp::any().map(move || ctx.clone())
}

fn with_req(key: String) -> impl Filter<Extract = (Request,), Error = Rejection> + Clone {
    warp::header::headers_cloned().and(warp::body::json()).map(
        move |headers: HeaderMap, body: Document| {
            let token = headers
                .get("X-Parse-Session-Token")
                .map_or(None, |h| h.to_str().map_or(None, |s| decode_token(s, &key)));
            let master = headers.get("X-Parse-Master-Key").map_or(false, |k| {
                k.to_str()
                    .map_or(false, |k| if k == key { true } else { false })
            });

            let user = {
                if master {
                    UserKind::Master
                } else if let Some(t) = token {
                    UserKind::Client(t)
                } else {
                    UserKind::Guest
                }
            };
            Request {
                headers,
                body,
                user,
            }
        },
    )
}

/// The server
#[derive(Clone)]
pub struct Server {
    config: ServerConfig,
    context: Arc<Context>,
}

// Rhymer server instance, which is passed to server-side hooks and functions.
impl Server {
    /// Login with username and password
    pub async fn login(&self, name: &str, pwd: &str) -> Result<User, ()> {
        let mut u = User::with_context(self.context.clone());
        u.login(name, pwd).await.map_or_else(|e| Err(()), |t| Ok(u))
    }

    /// Create an object in specific class
    pub fn object(&self, class: &str) -> Object {
        Object::from(self.context.clone(), UserKind::Master)
    }

    /// Run a function by name
    pub fn f(&self, name: &str) -> Function {
        unimplemented!();
    }
}

/// Server configuration
#[derive(Debug, Clone)]
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

impl Server {
    fn with_all(
        &self,
    ) -> impl Filter<Extract = (Request, Arc<Context>), Error = Rejection> + Clone {
        warp::any()
            .and(with_req(self.config.secret.clone()))
            .and(with_context(self.context.clone()))
    }

    /// Create a server from option
    pub async fn from_option(config: ServerConfig) -> Self {
        let context = Arc::new(Context {
            db: Database::from_url(config.database_url.clone()).await,
            config: config.clone(), //func: Functions::default(),
        });
        Server { config, context }
    }

    /// Run a server
    ///
    /// # Example
    ///
    /// ```
    /// use pretty_env_logger;
    /// use rhymer::{Server, ServerConfig};
    /// use tokio;

    /// #[tokio::main]
    /// async fn main() {
    ///     pretty_env_logger::init();
    ///     let mongo_user = "openbaas";
    ///     let mongo_pwd = "openbaas";
    ///     let mongo_db = "dev";
    ///     let mut r = Server::from_option(ServerConfig {
    ///         port: 8086,
    ///         secret: "YOU WILL NEVER KNOWN".to_owned(),
    ///         database_url: format!(
    ///             "mongodb://{}:{}@localhost:27017/{}",
    ///             mongo_user, mongo_pwd, mongo_db
    ///         ),
    ///         body_limit: 16 * 1024,
    ///     })
    ///     .await;
    ///     r.run().await;
    /// }
    /// ```
    pub async fn run(&mut self) {
        let signup = warp::post()
            .and(warp::path("users"))
            .and(warp::body::json())
            .and(self.with_all())
            .and_then(user::signup);

        let login = warp::get()
            .and(warp::path("login"))
            .and(warp::query::<user::LoginQuery>())
            .and(self.with_all())
            .and_then(user::login);

        let user_routes = signup.or(login);

        let create = warp::post()
            .and(warp::path!("classes" / ClassName))
            .and(warp::body::json())
            .and(self.with_all())
            .and_then(object::create);

        let retrieve = warp::get()
            .and(warp::path!("classes" / ClassName / String))
            .and(self.with_all())
            .and_then(object::retrieve);

        let retrieve_by_filter = warp::get()
            .and(warp::path!("classes" / ClassName))
            .and(warp::query())
            .and(self.with_all())
            .and_then(object::retrieve_by_filter);

        let update = warp::put()
            .and(warp::path!("classes" / ClassName / String))
            .and(warp::body::json())
            .and(self.with_all())
            .and_then(object::update);

        let delete = warp::delete()
            .and(warp::path!("classes" / ClassName / String))
            .and(self.with_all())
            .and_then(object::delete);

        let object_routes = create
            .or(retrieve)
            .or(retrieve_by_filter)
            .or(update)
            .or(delete);

        let routes = user_routes
            .or(object_routes)
            .recover(error::handle_rejection);

        warp::serve(warp::body::content_length_limit(self.config.body_limit).and(routes))
            .run(([127, 0, 0, 1], self.config.port))
            .await;
    }
}
