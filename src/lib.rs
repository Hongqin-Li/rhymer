#![deny(missing_docs)]
//! Backend-as-a-Service in Rust
#![feature(async_closure)]
#![feature(with_options)]

#[macro_use]
extern crate log;

mod acl;
mod database;
mod file;
mod function;
mod object;
mod server;
/// User.
pub mod user;

/// Error rejections.
pub mod error;
mod validator;

pub use acl::Acl;
pub use file::File;
pub use mongodb::bson::Document;
pub use server::Config;
pub use server::Context;
pub use server::Request;
pub use server::Server;
pub use warp::Rejection;

#[cfg(test)]
mod tests {
    use super::*;

    use lazy_static::lazy_static;
    use mongodb::bson::doc;
    use warp::{Filter, Rejection, Reply};

    pub const TEST_SERVER_KEY: &str = "xxx";

    /// Helper macro for testing requests that requires user login.
    #[macro_export]
    macro_rules! with_user {
        ($id:expr, $method:expr) => {
            warp::test::request()
                .method($method)
                .header("x-parse-session-token", crate::user::tests::tmp_token($id))
        };
    }

    pub async fn test_server() -> Server {
        pretty_env_logger::try_init();

        let mongo_user = "rhymer-test";
        let mongo_pwd = "rhymer-test";
        let mongo_db = "rhymer-test";
        let url = format!(
            "mongodb://{}:{}@localhost:27017/{}",
            mongo_user, mongo_pwd, mongo_db
        );

        let client_options = mongodb::options::ClientOptions::parse(&url).await.unwrap();
        let name = client_options
            .clone()
            .credential
            .unwrap()
            .source
            .expect("mongodb database name should be provided in url.");
        let client = mongodb::Client::with_options(client_options).unwrap();
        let db = client.database(&name);
        for coll in db.list_collection_names(doc! {}).await.unwrap() {
            if coll.starts_with("_") {
                // Delete manually to keep the index of built-in collections.
                db.collection(&coll)
                    .delete_many(doc! {}, None)
                    .await
                    .unwrap();
            } else {
                db.collection(&coll).drop(None).await.unwrap();
            }
        }

        let r = Server::from_option(Config {
            port: 8086,
            secret: TEST_SERVER_KEY.to_string(),
            database_url: url,
            body_limit: 16 * 1024,
            server_url: "useless".to_string(),
        })
        .await;
        r
    }

    pub async fn test_api() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        let r = test_server().await;
        r.routes().await
    }
}
