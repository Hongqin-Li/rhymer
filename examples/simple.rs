use pretty_env_logger;
use rhymer::{Server, ServerConfig};
use tokio;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    let mongo_user = "rhymer-test";
    let mongo_pwd = "rhymer-test";
    let mongo_db = "rhymer-test";
    let mut r = Server::from_option(ServerConfig {
        port: 8086,
        secret: "YOU WILL NEVER KNOWN".to_owned(),
        database_url: format!(
            "mongodb://{}:{}@localhost:27017/{}",
            mongo_user, mongo_pwd, mongo_db
        ),
        body_limit: 16 * 1024,
    })
    .await;
    r.run().await;
}
