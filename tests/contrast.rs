//! WCAG contrast guarantees of generated themes, for every accent, gray
//! tint and mode.

use rust_ui::prelude::*;
use rust_ui::Contrast;

fn themes(contrast: Contrast) -> Vec<(String, Theme)> {
    let mut out = Vec::new();
    for dark in [true, false] {
        for accent in Accent::ALL {
            for gray in
                [GrayTint::Gray, GrayTint::Slate, GrayTint::Zinc, GrayTint::Mauve, GrayTint::Sand, GrayTint::Sage]
            {
                let t = Theme::from_config(ThemeConfig {
                    dark,
                    accent: accent.color(dark),
                    gray,
                    contrast,
                    ..ThemeConfig::dark()
                });
                out.push((format!("{accent:?}/{gray:?}/{}", if dark { "dark" } else { "light" }), t));
            }
        }
    }
    out
}

#[track_caller]
fn at_least(name: &str, what: &str, fg: Color, bg: Color, ratio: f32) {
    let c = fg.contrast(bg);
    assert!(c >= ratio - 0.01, "{name}: {what} is {c:.2}:1, needs {ratio}:1");
}

#[test]
fn high_contrast_meets_wcag_everywhere() {
    for (name, t) in themes(Contrast::High) {
        let c = &t.colors;
        assert!(t.shadow_popover.is_empty() && t.shadow_sm.is_empty(), "{name}: no shadows");
        for (sname, surface) in [
            ("background", c.background),
            ("surface", c.surface),
            ("panel", c.panel),
            ("elevated", c.elevated),
            ("input", c.input),
        ] {
            at_least(&name, &format!("text on {sname}"), c.text, surface, 7.0);
            at_least(&name, &format!("text_muted on {sname}"), c.text_muted, surface, 7.0);
            at_least(&name, &format!("text_faint on {sname}"), c.text_faint, surface, 4.5);
            at_least(&name, &format!("accent on {sname}"), c.accent, surface, 4.5);
            at_least(&name, &format!("code_text on {sname}"), c.code_text, surface, 7.0);
            at_least(&name, &format!("danger on {sname}"), c.danger, surface, 4.5);
            at_least(&name, &format!("success on {sname}"), c.success, surface, 4.5);
            at_least(&name, &format!("warning on {sname}"), c.warning, surface, 4.5);
            at_least(&name, &format!("border on {sname}"), c.border, surface, 3.0);
            at_least(&name, &format!("focus ring on {sname}"), c.focus_ring, surface, 3.0);
        }
        at_least(&name, "accent_text on accent", c.accent_text, c.accent, 4.5);
        at_least(&name, "danger_text on danger", c.danger_text, c.danger, 4.5);
        assert_eq!(c.border.a, 1.0, "{name}: opaque borders");
    }
}

#[test]
fn normal_themes_meet_wcag_aa_for_text() {
    for (name, t) in themes(Contrast::Normal) {
        let c = &t.colors;
        for (sname, surface) in [("background", c.background), ("surface", c.surface), ("panel", c.panel)] {
            at_least(&name, &format!("text on {sname}"), c.text, surface, 7.0);
            at_least(&name, &format!("text_muted on {sname}"), c.text_muted, surface, 4.5);
        }
        // Buttons: WCAG's 3:1 for large/bold UI text (the rule the palette uses).
        at_least(&name, "accent_text on accent", c.accent_text, c.accent, 3.0);
        at_least(&name, "danger_text on danger", c.danger_text, c.danger, 3.0);
    }
}

#[test]
fn contrast_is_a_knob() {
    let t = Theme::dark().with_accent(hex("#f59e0b")).with_contrast(Contrast::High);
    assert_eq!(t.name, "High Contrast Dark");
    assert_eq!(t.config.contrast, Contrast::High);
    // Other knobs keep it.
    assert_eq!(t.with_dark(false).name, "High Contrast Light");
}
