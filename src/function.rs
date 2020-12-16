use std::collections::HashMap;

type HookMap = HashMap<String, Vec<fn() -> i32>>;
type Inits = Vec<fn() -> i32>;
type CodeMap = HashMap<String, Vec<fn() -> i32>>;

pub struct Function {}

#[derive(Debug, Clone, Default)]
pub struct Functions {
    before_save: HookMap,
    after_save: HookMap,
    before_delete: HookMap,
    after_delete: HookMap,
    init: Inits,
    cloud_code: CodeMap,
}
