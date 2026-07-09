use super::{
    column_header_style, palette_for, set_active, theme_from_str, theme_to_str, Mode, ThemeChoice,
    ANGIE_DARK, ANGIE_LIGHT, NORD_DARK, NORD_LIGHT, SLATE_DARK, SLATE_LIGHT,
};

fn assert_palette_eq(actual: super::Palette, expected: super::Palette, label: &str) {
    assert_eq!(actual.accent, expected.accent, "{label}: accent mismatch");
    assert_eq!(
        actual.surface_base, expected.surface_base,
        "{label}: surface_base mismatch"
    );
    assert_eq!(actual.danger, expected.danger, "{label}: danger mismatch");
}

#[test]
fn palette_for_covers_all_six_theme_mode_pairs() {
    assert_palette_eq(
        palette_for(ThemeChoice::Angie, Mode::Dark),
        ANGIE_DARK,
        "angie/dark",
    );
    assert_palette_eq(
        palette_for(ThemeChoice::Angie, Mode::Light),
        ANGIE_LIGHT,
        "angie/light",
    );
    assert_palette_eq(
        palette_for(ThemeChoice::Slate, Mode::Dark),
        SLATE_DARK,
        "slate/dark",
    );
    assert_palette_eq(
        palette_for(ThemeChoice::Slate, Mode::Light),
        SLATE_LIGHT,
        "slate/light",
    );
    assert_palette_eq(
        palette_for(ThemeChoice::Nord, Mode::Dark),
        NORD_DARK,
        "nord/dark",
    );
    assert_palette_eq(
        palette_for(ThemeChoice::Nord, Mode::Light),
        NORD_LIGHT,
        "nord/light",
    );
}

#[test]
fn theme_from_str_maps_slate_aliases_case_insensitive_trimmed() {
    for alias in ["slate", "SLATE", " slate ", "slate-amber", "amber", "AMBER"] {
        assert_eq!(
            theme_from_str(alias),
            ThemeChoice::Slate,
            "alias {alias:?} must resolve to Slate"
        );
    }
}

#[test]
fn theme_from_str_maps_nord_aliases_case_insensitive_trimmed() {
    for alias in ["nord", "NORD", " nord ", "nord-frost", "frost", "FROST"] {
        assert_eq!(
            theme_from_str(alias),
            ThemeChoice::Nord,
            "alias {alias:?} must resolve to Nord"
        );
    }
}

#[test]
fn theme_from_str_falls_back_to_angie_for_unknown_or_empty() {
    for input in ["", "unknown", "ANGIE", " angie ", "xyz"] {
        assert_eq!(
            theme_from_str(input),
            ThemeChoice::Angie,
            "input {input:?} must fall back to Angie"
        );
    }
}

#[test]
fn theme_to_str_round_trips_for_canonical_names() {
    for canonical in ["angie", "slate", "nord"] {
        let round_tripped = theme_to_str(theme_from_str(canonical));
        assert_eq!(
            round_tripped, canonical,
            "round-trip for {canonical:?} must be stable"
        );
    }
}

// The only test in this module that mutates the process-wide active palette;
// it restores ANGIE_DARK before returning so no other test observes a stale
// palette (tests in this binary share one process).
#[test]
fn set_active_changes_builder_output_and_is_restored() {
    set_active(ThemeChoice::Slate, Mode::Dark);
    assert_eq!(
        column_header_style().fg,
        Some(SLATE_DARK.accent),
        "after set_active(Slate, Dark), column_header_style fg must be SLATE_DARK.accent"
    );

    set_active(ThemeChoice::Angie, Mode::Dark);
    assert_eq!(
        column_header_style().fg,
        Some(ANGIE_DARK.accent),
        "set_active(Angie, Dark) must restore the default palette"
    );
}
