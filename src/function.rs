use std::{collections::HashMap, pin::Pin, sync::Arc};

use warp::{Future, Rejection, Reply};

use crate::{
    error::not_found,
    server::{Context, Request},
};

pub type HookFunc = Box<
    fn(
        Request,
        Arc<Context>,
    ) -> Pin<Box<dyn Future<Output = Result<Request, Rejection>> + Send + 'static>>,
>;
pub type Function = Box<
    fn(
        &mut Request,
        Arc<Context>,
        HashMap<String, String>,
    ) -> Pin<Box<dyn Future<Output = Result<String, Rejection>> + Send + 'static>>,
>;
pub type HookMap = HashMap<String, HookFunc>;
pub type FuncMap = HashMap<String, Function>;

pub async fn run(
    name: String,
    arg: HashMap<String, String>,
    mut req: Request,
    ctx: Arc<Context>,
) -> Result<impl Reply, Rejection> {
    if let Some(f) = ctx.function.get(&name) {
        trace!("run function '{}', args {:?}", name, arg.clone());
        f(&mut req, ctx.clone(), arg).await
    } else {
        not_found(format!("function '{}' not found", name))
    }
}

#[cfg(test)]
mod test {
    use std::{collections::HashMap, convert::TryFrom, sync::{Arc, Mutex}, thread::sleep, time::Duration};

    use mongodb::bson::Document;
    use serde_json::{json, Map, Value};
    use warp::{hyper::StatusCode, Rejection};

    use crate::{error::bad_request, server::{Context, Request}};

    use super::super::tests::{test_api, test_server};
    use lazy_static::lazy_static;

    lazy_static! {
        static ref CNT: Mutex<i32> = Mutex::new(0);
    }

    async fn inc(req: Request, ctx: Arc<Context>) -> Result<Request, Rejection> {
        *CNT.lock().unwrap() += 1;
        Ok(req)
    }
    async fn dec(req: Request, ctx: Arc<Context>) -> Result<Request, Rejection> {
        *CNT.lock().unwrap() -= 1;
        Ok(req)
    }
    

    async fn test_err(req: Request, ctx: Arc<Context>) -> Result<Request, Rejection> {
        bad_request("test err")
    }

    #[tokio::test]
    async fn test_hooks() {
        let create1 = async move |api, class, body| {
            warp::test::request()
                .method("POST")
                .path(&format!("/classes/{}", class))
                .json(&body)
                .reply(api)
                .await
        };
        let delete1 = async move |api, class, id| {
            warp::test::request()
                .method("DELETE")
                .path(&format!("/classes/{}/{}", class, id))
                .reply(api)
                .await
        };


        // let retrieve1 = async move |api, class, id| {
        //     warp::test::request()
        //         .method("GET")
        //         .path(&format!("/classes/{}/{}", class, id))
        //         .reply(api)
        //         .await
        // };

        // let update1 = async move |api, class, id, body| {
        //     warp::test::request()
        //         .method("PUT")
        //         .path(&format!("/classes/{}/{}", class, id))
        //         .json(&body)
        //         .reply(api)
        //         .await
        // };

        let mut ids = Vec::new();
        // Test before save.
        let mut s = test_server().await;
        s.before_save("foo", Box::new(|req, ctx| Box::pin(inc(req, ctx))));

        let api = s.routes().await;
        let resp = create1(&api, "foo", json!({"name": "a"})).await;
        let body = Document::try_from(
            serde_json::from_slice::<Map<String, Value>>(&resp.body()[..]).unwrap(),
        )
        .unwrap();
        ids.push(body.get_str("objectId").unwrap());
        assert_eq!(resp.status(), StatusCode::CREATED);
        let resp = create1(&api, "foo", json!({"name": "b"})).await;
        let body = Document::try_from(
            serde_json::from_slice::<Map<String, Value>>(&resp.body()[..]).unwrap(),
        )
        .unwrap();
        ids.push(body.get_str("objectId").unwrap());
        assert_eq!(resp.status(), StatusCode::CREATED);
        let resp = create1(&api, "foo", json!({"name": "c"})).await;
        let body = Document::try_from(
            serde_json::from_slice::<Map<String, Value>>(&resp.body()[..]).unwrap(),
        )
        .unwrap();
        ids.push(body.get_str("objectId").unwrap());
        assert_eq!(resp.status(), StatusCode::CREATED);

        assert_eq!(*CNT.lock().unwrap(), ids.len() as i32);

        // Test after save.
        s.after_save("foo", Box::new(|req, ctx| Box::pin(test_err(req, ctx))));
        let api = s.routes().await;
        let resp = create1(&api, "foo", json!({"name": "a"})).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        *CNT.lock().unwrap() -= 1;
        assert_eq!(*CNT.lock().unwrap(), ids.len() as i32);     

        // Test before/after destroy.
        s.before_destroy("foo", Box::new(|req, ctx| Box::pin(dec(req, ctx))));
        s.after_destroy("foo", Box::new(|req, ctx| Box::pin(dec(req, ctx))));
        let api = s.routes().await;

        for id in ids.iter() {
            let resp = delete1(&api, "foo", id).await;
            assert_eq!(resp.status(), StatusCode::OK);
        }
        assert_eq!(*CNT.lock().unwrap(), -(ids.len() as i32));

    }
}
