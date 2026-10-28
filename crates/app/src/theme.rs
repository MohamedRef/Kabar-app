//! Procedural batik-inspired backgrounds. Original geometry generated in
//! code (kawung-like circles), never copied artwork.

use kabar_core::theme::{self as palette, Palette};

/// CSS for every theme: the colour tokens from `kabar_core::theme`, plus a
/// `--batik` background tile drawn in each theme's pattern colour.
pub fn stylesheet() -> String {
    let batik = |id: &str, p: &Palette| {
        let tile = if p.is_true_black() {
            "none".to_string()
        } else {
            kawung_tile(p.pattern.trim_start_matches('#'), p.pattern_opacity, 72)
        };
        format!(".app[data-theme=\"{id}\"] {{ --batik: {tile}; }}\n")
    };
    let mut css = palette::stylesheet();
    css.push_str(&palette::rules_for_all(batik));
    css
}

/// A seamless kawung-like tile as an SVG `data:` URL for CSS `background-image`.
/// `color` is a CSS color without `#` (e.g. "2b3a67").
pub fn kawung_tile(color: &str, opacity: f32, size: u32) -> String {
    let s = size as f32;
    let (h, r) = (s / 2.0, s * 0.23);
    // Four petals (ellipses) meeting at the tile centre and at the corners.
    let petal = |cx: f32, cy: f32, rot: i32| {
        format!(
            "<ellipse cx='{cx:.1}' cy='{cy:.1}' rx='{r:.1}' ry='{ry:.1}' transform='rotate({rot} {cx:.1} {cy:.1})'/>",
            ry = r * 0.55
        )
    };
    let d = r * 0.72;
    let mut petals = String::new();
    for (cx, cy) in [(0.0, 0.0), (h, h), (s, 0.0), (0.0, s), (s, s)] {
        petals.push_str(&petal(cx - d, cy - d, 45));
        petals.push_str(&petal(cx + d, cy - d, -45));
        petals.push_str(&petal(cx - d, cy + d, -45));
        petals.push_str(&petal(cx + d, cy + d, 45));
    }
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{size}' height='{size}'>\
         <g fill='none' stroke='%23{color}' stroke-opacity='{opacity}' stroke-width='1.2'>{petals}\
         <circle cx='{h:.1}' cy='{h:.1}' r='{dot:.1}'/></g></svg>",
        dot = s * 0.04
    );
    format!("url(\"data:image/svg+xml;utf8,{svg}\")")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_is_a_css_data_url_without_raw_hashes() {
        let tile = kawung_tile("2b3a67", 0.08, 64);
        assert!(tile.starts_with("url(\"data:image/svg+xml;utf8,<svg"));
        assert!(tile.contains("%232b3a67"));
        assert!(!tile.contains('#'));
        assert_eq!(tile.matches("<ellipse").count(), 20);
    }

    #[test]
    fn app_root_takes_its_text_colour_from_the_theme() {
        let css = include_str!("../assets/main.css");
        let start = css.find("\n.app {").expect("main.css has an .app rule");
        let rule = &css[start..start + css[start..].find('}').expect("closed rule")];
        assert!(rule.contains("color: var(--text)"), ".app must set color from --text: {rule}");
        assert!(rule.contains("background-color: var(--bg)"));
    }

    #[test]
    fn design_tokens_are_defined() {
        // docs/DESIGN.md lists these; screens rely on them existing.
        let css = include_str!("../assets/main.css");
        for token in [
            "--fs-xs:",
            "--fs-md:",
            "--fs-3xl:",
            "--lh-arabic:",
            "--measure:",
            "--measure-ar:",
            "--sp-1:",
            "--sp-8:",
            "--gutter:",
            "--stack:",
            "--card-pad:",
            "--line-soft:",
            "--shadow-raised:",
            "--dur:",
            "--ease:",
        ] {
            assert!(css.contains(token), "main.css must define {token}");
        }
        assert!(css.contains("@media (prefers-reduced-motion: reduce)"));
    }

    #[test]
    fn stylesheets_take_colours_from_tokens() {
        // A raw hex colour in a rule doesn't follow the theme (dark mode
        // breaks). Hex belongs only in custom-property definitions.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let hex = |s: &str| {
            s.match_indices('#').any(|(i, _)| {
                let digits = s[i + 1..].chars().take_while(char::is_ascii_hexdigit).count();
                matches!(digits, 3 | 4 | 6 | 8)
            })
        };
        for entry in std::fs::read_dir(&dir).expect("assets dir") {
            let path = entry.expect("dir entry").path();
            if path.extension().is_none_or(|e| e != "css") {
                continue;
            }
            let css = std::fs::read_to_string(&path).expect("readable css");
            // Comments may cite issues ("#152"); blank them, keeping line numbers.
            let mut code = String::with_capacity(css.len());
            let mut rest = css.as_str();
            while let Some(start) = rest.find("/*") {
                code.push_str(&rest[..start]);
                let end = rest[start..].find("*/").map_or(rest.len(), |e| start + e + 2);
                code.extend(rest[start..end].chars().filter(|c| *c == '\n'));
                rest = &rest[end..];
            }
            code.push_str(rest);
            for (n, line) in code.lines().enumerate() {
                // The declarations after the last `{` (or the whole line).
                let is_token = line.rsplit('{').next().is_some_and(|d| d.trim_start().starts_with("--"));
                assert!(
                    !hex(line) || is_token,
                    "{}:{}: raw colour outside a token; use a theme variable\n{line}",
                    path.display(),
                    n + 1
                );
            }
        }
    }

    #[test]
    fn every_theme_gets_a_batik_tile() {
        use kabar_core::settings::Theme;
        let css = stylesheet();
        assert!(css.contains(".app[data-theme=\"black\"] { --batik: none; }"));
        for theme in Theme::ALL {
            assert!(css.contains(&format!(".app[data-theme=\"{}\"] {{ --batik:", theme.as_str())));
        }
    }
}
