use warp::Reply;
use warp::{
    hyper::StatusCode,
    reject::{self, Reject},
    Rejection,
};

use paste::paste;

#[derive(Debug)]
struct Error {
    error: String,
    code: StatusCode,
}

#[derive(serde::Serialize)]
struct ErrorMessage {
    code: u16,
    error: String,
}

impl Reject for Error {}

macro_rules! err1 {
    ($e:ident, $se:expr) => {

        #[doc = "Rejection with error `"]
        #[doc = $se]
        #[doc = "`"]
        pub fn $e<T>(msg: impl Into<String>) -> Result<T, Rejection> {
            Err(reject::custom(Error {
                error: msg.into(),
                code: paste! { StatusCode:: [<$e:upper>] },
            }))
        }
    };
}
macro_rules! err {
    ($e:ident) => {
        err1!($e, stringify!($e));
    };
}

err!(unauthorized);
err!(bad_request);
err!(internal_server_error);
err!(not_found);
err!(conflict);

pub(crate) async fn handle_rejection(r: Rejection) -> Result<impl Reply, Rejection> {
    if let Some(err) = r.find::<Error>() {
        let json = warp::reply::json(&ErrorMessage {
            code: err.code.as_u16(),
            error: err.error.clone(),
        });
        Ok(warp::reply::with_status(json, err.code))
    } else {
        error!("{:?}", r);
        Err(r)
    }
}
