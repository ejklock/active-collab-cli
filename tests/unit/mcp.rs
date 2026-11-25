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

#[test]
fn heartbeat_progress_rises_with_every_beat_and_stays_below_the_total() {
    let beats = [1_u32, 2, 98, 99, 100, 101, 1000, 100_000];
    let values: Vec<f64> = beats.iter().map(|n| heartbeat_progress(*n)).collect();
    assert!(
        values.windows(2).all(|pair| pair[0] < pair[1]),
        "{values:?}"
    );
    assert!(values.iter().all(|v| *v < PROGRESS_TOTAL), "{values:?}");
}
