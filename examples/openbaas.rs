#![feature(with_options)]
#![feature(async_closure)]

use std::{
    collections::HashMap,
    fs::{self},
    io,
    sync::Arc,
    vec,
};

use mongodb::bson::doc;
use serde_json::json;
use std::io::{Read, Write};
use tokio::process::Command;

use log::trace;
use pretty_env_logger;
use rhymer::{
    error::{bad_request, internal_server_error, unauthorized},
    user::UserKind,
    Acl, Config, Context, File, Rejection, Request, Server,
};
use tempfile::TempDir;
use tokio;

fn user_namespace(uid: &str) -> String {
    format!("ob-user-{}", uid)
}
fn user_image_page(uid: &str) -> String {
    format!("ob-page-{}", uid)
}

// FIXME: just by `kubectl proxy`
fn external_url(namespace: &str, service: &str) -> String {
    format!(
        "http://127.0.0.1:8001/api/v1/namespaces/{}/services/{}/proxy/",
        namespace, service
    )
}

async fn page_url(
    req: Request,
    ctx: Arc<Context>,
    arg: HashMap<String, String>,
) -> Result<String, Rejection> {
    if let Some(uid) = arg.get("uid") {
        let (ns, svc) = (user_namespace(uid), user_image_page(uid));
        let url = external_url(&ns, &svc);

        let mut obj = ctx.object("_User");
        obj.get(uid).await?;
        let purl = if let Some(purl) = obj.data.get("pageSourceUrl").map(|b| b.as_str().unwrap()) {
            purl
        } else {
            ""
        };

        trace!("page_url: page source url {}", &purl);

        Ok(json!({
            "pageUrl": url,
            "pageSourceUrl": purl,
        })
        .to_string())
    } else {
        unauthorized("Please login first")
    }
}

async fn deploy_page(
    req: Request,
    ctx: Arc<Context>,
    arg: HashMap<String, String>,
) -> Result<String, Rejection> {
    let url = arg
        .get("url")
        .map_or_else(|| bad_request("Please provide url"), |s| Ok(s))?;

    let dir = TempDir::new().unwrap();

    let zip_file = dir.path().join("index.zip");
    let dockerfile = dir.path().join("Dockerfile");
    let deploy_file = dir.path().join("deploy.yml");
    let zip_file_str = zip_file.to_str().unwrap();
    let dockerfile_str = dockerfile.to_str().unwrap();
    let deploy_file_str = deploy_file.to_str().unwrap();

    let user_id = if let UserKind::Client(c) = req.user {
        c.id
    } else {
        return unauthorized("Please login first");
    };
    let image_name = user_image_page(&user_id);
    let namespace = user_namespace(&user_id);

    trace!("deploy page temp dir: {:?}", &dir);

    // Download index.zip
    let mut resp = reqwest::get(url).await.expect("request failed");
    let mut out = fs::File::with_options()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&zip_file)
        .expect("failed to create file");
    io::copy(&mut resp.bytes().await.unwrap().as_ref(), &mut out).expect("failed to copy content");

    macro_rules! exec {
        ($cmd:expr, $args:expr) => {
            Command::new($cmd)
                .current_dir(&dir)
                .args($args)
                .output()
                .await
                .map_or_else(
                    |_| internal_server_error("Failed to execute command"),
                    |o| Ok(o),
                )
        };
    }

    // unzip index.zip
    exec!("unzip", &["-o", &zip_file_str, "-d", "public"])?;

    // Create dockerfile and build.
    fs::write(
        &dockerfile,
        "
FROM nginx:1.13.0-alpine
COPY ./public /usr/share/nginx/html
    ",
    )
    .expect("failed to write file");

    exec!("docker", &["build", "-t", &image_name, "."])?;

    // Load to kind's internal docker so that we can use it in the cluster
    exec!("kind", &["load", "docker-image", &image_name])?;

    // NOTE: use local docker images by `imagePullPolicy: Never`
    // Create Deployment file and and deploy to k8s
    fs::write(
        &deploy_file,
        format!(
            "
  apiVersion: apps/v1
  kind: Deployment
  metadata:
    name: {}
    labels:
      app: page
  spec:
    replicas: 1
    revisionHistoryLimit: 5
    selector:
      matchLabels:
        app: page
    template:
      metadata:
        labels:
          app: page
      spec:
        containers:
        - name: {}
          image: {}
          imagePullPolicy: Never
          ports:
          - containerPort: 80
",
            image_name, image_name, image_name
        ),
    )
    .expect("Failed to create Dockerfile");

    // Create user namespace if not exist
    // List the namespace by `kubectl get namespaces`
    let result = exec!("kubectl", &["create", "namespace", &namespace]);

    // Deploy to k8s
    let result = exec!(
        "kubectl",
        &["delete", "-f", &deploy_file_str, "--namespace", &namespace]
    );
    exec!(
        "kubectl",
        &["apply", "-f", &deploy_file_str, "--namespace", &namespace]
    )?;

    let result = exec!(
        "kubectl",
        &["delete", "service", &image_name, "--namespace", &namespace]
    );
    exec!(
        "kubectl",
        &[
            "expose",
            "deployment",
            &image_name,
            "--namespace",
            &namespace
        ]
    )?;

    // Update user page url
    let mut user = ctx.user(user_id);
    user.set("pageSourceUrl", url);
    user.save().await?;

    Ok(json!({ "url": external_url(&namespace, &image_name) }).to_string())
}

async fn after_save_file(f: File, req: Request, ctx: Arc<Context>) -> Result<File, Rejection> {
    match req.user.clone() {
        UserKind::Client(c) => {
            let data = doc! {
                "fileSize": &f.file_size,
                "fileName": &f.file_name,
                "createdBy": &c.id,
                "url": &f.url,
                "name": &f.name,
            };
            trace!("after save file: {:?}", &data);

            let mut acl = Acl::new();
            acl.set_public_invisiable();
            acl.set_writable(c.id);

            let mut obj = ctx.object("UserFile");
            obj.set_doc(data);
            obj.set_acl(acl);
            obj.save().await?;

            Ok(f)
        }
        UserKind::Master => unimplemented!(),
        UserKind::Guest => internal_server_error("Guest cannot save files"),
    }
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    let mongo_user = "rhymer-test";
    let mongo_pwd = "rhymer-test";
    let mongo_db = "rhymer-test";
    let mut r = Server::from_option(Config {
        port: 8086,
        secret: "YOU WILL NEVER KNOWN".to_owned(),
        database_url: format!(
            "mongodb://{}:{}@localhost:27017/{}",
            mongo_user, mongo_pwd, mongo_db
        ),
        server_url: "http://localhost:8086".to_string(),
        body_limit: 16 * 1024,
    })
    .await;

    r.after_save_file(Box::new(|f, req, ctx| {
        Box::pin(after_save_file(f, req, ctx))
    }));

    r.define(
        "deploy-page",
        Box::new(|req, ctx, arg| Box::pin(deploy_page(req, ctx, arg))),
    );

    r.define(
        "page-url",
        Box::new(|req, ctx, arg| Box::pin(page_url(req, ctx, arg))),
    );

    r.run().await;
}
