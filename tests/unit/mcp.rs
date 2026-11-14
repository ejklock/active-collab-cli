use super::*;
use serde_json::json;

fn operation_names(schema: &JsonObject) -> Vec<&str> {
    schema["properties"]["operation"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect()
}

#[test]
fn read_schema_is_one_flat_object_naming_every_read_operation() {
    let schema = input_schema::<ReadOperation>();

    assert_eq!(schema["type"], "object");
    assert!(!schema.contains_key("oneOf"));
    assert_eq!(schema["required"], json!(["operation"]));
    assert_eq!(operation_names(&schema), ["get", "current", "mine"]);
    assert!(schema["properties"]["ref"].is_object());
}

#[test]
fn write_schema_is_one_flat_object_naming_every_write_operation() {
    let schema = input_schema::<WriteOperation>();

    assert_eq!(schema["type"], "object");
    assert!(!schema.contains_key("oneOf"));
    assert_eq!(schema["required"], json!(["operation"]));
    assert_eq!(
        operation_names(&schema),
        ["comment", "log_time", "set_task", "time_report"]
    );
    assert!(schema["properties"]["hours"].is_object());
}

#[test]
#[should_panic(
    expected = "operation second declares field ref differently from an earlier operation"
)]
fn a_field_declared_differently_by_two_operations_panics_naming_both() {
    let variant = |name: &str, doc: &str| {
        json!({
            "properties": {
                "operation": { "const": name },
                "ref": { "type": "string", "description": doc }
            },
            "required": ["operation", "ref"]
        })
    };
    let mut schema = JsonObject::new();
    schema.insert(
        "oneOf".into(),
        json!([variant("first", "Task URL."), variant("second", "Task id.")]),
    );

    flatten_operations(&mut schema);
}
