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
        [
            "comment",
            "log_time",
            "set_task",
            "time_report",
            "attach_task"
        ]
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

fn read_operation(arguments: serde_json::Value) -> Result<ReadOperation, McpError> {
    let Value::Object(map) = arguments else {
        panic!("arguments must be an object");
    };
    parse_operation(map)
}

#[test]
fn mcp_mine_filter_accepts_labels_and_task_list() {
    let op = read_operation(json!({
        "operation": "mine",
        "labels": ["STATUS:GO LIVE", "ROTINA MENSAL"],
        "task_list": "Go live"
    }));
    let Ok(ReadOperation::Mine {
        labels, task_list, ..
    }) = op
    else {
        panic!("labels and task_list must be accepted by the mine operation");
    };
    assert_eq!(labels, ["STATUS:GO LIVE", "ROTINA MENSAL"]);
    assert_eq!(task_list.as_deref(), Some("Go live"));
}

#[test]
fn mcp_mine_filter_defaults_to_no_filter() {
    let Ok(ReadOperation::Mine {
        labels, task_list, ..
    }) = read_operation(json!({ "operation": "mine" }))
    else {
        panic!("a bare mine operation must parse");
    };
    assert!(labels.is_empty());
    assert!(task_list.is_none());
}

#[test]
fn mcp_mine_filter_rejects_labels_given_as_a_string() {
    let err = read_operation(json!({ "operation": "mine", "labels": "STATUS:GO LIVE" }))
        .err()
        .expect("a string is not an array of labels");
    assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
}

#[test]
fn mcp_mine_filter_still_rejects_an_unknown_argument() {
    let err = read_operation(json!({ "operation": "mine", "label": ["x"] }))
        .err()
        .expect("an unknown argument must be rejected");
    assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
}

#[test]
fn mcp_mine_filter_schema_describes_both_arguments() {
    let schema = input_schema::<ReadOperation>();
    for name in ["labels", "task_list"] {
        let description = schema["properties"][name]["description"].as_str();
        assert!(
            description.is_some_and(|d| !d.is_empty()),
            "{name} must be listed with a description"
        );
    }
    assert_eq!(schema["properties"]["labels"]["type"], "array");
}

#[test]
fn mcp_mine_filter_rejects_an_empty_label() {
    for labels in [json!([""]), json!(["STATUS:GO LIVE", ""])] {
        let err = read_operation(json!({ "operation": "mine", "labels": labels }))
            .err()
            .expect("an empty label must be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }
}

#[test]
fn mcp_mine_filter_rejects_an_empty_task_list() {
    let err = read_operation(json!({ "operation": "mine", "task_list": "" }))
        .err()
        .expect("an empty task list must be rejected");
    assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
}

#[test]
fn mcp_mine_filter_accepts_empty_array_and_a_blank_label() {
    let Ok(ReadOperation::Mine { labels, .. }) =
        read_operation(json!({ "operation": "mine", "labels": [] }))
    else {
        panic!("an empty labels array must be accepted");
    };
    assert!(labels.is_empty());
    let Ok(ReadOperation::Mine { labels, .. }) =
        read_operation(json!({ "operation": "mine", "labels": [" "] }))
    else {
        panic!("a blank label is not the empty string and must be accepted");
    };
    assert_eq!(labels, [" "]);
}

#[test]
fn mcp_get_no_dependencies_is_accepted_and_defaults_to_false() {
    let Ok(ReadOperation::Get {
        no_dependencies, ..
    }) = read_operation(json!({ "operation": "get", "ref": "5/10", "no_dependencies": true }))
    else {
        panic!("no_dependencies must be accepted by the get operation");
    };
    assert!(no_dependencies);

    let Ok(ReadOperation::Get {
        no_dependencies, ..
    }) = read_operation(json!({ "operation": "get", "ref": "5/10" }))
    else {
        panic!("a bare get operation must parse");
    };
    assert!(!no_dependencies);
}

#[test]
fn mcp_get_no_dependencies_rejects_a_string() {
    let err =
        read_operation(json!({ "operation": "get", "ref": "5/10", "no_dependencies": "yes" }))
            .err()
            .expect("a string is not a boolean");
    assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
}

#[test]
fn mcp_get_no_dependencies_schema_has_a_description() {
    let schema = input_schema::<ReadOperation>();
    let description = schema["properties"]["no_dependencies"]["description"].as_str();
    assert!(
        description.is_some_and(|d| !d.is_empty()),
        "no_dependencies must be listed with a description"
    );
}

#[test]
fn mcp_mine_phase_parses_a_key_and_defaults_to_no_filter() {
    let Ok(ReadOperation::Mine { phase, .. }) =
        read_operation(json!({ "operation": "mine", "phase": "in_review" }))
    else {
        panic!("a phase key must be accepted by the mine operation");
    };
    assert_eq!(phase, Some(crate::board::Phase::InReview));
    let Ok(ReadOperation::Mine { phase, .. }) = read_operation(json!({ "operation": "mine" }))
    else {
        panic!("a bare mine operation must parse");
    };
    assert_eq!(phase, None);
}

#[test]
fn mcp_mine_phase_rejects_an_unknown_key_and_a_non_string() {
    for phase in [json!("done"), json!("IN_REVIEW"), json!(1), json!("")] {
        let err = read_operation(json!({ "operation": "mine", "phase": phase }))
            .err()
            .expect("an invalid phase must be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }
}

#[test]
fn mcp_mine_phase_schema_lists_the_four_keys_with_a_description() {
    let schema = input_schema::<ReadOperation>();
    let phase = &schema["properties"]["phase"];
    assert!(
        phase["description"].as_str().is_some_and(|d| !d.is_empty()),
        "phase must be listed with a description"
    );
    let shown = phase.to_string();
    for key in ["to_do", "in_progress", "in_review", "ready_for_release"] {
        assert!(shown.contains(&format!("\"{key}\"")), "{shown}");
    }
}
