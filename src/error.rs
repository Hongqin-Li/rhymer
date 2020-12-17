use warp::Reply;
use warp::{
    hyper::StatusCode,
    reject::{self, Reject},
    Rejection,
};

#[derive(Debug)]
struct Error {
    message: String,
    code: StatusCode,
}

#[derive(serde::Serialize)]
struct ErrorMessage {
    code: u16,
    message: String,
}

impl Reject for Error {}

pub fn unauthorized<T>(msg: impl Into<String>) -> Result<T, Rejection> {
    Err(reject::custom(Error {
        message: msg.into(),
        code: StatusCode::UNAUTHORIZED,
    }))
}

pub fn bad_request<T>(msg: impl Into<String>) -> Result<T, Rejection> {
    Err(reject::custom(Error {
        message: msg.into(),
        code: StatusCode::BAD_REQUEST,
    }))
}

pub fn internal_server_error<T>(msg: impl Into<String>) -> Result<T, Rejection> {
    Err(reject::custom(Error {
        message: msg.into(),
        code: StatusCode::INTERNAL_SERVER_ERROR,
    }))
}

pub fn not_found<T>(msg: impl Into<String>) -> Result<T, Rejection> {
    Err(reject::custom(Error {
        message: msg.into(),
        code: StatusCode::NOT_FOUND,
    }))
}

pub async fn handle_rejection(r: Rejection) -> Result<impl Reply, Rejection> {
    trace!("{:?}", r);
    if let Some(err) = r.find::<Error>() {
        let json = warp::reply::json(&ErrorMessage {
            code: err.code.as_u16(),
            message: err.message.clone(),
        });
        Ok(warp::reply::with_status(json, err.code))
    } else {
        Err(r)
    }
}
