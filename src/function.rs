use std::{collections::HashMap, pin::Pin, sync::Arc};

use warp::{Future, Rejection, Reply};

use crate::{
    error::not_found,
    file::File,
    server::{Context, Request},
};

pub type HookFunc = Box<
    fn(
        Request,
        Arc<Context>,
    ) -> Pin<Box<dyn Future<Output = Result<Request, Rejection>> + Send + 'static>>,
>;
pub type FileHook = Box<
    fn(
        File,
        Request,
        Arc<Context>,
    ) -> Pin<Box<dyn Future<Output = Result<File, Rejection>> + Send + 'static>>,
>;
pub type Function = Box<
    fn(
        Request,
        Arc<Context>,
        HashMap<String, String>,
    ) -> Pin<Box<dyn Future<Output = Result<String, Rejection>> + Send + 'static>>,
>;
pub type HookMap = HashMap<String, HookFunc>;
pub type FuncMap = HashMap<String, Function>;

pub async fn run(
    name: String,
    arg: HashMap<String, String>,
    req: Request,
    ctx: Arc<Context>,
) -> Result<impl Reply, Rejection> {
    if let Some(f) = ctx.function.get(&name) {
        trace!("run function '{}', args {:?}", name, arg.clone());
        f(req, ctx.clone(), arg).await
    } else {
        not_found(format!("function '{}' not found", name))
    }
}

pub async fn run_post(
    name: String,
    arg: HashMap<String, String>,
    req: Request,
    ctx: Arc<Context>,
) -> Result<impl Reply, Rejection> {
    if let Some(f) = ctx.function.get(&name) {
        trace!("run function '{}', args {:?}", name, arg.clone());
        f(req, ctx.clone(), arg).await
    } else {
        not_found(format!("function '{}' not found", name))
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::HashMap,
        convert::TryFrom,
        fs,
        path::PathBuf,
        sync::{Arc, Mutex},
        thread::sleep,
        time::Duration,
    };

    use mongodb::bson::Document;
    use serde_json::{json, Map, Value};
    use warp::{hyper::StatusCode, Rejection};

    use crate::{
        error::bad_request,
        file::File,
        server::{Context, Request},
        tests::TEST_SERVER_KEY,
        with_user,
    };

    use super::super::tests::{test_api, test_server};
    use lazy_static::lazy_static;

    lazy_static! {
        static ref CNT: Mutex<i32> = Mutex::new(0);
    }
    fn reset_cnt() {
        *CNT.lock().unwrap() = 0;
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

    async fn test_file(f: File, req: Request, ctx: Arc<Context>) -> Result<File, Rejection> {
        *CNT.lock().unwrap() += 1;
        Ok(f)
    }

    #[tokio::test]
    async fn test_hooks() {
        reset_cnt();

        let create1 = async move |api, class, body| {
            with_user!("foo", "POST")
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

    #[tokio::test]
    async fn test_file_hooks() {
        reset_cnt();
        let appid = "test-appid";

        let create1 = async move |api, name, body| {
            with_user!("foo", "POST")
                .header("x-parse-application-id", appid)
                .path(&format!("/files/{}", name))
                .body(&body)
                .reply(api)
                .await
        };
        let delete1 = async move |api, appid, fname| {
            warp::test::request()
                .header("x-parse-master-key", TEST_SERVER_KEY)
                .method("DELETE")
                .path(&format!("/files/{}/{}", appid, fname))
                .reply(api)
                .await
        };

        // Test before/after save file.
        let mut s = test_server().await;
        s.before_save_file(Box::new(|f, req, ctx| Box::pin(test_file(f, req, ctx))));
        s.after_save_file(Box::new(|f, req, ctx| Box::pin(test_file(f, req, ctx))));
        s.before_delete_file(Box::new(|f, req, ctx| Box::pin(test_file(f, req, ctx))));
        s.after_delete_file(Box::new(|f, req, ctx| Box::pin(test_file(f, req, ctx))));

        let api = s.routes().await;
        let content = json!({"name": "a"}).to_string();
        let resp = create1(&api, "foo", &content).await;
        let body = Document::try_from(
            serde_json::from_slice::<Map<String, Value>>(&resp.body()[..]).unwrap(),
        )
        .unwrap();
        dbg!(&resp);
        assert_eq!(resp.status(), StatusCode::CREATED);
        assert!(body.get("url").is_some());
        assert!(body.get("name").is_some());
        assert_eq!(*CNT.lock().unwrap(), 2);

        let url = body.get_str("url").unwrap();
        let mut url_paths: Vec<&str> = url.split('/').collect();
        assert!(url_paths.len() > 3);
        let url_fname = url_paths.pop().unwrap();
        let url_appid = url_paths.pop().unwrap();
        assert_eq!(url_appid, appid);

        let p = PathBuf::from(format!("./files/{}/{}", &url_appid, &url_fname));
        let s = fs::read_to_string(dbg!(&p)).expect("failed to read file");
        assert_eq!(s, content);

        // Test before/after delete file.
        let resp = delete1(&api, url_appid, url_fname).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(*CNT.lock().unwrap(), 4);
        assert_eq!(p.exists(), false);
    }

    async fn test_f(
        req: Request,
        ctx: Arc<Context>,
        arg: HashMap<String, String>,
    ) -> Result<String, Rejection> {
        Ok(arg.get("bar").map_or("none".to_string(), |s| s.to_owned()))
    }

    #[tokio::test]
    async fn test_func() {
        let mut s = test_server().await;
        s.define(
            "foo",
            Box::new(|req, ctx, arg| Box::pin(test_f(req, ctx, arg))),
        );

        let api = s.routes().await;

        let invoke1 = async move |api, name, query| {
            warp::test::request()
                .method("GET")
                .path(&format!("/functions/{}?{}", name, query))
                .reply(api)
                .await
        };
        let invoke1_post = async move |api, name, query| {
            warp::test::request()
                .method("POST")
                .path(&format!("/functions/{}", name))
                .body(&query)
                .reply(api)
                .await
        };

        let resp = invoke1(&api, "xxx", "").await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let resp = invoke1(&api, "foo", "a=1&bar=2").await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(String::from_utf8(resp.body()[..].to_vec()).unwrap(), "2");

        let resp = invoke1_post(&api, "foo", json!({"bar": "2"}).to_string()).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(String::from_utf8(resp.body()[..].to_vec()).unwrap(), "2");
    }
}
