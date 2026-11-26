use super::*;

fn labels(names: &[&str]) -> Vec<Label> {
    names
        .iter()
        .enumerate()
        .map(|(id, name)| Label {
            id: i64::try_from(id).unwrap(),
            name: (*name).to_owned(),
        })
        .collect()
}

fn placed(names: &[&str], list: Option<&str>) -> Placement {
    place(&labels(names), list, &BoardMapping::default())
}

#[test]
fn phase_keys_round_trip_in_display_order() {
    let keys: Vec<&str> = Phase::ALL.iter().map(|p| p.key()).collect();
    assert_eq!(
        keys,
        ["to_do", "in_progress", "in_review", "ready_for_release"]
    );
    for phase in Phase::ALL {
        assert_eq!(Phase::from_key(phase.key()), Some(phase));
    }
    assert_eq!(Phase::from_key("done"), None);
    assert_eq!(Phase::from_key("IN_REVIEW"), None);
    assert_eq!(Phase::default(), Phase::ToDo);
}

#[test]
fn phase_serializes_as_its_key() {
    assert_eq!(
        serde_json::to_string(&Phase::ReadyForRelease).unwrap(),
        "\"ready_for_release\""
    );
}

#[test]
fn status_prefix_accepts_spaces_and_any_case() {
    assert_eq!(
        placed(&["STATUS: GO LIVE"], None).phase,
        Phase::ReadyForRelease
    );
    assert_eq!(
        placed(&["STATUS:GO LIVE"], None).phase,
        Phase::ReadyForRelease
    );
    assert_eq!(
        placed(&["  status:go live "], None).phase,
        Phase::ReadyForRelease
    );
}

#[test]
fn a_label_without_the_status_prefix_never_counts() {
    assert_eq!(
        placed(&["TAMANHO:P", "GO LIVE"], None),
        Placement::default()
    );
    let flag_like = placed(&["BLOQUEADO"], None);
    assert!(!flag_like.flagged);
}

#[test]
fn the_furthest_matching_status_label_wins() {
    let placement = placed(&["STATUS:DESENVOLVIMENTO", "STATUS:VALIDAÇÃO"], None);
    assert_eq!(placement.phase, Phase::InReview);
}

#[test]
fn a_status_label_beats_the_list() {
    assert_eq!(
        placed(&["STATUS:ESTIMATIVA"], Some("Go live")).phase,
        Phase::ToDo
    );
}

#[test]
fn a_flag_only_label_flags_without_deciding_the_phase() {
    let placement = placed(&["STATUS:AGUARDANDO CLIENTE"], Some("Review"));
    assert_eq!(
        placement,
        Placement {
            phase: Phase::InReview,
            flagged: true
        }
    );
}

#[test]
fn the_list_decides_when_no_status_label_does() {
    assert_eq!(
        placed(&[], Some("✅ Ready to go")).phase,
        Phase::ReadyForRelease
    );
    assert_eq!(placed(&[], Some("📝 To do")).phase, Phase::ToDo);
    assert_eq!(placed(&[], Some("Backlog Doing")).phase, Phase::InProgress);
}

#[test]
fn an_unknown_or_missing_list_is_to_do() {
    assert_eq!(placed(&[], Some("Acompanhamento")), Placement::default());
    assert_eq!(placed(&[], None), Placement::default());
}

#[test]
fn keywords_are_trimmed_and_lowercased_on_both_sides() {
    let mut mapping = BoardMapping::default();
    mapping.in_review.labels = vec!["  ANÁLISE Extra ".to_owned()];
    let placement = place(&labels(&["STATUS: análise EXTRA final"]), None, &mapping);
    assert_eq!(placement.phase, Phase::InReview);
}

#[test]
fn an_empty_keyword_matches_nothing() {
    let mut mapping = BoardMapping::default();
    mapping.in_progress.lists = vec![String::new(), "  ".to_owned()];
    assert_eq!(place(&[], Some("anything"), &mapping).phase, Phase::ToDo);
}

#[test]
fn a_mapping_round_trips_through_json() {
    let mapping = BoardMapping::default();
    let json = serde_json::to_string(&mapping).unwrap();
    assert_eq!(
        serde_json::from_str::<BoardMapping>(&json).unwrap(),
        mapping
    );
}

fn settings_with(value: Option<&str>) -> (tempfile::TempDir, crate::store::Store) {
    let dir = tempfile::TempDir::new().unwrap();
    let config = crate::config::Config {
        db_path: dir.path().join("board.db"),
        task_cache_ttl_hours: 24,
    };
    let store = crate::store::Store::open(&config).unwrap();
    if let Some(value) = value {
        crate::store::settings::SettingsRepository::new(store.conn())
            .set("board_mapping", value)
            .unwrap();
    }
    (dir, store)
}

