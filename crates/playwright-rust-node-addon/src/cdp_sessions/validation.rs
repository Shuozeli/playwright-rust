//! Serialized CDP input validation and measured output ownership.

use playwright_rust_node_host::HostError;
use serde_json::Value;

pub fn parse_params_json(source: &str) -> Result<Value, HostError> {
    let params = serde_json::from_str::<Value>(source).map_err(|error| {
        HostError::InvalidArgument(format!("CDP params are not valid JSON: {error}"))
    })?;
    Ok(params)
}

pub fn measured_result(value: Value) -> Result<(Value, u64), HostError> {
    if !value.is_object() {
        return Err(HostError::Backend(
            "CDP command returned a non-object result".into(),
        ));
    }
    let bytes = serde_json::to_vec(&value)
        .map_err(|error| HostError::Backend(error.to_string()))?
        .len();
    let bytes = u64::try_from(bytes)
        .map_err(|_| HostError::ResourceExhausted("CDP result byte length overflow".into()))?;
    Ok((value, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_parser_accepts_input_beyond_the_retired_one_megabyte_limit() {
        // Arrange
        let source = format!(r#"{{"value":"{}"}}"#, "x".repeat(2 * 1024 * 1024));

        // Act
        let parsed = parse_params_json(&source).unwrap();

        // Assert
        assert_eq!(parsed["value"].as_str().unwrap().len(), 2 * 1024 * 1024);
    }

    #[test]
    fn params_parser_preserves_non_object_json_for_chromium_validation() {
        // Arrange
        let source = r#"["value", 7, null]"#;

        // Act
        let parsed = parse_params_json(source).unwrap();

        // Assert
        assert_eq!(parsed, serde_json::json!(["value", 7, null]));
    }
}
