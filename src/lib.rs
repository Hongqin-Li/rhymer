#![deny(missing_docs)]
//! Backend-as-a-Service in Rust
#![feature(async_closure)]

#[macro_use]
extern crate log;

mod acl;
mod database;
mod error;
mod function;
mod object;
mod server;
mod user;
mod validator;

pub use mongodb::bson::Document;
pub use server::Server;
pub use server::ServerConfig;

#[cfg(test)]
mod tests {
    use super::*;

    use mongodb::bson::doc;
    use warp::{Filter, Rejection, Reply};

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

        Server::from_option(ServerConfig {
            port: 8086,
            secret: "YOU WILL NEVER KNOWN".to_owned(),
            database_url: url,
            body_limit: 16 * 1024,
        })
        .await
    }

    pub async fn test_api() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        let mut r = test_server().await;
        r.routes().await
    }
}