#[test]
fn load_without_a_stored_value_gives_the_defaults() {
    let (_dir, store) = settings_with(None);
    let settings = crate::store::settings::SettingsRepository::new(store.conn());
    assert_eq!(load(&settings), BoardMapping::default());
}

#[test]
fn load_of_a_value_that_does_not_parse_gives_the_defaults() {
    for broken in ["not json", "[]", r#"{"to_do":5}"#, ""] {
        let (_dir, store) = settings_with(Some(broken));
        let settings = crate::store::settings::SettingsRepository::new(store.conn());
        assert_eq!(load(&settings), BoardMapping::default(), "{broken}");
    }
}

#[test]
fn load_fills_a_missing_field_with_its_default() {
    let (_dir, store) = settings_with(Some(r#"{"flag_labels":["espera"]}"#));
    let settings = crate::store::settings::SettingsRepository::new(store.conn());
    let loaded = load(&settings);
    assert_eq!(loaded.flag_labels, ["espera"]);
    assert_eq!(loaded.in_review, BoardMapping::default().in_review);
}

#[test]
fn keywords_split_on_commas_trim_and_drop_empty_entries() {
    assert_eq!(parse_keywords("to test, homolog ,"), ["to test", "homolog"]);
    assert!(parse_keywords("").is_empty());
    assert!(parse_keywords(" , ,").is_empty());
}

fn checked(value: Option<&str>) -> StoredMapping {
    let (_dir, store) = settings_with(value);
    load_checked(&crate::store::settings::SettingsRepository::new(
        store.conn(),
    ))
}

#[test]
fn board_overrides_absent_or_empty_object_give_the_built_ins_as_valid() {
    for stored in [None, Some("{}")] {
        let loaded = checked(stored);
        assert_eq!(loaded.mapping, BoardMapping::default(), "{stored:?}");
        assert!(!loaded.invalid, "{stored:?}");
    }
}

#[test]
fn board_overrides_one_column_list_leaves_every_other_field_built_in() {
    let loaded = checked(Some(r#"{"to_do":{"lists":["inbox"]}}"#));
    let built_in = BoardMapping::default();
    assert!(!loaded.invalid);
    assert_eq!(loaded.mapping.to_do.lists, ["inbox"]);
    assert_eq!(loaded.mapping.to_do.labels, built_in.to_do.labels);
    assert_eq!(loaded.mapping.in_progress, built_in.in_progress);
    assert_eq!(loaded.mapping.in_review, built_in.in_review);
    assert_eq!(loaded.mapping.ready_for_release, built_in.ready_for_release);
    assert_eq!(loaded.mapping.flag_labels, built_in.flag_labels);
}

#[test]
fn board_overrides_empty_flag_labels_clear_the_flag_keywords() {
    let loaded = checked(Some(r#"{"flag_labels":[]}"#));
    assert!(!loaded.invalid);
    assert!(loaded.mapping.flag_labels.is_empty());
    assert_eq!(loaded.mapping.to_do, BoardMapping::default().to_do);
}

#[test]
fn board_overrides_a_field_that_is_not_an_array_of_strings_is_invalid_and_keeps_the_built_in() {
    for bad in [
        r#"{"to_do":{"lists":"inbox"},"flag_labels":["espera"]}"#,
        r#"{"to_do":{"lists":[1]},"flag_labels":["espera"]}"#,
    ] {
        let loaded = checked(Some(bad));
        assert!(loaded.invalid, "{bad}");
        assert_eq!(
            loaded.mapping.to_do.lists,
            BoardMapping::default().to_do.lists,
            "{bad}"
        );
        assert_eq!(loaded.mapping.flag_labels, ["espera"], "{bad}");
    }
}

#[test]
fn board_overrides_a_stored_value_that_is_not_an_object_is_invalid_with_the_built_ins() {
    for bad in ["not json", "[1]"] {
        let loaded = checked(Some(bad));
        assert!(loaded.invalid, "{bad}");
        assert_eq!(loaded.mapping, BoardMapping::default(), "{bad}");
    }
}

#[test]
fn board_overrides_reach_only_the_fields_they_name_when_the_built_ins_change() {
    let loaded = checked(Some(r#"{"in_review":{"labels":["qa"]}}"#));
    // Expected values come from the running version's defaults, not literals.
    let built_in = BoardMapping::default();
    assert_eq!(loaded.mapping.in_review.labels, ["qa"]);
    assert_eq!(loaded.mapping.in_review.lists, built_in.in_review.lists);
    assert_eq!(loaded.mapping.to_do, built_in.to_do);
    assert_eq!(loaded.mapping.flag_labels, built_in.flag_labels);
}
