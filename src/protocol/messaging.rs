extern crate log;


pub fn prepare_begin() -> serde_json::Value {
    serde_json::json!({
        "kind": "begin",
    })
}

pub fn prepare_end() -> serde_json::Value {
    serde_json::json!({
        "kind": "end",
    })
}


