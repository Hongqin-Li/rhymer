use std::convert::TryFrom;
use std::{
    fs::{self},
    sync::Arc,
    todo,
};
use std::{
    io::{Read, Write},
    path::PathBuf,
};

use serde_json::json;
use uuid::Uuid;
use warp::{hyper::body::Bytes, multipart::FormData, Rejection, Reply};

use crate::error::{internal_server_error, not_found, unauthorized};
use crate::{user::UserKind, Context, Request};

pub struct File {
    pub name: String,
    pub data: Bytes,

    pub file_size: u64,
    pub file_name: String,
    pub url: String,

    appid: String,
    user: UserKind,
    ctx: Arc<Context>,
}

impl File {
    pub fn new(
        name: impl Into<String>,
        data: Bytes,
        appid: impl Into<String>,
        user: UserKind,
        ctx: Arc<Context>,
    ) -> Self {
        let file_size = u64::try_from(data.len()).expect("failed to convert data length");
        Self {
            name: name.into(),
            data,
            file_size,
            file_name: String::default(),
            url: String::default(),

            appid: appid.into(),
            user,
            ctx,
        }
    }

    pub async fn save(&mut self) -> Result<(), Rejection> {
        let user = self.user.clone();
        match user {
            UserKind::Client(c) => {
                let mut path = PathBuf::from(format!("./files/{}", self.appid));

                fs::create_dir_all(&path).unwrap();

                self.file_name = format!("{}-{}-{}", c.id, Uuid::new_v4(), self.name);
                path.push(&self.file_name);

                self.url = format!(
                    "{}/files/{}/{}",
                    self.ctx.config.server_url.clone(),
                    self.appid,
                    self.file_name
                );

                if path.exists() {
                    return internal_server_error("File already exists");
                }
                let mut f = fs::File::with_options()
                    .create(true)
                    .write(true)
                    .open(path)
                    .map_or_else(
                        |e| internal_server_error("Error when creating file"),
                        |f| Ok(f),
                    )?;
                f.write(self.data.as_ref()).map_or_else(
                    |e| internal_server_error("Error when writing to file"),
                    |r| Ok(r),
                )?;
                trace!("create file: by user of name {} and id {}", c.name, c.id);
                Ok(())
            }
            UserKind::Guest => {
                // FIXME:
                internal_server_error("Please login to upload file")
            }
            UserKind::Master => internal_server_error("unimplemented"),
        }
    }
}

pub async fn create(
    name: String,
    buf: Bytes,
    req: Request,
    ctx: Arc<Context>,
) -> Result<impl Reply, Rejection> {
    let mut file = File::new(
        name,
        buf,
        req.headers
            .clone()
            .get("x-parse-application-id")
            .unwrap()
            .to_str()
            .unwrap(),
        req.user.clone(),
        ctx.clone(),
    );
    if let Some(f) = &ctx.before_save_file {
        file = f(file, req.clone(), ctx.clone()).await?;
    }

    file.save().await?;

    if let Some(f) = &ctx.after_save_file {
        file = f(file, req, ctx.clone()).await?;
    }

    let result = json!({
        "name": file.file_name,
        "url": file.url,
    })
    .to_string();
    Ok(warp::reply::with_status(
        result,
        warp::http::StatusCode::CREATED,
    ))
}

/// Everyone have access to files.
pub async fn retrieve(
    file: warp::filters::fs::File,
    req: Request,
    ctx: Arc<Context>,
) -> Result<impl Reply, Rejection> {
    Ok(file)
}

pub async fn delete(
    appid: String,
    name: String,
    req: Request,
    ctx: Arc<Context>,
) -> Result<impl Reply, Rejection> {
    if let UserKind::Master = req.user {
        trace!("delete file appid: {}, name: {}", appid, name);
        let mut file = File::new(
            &name,
            Bytes::default(),
            &appid,
            req.user.clone(),
            ctx.clone(),
        );
        if let Some(f) = &ctx.before_delete_file {
            file = f(file, req.clone(), ctx.clone()).await?;
        }
        // Delete
        let mut path = PathBuf::from("./files");
        path.push(appid);
        path.push(name);

        fs::remove_file(&path).map_or_else(|e| not_found("Failed to remove file"), |s| Ok(s))?;

        if let Some(f) = &ctx.after_delete_file {
            f(file, req.clone(), ctx.clone()).await?;
        }
        Ok("")
    } else {
        unauthorized("Only Master is allowed to delete files")
    }
}
