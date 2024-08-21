use log::info;
use serde_json::{json, Value};

// This function generates a JSON response with a single key-value pair.
// It also includes logging for successful response creation.
pub fn hex_response(key: &str, value: &str) -> Value {
    // Log the attempt to create a JSON response.
    info!(
        "Creating JSON response for key: {} with value: {}",
        key, value
    );

    json!({ key: value })
}
