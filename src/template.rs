use lazy_static::lazy_static;
use tera::{Tera, Value, Result as TeraResult};
use std::collections::HashMap;

lazy_static! {
    pub static ref TERA: Tera = {
        // Adjust the path to be relative to the project root where Cargo.toml is.
        let mut tera = match Tera::new("templates/**/*.html") {
            Ok(t) => t,
            Err(e) => {
                println!("Tera parsing error(s): {}", e);
                ::std::process::exit(1);
            }
        };
        
        // Register custom filters
        tera.register_filter("json", json_filter);
        
        tera
    };
}

// Custom json filter function
fn json_filter(value: &Value, _: &HashMap<String, Value>) -> TeraResult<Value> {
    match serde_json::to_string(value) {
        Ok(json_string) => Ok(Value::String(json_string)),
        Err(e) => Err(tera::Error::msg(format!("JSON serialization error: {}", e))),
    }
}

