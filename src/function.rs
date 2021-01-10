use std::{collections::HashMap, pin::Pin, sync::Arc};

use warp::{Future, Rejection, Reply};

use crate::{
    error::not_found,
    server::{Context, Request},
};

pub type HookFunc = Box<
    fn(&mut Request, Arc<Context>) -> Pin<Box<dyn Future<Output = Result<(), Rejection>> + Send>>,
>;
pub type Function = Box<
    fn(
        &mut Request,
        Arc<Context>,
        HashMap<String, String>,
    ) -> Pin<Box<dyn Future<Output = Result<String, Rejection>> + Send>>,
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
