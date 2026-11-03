use super::{Colors, Preset, ResolvedTheme, Selection, MAX_THEME_BYTES};
use iced::Color;

#[test]
fn sparse_overrides_inherit_a_named_preset_and_resolve_deterministically() {
    for (base, preset) in [
        ("dark", Preset::Dark),
        ("light", Preset::Light),
        ("catppuccin_mocha", Preset::CatppuccinMocha),
    ] {
        let source =
            format!("version=1\nbase='{base}'\n[colors]\naccent='#a7d9ff'\naccent_text='#101c28'");
        let first = ResolvedTheme::parse(&source, 5).expect("valid sparse theme");
        let second = ResolvedTheme::parse(&source, 6).expect("same theme");
        assert_eq!(first.colors, second.colors);
        assert_eq!(first.derived, second.derived);
        assert_eq!(first.colors.background, Colors::preset(preset).background);
        assert_eq!(first.colors.accent, super::rgb(0xa7d9ff));
        assert_eq!(first.colors.accent_strong, first.colors.accent);
        assert_eq!(first.iced.palette(), second.iced.palette());
        assert_eq!((first.revision, second.revision), (5, 6));
    }
}

#[test]
fn invalid_version_unknown_fields_types_and_colors_name_the_problem() {
    for (source, field) in [
        ("version=2\nbase='dark'", "version"),
        ("version=1\nbase='other.toml'", "base"),
        ("version=1\nbase='dark'\nparent='file.toml'", "parent"),
        (
            "version=1\nbase='dark'\n[colors]\nbutton='#123456'",
            "button",
        ),
        (
            "version=1\nbase='dark'\n[colors]\ntext='#abc'",
            "colors.text",
        ),
        (
            "version=1\nbase='dark'\n[colors]\ntext='#12345680'",
            "colors.text",
        ),
        (
            "version=1\nbase='dark'\n[colors]\ntext='#gggggg'",
            "colors.text",
        ),
        ("version=1\nbase='dark'\n[colors]\ntext=42", "text"),
    ] {
        assert!(
            ResolvedTheme::parse(source, 0)
                .expect_err("invalid theme")
                .contains(field),
            "{source}"
        );
    }
}

#[test]
fn shared_resonant_role_names_are_valid_and_literal_colors_are_not_corrected() {
    let theme=ResolvedTheme::parse("version=1\nbase='light'\n[colors]\ntext='#000000'\nplayhead='#abcdef'\ndrums='#112233'\nmeter_clip='#000000'",0).expect("shared schema");
    assert_eq!(theme.colors.text, Color::BLACK);
    assert_eq!(theme.colors.playhead, super::rgb(0xabcdef));
    assert_eq!(theme.colors.drums, super::rgb(0x112233));
    assert_eq!(
        theme.iced.extended_palette().background.base.text,
        Color::BLACK
    );
}

#[test]
fn built_in_text_and_primary_actions_have_accessible_contrast() {
    for preset in Preset::ALL {
        let resolved = ResolvedTheme::preset(preset, 1);
        let colors = resolved.colors;
        for surface in [
            colors.background,
            colors.chrome,
            colors.panel,
            colors.raised,
            resolved.derived.hover,
            resolved.derived.pressed,
            resolved.derived.selection_fill,
        ] {
            assert!(
                super::contrast(colors.text, surface) >= 4.5,
                "{preset} text"
            );
            assert!(
                super::contrast(colors.text_muted, surface) >= 4.5,
                "{preset} secondary text: {}",
                super::contrast(colors.text_muted, surface)
            );
        }
        assert!(
            super::contrast(colors.accent, colors.panel) >= 4.5,
            "{preset} accent"
        );
        assert!(
            super::contrast(colors.accent_text, colors.accent) >= 4.5,
            "{preset} primary action"
        );
        assert!(
            super::contrast(colors.accent_text, colors.accent_strong) >= 4.5,
            "{preset} strong action"
        );
    }
}

#[test]
fn extended_palette_retains_each_authored_surface_and_primary_pair() {
    for preset in Preset::ALL {
        let resolved = ResolvedTheme::preset(preset, 0);
        let palette = resolved.iced.extended_palette();
        assert_eq!(palette.background.weak.color, resolved.colors.panel);
        assert_eq!(palette.background.neutral.color, resolved.colors.raised);
        assert_eq!(palette.background.weakest.color, resolved.colors.chrome);
        assert_eq!(palette.primary.base.color, resolved.colors.accent);
        assert_eq!(palette.primary.base.text, resolved.colors.accent_text);
    }
}

#[test]
fn imported_theme_survives_original_deletion_and_round_trips_preferences() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("Studio.toml");
    std::fs::write(
        &path,
        "version=1\nbase='dark'\n[colors]\naccent='#a7d9ff'\naccent_text='#101c28'",
    )
    .expect("theme fixture");
    let selected = Selection::import(&path).expect("import theme");
    std::fs::remove_file(&path).expect("remove original");
    let persisted = serde_json::to_vec(&selected).expect("serialize selection");
    let restored: Selection = serde_json::from_slice(&persisted).expect("restore selection");
    assert_eq!(selected, restored);
    assert_eq!(restored.label(), "Studio");
    assert_eq!(
        restored.resolve(3).expect("embedded theme").colors.accent,
        super::rgb(0xa7d9ff)
    );
}

#[test]
fn oversized_and_non_utf8_imports_fail_without_unbounded_reads() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("large.toml");
    std::fs::write(&path, vec![b' '; MAX_THEME_BYTES + 1]).expect("oversized fixture");
    assert!(Selection::import(&path)
        .expect_err("size limit")
        .contains("64 KiB"));
    assert!(ResolvedTheme::parse(&" ".repeat(MAX_THEME_BYTES + 1), 0).is_err());
    std::fs::write(&path, [0xff, 0xfe]).expect("binary fixture");
    assert!(Selection::import(&path)
        .expect_err("text format")
        .contains("UTF-8"));
}

#[test]
fn bundled_examples_follow_the_shared_theme_format() {
    for source in [
        include_str!("../../../../assets/themes/studio.toml"),
        include_str!("../../../../assets/themes/catppuccin-mocha.toml"),
    ] {
        ResolvedTheme::parse(source, 0).expect("bundled example");
    }
}
