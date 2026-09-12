use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RgbaColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl RgbaColor {
    pub fn luminance(self) -> f32 {
        0.299 * self.r + 0.587 * self.g + 0.114 * self.b
    }

    /// Composite this color at `alpha` over an opaque backdrop.
    fn over_with(self, alpha: f32, backdrop: RgbaColor) -> RgbaColor {
        RgbaColor {
            r: self.r * alpha + backdrop.r * (1.0 - alpha),
            g: self.g * alpha + backdrop.g * (1.0 - alpha),
            b: self.b * alpha + backdrop.b * (1.0 - alpha),
            a: 1.0,
        }
    }

    /// WCAG 2.x relative luminance of an opaque color.
    fn wcag_luminance(self) -> f32 {
        fn channel(c: f32) -> f32 {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }

    /// WCAG contrast ratio between two opaque colors, 1.0 ..= 21.0.
    fn contrast_ratio(self, other: RgbaColor) -> f32 {
        let (light, dark) = if self.wcag_luminance() >= other.wcag_luminance() {
            (self, other)
        } else {
            (other, self)
        };
        (light.wcag_luminance() + 0.05) / (dark.wcag_luminance() + 0.05)
    }
}

pub fn parse_hex_color(value: &str) -> Option<RgbaColor> {
    let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
    let hex = value.strip_prefix('#')?;
    let (r, g, b, a) = match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            (r, g, b, 255)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b, 255)
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            (r, g, b, a)
        }
        _ => return None,
    };
    Some(RgbaColor {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: a as f32 / 255.0,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct OmarchyPalette {
    pub dark: bool,
    pub background: String,
    pub foreground: String,
    pub accent: String,
    pub selection: String,
    pub muted: String,
    /// Elevated surface used for ghost-button hover. Omarchy's
    /// `lighter_background` (dark) / `darker_background` (light).
    pub hover: String,
    /// Ink color Omarchy wants drawn on top of the selection highlight.
    /// Themes that expect text to read *through* the selection omit it.
    pub selection_foreground: Option<String>,
}

impl OmarchyPalette {
    pub fn fallback(dark: bool) -> Self {
        if dark {
            Self {
                dark: true,
                background: "#101010".into(),
                foreground: "#eeeeee".into(),
                accent: "#5584aa".into(),
                selection: "#186a9a".into(),
                muted: "#909191".into(),
                hover: "#2a2a2a".into(),
                selection_foreground: None,
            }
        } else {
            Self {
                dark: false,
                background: "#ffffff".into(),
                foreground: "#222324".into(),
                accent: "#2077b2".into(),
                selection: "#2077b2".into(),
                muted: "#aeb1b5".into(),
                hover: "#e8e8e8".into(),
                selection_foreground: None,
            }
        }
    }

    pub fn load(dark_hint: bool) -> Self {
        Self::from_colors_file(&omarchy_colors_path(), dark_hint)
    }

    pub fn from_colors_file(path: &Path, dark_hint: bool) -> Self {
        let mut palette = Self::fallback(dark_hint);
        let Ok(raw) = fs::read_to_string(path) else {
            return palette;
        };
        let mut mode = String::new();
        let mut lighter = None;
        let mut darker = None;
        let mut muted_from_file = false;
        for line in raw.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = unquote(value.trim());
            match key {
                "mode" => mode = value,
                "background" => palette.background = value,
                "foreground" => palette.foreground = value,
                "accent" => palette.accent = value,
                "selection" => palette.selection = value,
                "selection_foreground" => {
                    palette.selection_foreground = parse_hex_color(&value).map(|_| value);
                }
                "muted" => {
                    palette.muted = value;
                    muted_from_file = true;
                }
                "lighter_background" => lighter = Some(value),
                "darker_background" => darker = Some(value),
                _ => {}
            }
        }
        if mode == "dark" {
            palette.dark = true;
        } else if mode == "light" {
            palette.dark = false;
        } else if let Some(bg) = parse_hex_color(&palette.background) {
            palette.dark = bg.luminance() < 0.5;
        }
        let resolved = Self::fallback(palette.dark);
        if !muted_from_file {
            palette.muted = resolved.muted;
        }
        palette.hover = if palette.dark {
            lighter.unwrap_or(resolved.hover)
        } else {
            darker.unwrap_or(resolved.hover)
        };
        palette
    }
}

pub fn omarchy_colors_path() -> PathBuf {
    home_dir().join(".local/state/omarchy/current/theme/colors.toml")
}

pub fn omarchy_watch_paths() -> Vec<PathBuf> {
    let current = home_dir().join(".local/state/omarchy/current");
    vec![
        current.clone(),
        current.join("theme"),
        current.join("theme/colors.toml"),
    ]
}

pub fn sanitized_text_scale(value: f32) -> f32 {
    if value <= 0.0 {
        1.0
    } else {
        value.clamp(0.5, 3.0)
    }
}

/// Alpha cap so glyphs painted over the selection stay readable: an opaque
/// highlight the same color as the text erases it (Ravenwood Emerald sets
/// `selection` and `foreground` to the same cream). Mirrors gpui-base's own
/// 0.4 fallback for unset selections.
pub const MAX_SELECTION_ALPHA: f32 = 0.4;

/// WCAG AA for body text; a selection highlight must not dip below it.
pub const MIN_READABLE_CONTRAST: f32 = 4.5;

/// The selection highlight color to install: the theme's selection at the
/// highest alpha (from its own, down to 0.05 in 0.05 steps) at which the
/// foreground still meets [`MIN_READABLE_CONTRAST`] over it, composited
/// against the page background. Normal themes (dark or muted selection,
/// contrasting foreground) keep a fully opaque selection; themes whose
/// selection matches their foreground get a translucent band instead.
///
/// Honoring Omarchy's `selection_foreground` by recoloring selected glyphs
/// is not viable: gpui-kit 0.6 projects one global selection color onto
/// every input, including the single-line find/replace fields, which have
/// no per-range ink hook — recoloring would fix the editor while erasing
/// find-box selections. The palette keeps parsing the key for tests and
/// future use.
pub fn selection_highlight(
    selection: &str,
    foreground: &str,
    background: &str,
) -> Option<RgbaColor> {
    let mut highlight = parse_hex_color(selection)?;
    let Some(fg) = parse_hex_color(foreground) else {
        highlight.a = highlight.a.min(MAX_SELECTION_ALPHA);
        return Some(highlight);
    };
    let Some(page) = parse_hex_color(background) else {
        highlight.a = highlight.a.min(MAX_SELECTION_ALPHA);
        return Some(highlight);
    };
    let mut alpha = highlight.a;
    loop {
        if fg.contrast_ratio(highlight.over_with(alpha, page)) >= MIN_READABLE_CONTRAST {
            break;
        }
        let next = (alpha - 0.05).max(0.05);
        if next == alpha {
            break;
        }
        alpha = next;
    }
    highlight.a = alpha;
    Some(highlight)
}

/// Cap a standalone selection color's alpha. Kept for callers that have no
/// foreground/page context (tests); prefer [`selection_highlight`].
pub fn clamp_selection_alpha(selection: &str) -> Option<RgbaColor> {
    let mut color = parse_hex_color(selection)?;
    color.a = color.a.min(MAX_SELECTION_ALPHA);
    Some(color)
}

pub fn detect_text_scale() -> f32 {
    if let Ok(output) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "text-scaling-factor"])
        .output()
    {
        if output.status.success() {
            let raw = String::from_utf8_lossy(&output.stdout);
            if let Ok(value) = raw.trim().parse::<f32>() {
                return sanitized_text_scale(value);
            }
        }
    }
    1.0
}

pub fn detect_system_dark() -> bool {
    if let Ok(output) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
    {
        if output.status.success() {
            let raw = String::from_utf8_lossy(&output.stdout);
            if raw.contains("prefer-dark") {
                return true;
            }
            if raw.contains("prefer-light") {
                return false;
            }
        }
    }
    true
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

fn home_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette_from(name: &str, dark_hint: bool, body: &str) -> OmarchyPalette {
        let dir = std::env::temp_dir().join(format!("piwrite-theme-{name}-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("colors.toml");
        fs::write(&path, body).unwrap();
        let palette = OmarchyPalette::from_colors_file(&path, dark_hint);
        let _ = fs::remove_dir_all(&dir);
        palette
    }

    #[test]
    fn loads_omarchy_theme() {
        let palette = palette_from(
            "basic",
            true,
            "mode = \"light\"\naccent = \"#112233\"\nselection = \"#445566\"\nbackground = \"#fefefe\"\nforeground = \"#101010\"\n",
        );
        assert_eq!(palette.background, "#fefefe");
        assert_eq!(palette.foreground, "#101010");
        assert_eq!(palette.accent, "#112233");
        assert_eq!(palette.selection, "#445566");
        assert!(!palette.dark);
        assert_eq!(palette.hover, OmarchyPalette::fallback(false).hover);
    }

    #[test]
    fn dark_hover_uses_lighter_background() {
        let palette = palette_from(
            "dark",
            false,
            "mode = \"dark\"\nbackground = \"#222822\"\nforeground = \"#e8d5b7\"\naccent = \"#4ade80\"\nselection = \"#e8d5b7\"\nmuted = \"#7f897d\"\nlighter_background = \"#2d3830\"\ndarker_background = \"#141814\"\n",
        );
        assert!(palette.dark);
        assert_eq!(palette.hover, "#2d3830");
        assert_eq!(palette.muted, "#7f897d");
    }

    #[test]
    fn light_hover_uses_darker_background() {
        let palette = palette_from(
            "light",
            true,
            "mode = \"light\"\nbackground = \"#f5f4ed\"\nforeground = \"#3f4a45\"\naccent = \"#064e3b\"\nselection = \"#3f4a45\"\nmuted = \"#7a8478\"\nlighter_background = \"#e8e7dc\"\ndarker_background = \"#edece1\"\n",
        );
        assert!(!palette.dark);
        assert_eq!(palette.hover, "#edece1");
        assert_eq!(palette.muted, "#7a8478");
    }

    #[test]
    fn parses_selection_foreground() {
        let palette = palette_from(
            "sel-fg",
            true,
            "mode = \"dark\"\nselection = \"#e8d5b7\"\nselection_foreground = \"#222822\"\nbackground = \"#222822\"\nforeground = \"#e8d5b7\"\n",
        );
        assert_eq!(palette.selection_foreground.as_deref(), Some("#222822"));
    }

    #[test]
    fn missing_selection_foreground_is_none() {
        let palette = palette_from(
            "no-sel-fg",
            true,
            "mode = \"dark\"\nselection = \"#45475a\"\nbackground = \"#1e1e2e\"\nforeground = \"#cdd6f4\"\n",
        );
        assert_eq!(palette.selection_foreground, None);
    }

    #[test]
    fn invalid_selection_foreground_is_ignored() {
        let palette = palette_from(
            "bad-sel-fg",
            true,
            "mode = \"dark\"\nselection = \"#45475a\"\nselection_foreground = \"not-a-color\"\nbackground = \"#1e1e2e\"\nforeground = \"#cdd6f4\"\n",
        );
        assert_eq!(palette.selection_foreground, None);
    }

    #[test]
    fn selection_alpha_is_clamped_when_no_context() {
        let opaque = clamp_selection_alpha("#e8d5b7").unwrap();
        assert!((opaque.a - 0.4).abs() < 1e-6);
        // A theme may set selection with its own alpha; only cap it.
        assert_eq!(parse_hex_color("#e8d5b766").unwrap().a, 0x66 as f32 / 255.0);
    }

    #[test]
    fn inverted_selection_gets_translucent_highlight() {
        // Ravenwood Emerald: selection == foreground == cream. An opaque band
        // erases the text; the installed highlight must fall back until the
        // foreground reads over it.
        let highlight = selection_highlight("#e8d5b7", "#e8d5b7", "#222822").unwrap();
        let page = parse_hex_color("#222822").unwrap();
        let fg = parse_hex_color("#e8d5b7").unwrap();
        assert!(highlight.a < 1.0);
        assert!(highlight.a <= MAX_SELECTION_ALPHA);
        assert!(fg.contrast_ratio(highlight.over_with(highlight.a, page)) >= MIN_READABLE_CONTRAST);
    }

    #[test]
    fn normal_theme_keeps_opaque_selection() {
        // catppuccin-dark: selection far darker than its light foreground.
        let highlight = selection_highlight("#45475a", "#cdd6f4", "#1e1e2e").unwrap();
        assert_eq!(highlight.a, 1.0);
        // And the highlight is still the theme's own color.
        assert_eq!(highlight.r, 0x45 as f32 / 255.0);
    }

    #[test]
    fn light_theme_selection_stays_opaque() {
        // rose-pine (light): dark text on a light selection band.
        let highlight = selection_highlight("#dfdad9", "#575279", "#faf4ed").unwrap();
        assert_eq!(highlight.a, 1.0);
    }

    #[test]
    fn hex_with_alpha_parses() {
        let c = parse_hex_color("#ffffff00").unwrap();
        assert_eq!(c.a, 0.0);
        let c = parse_hex_color("#ffffffff").unwrap();
        assert_eq!(c.a, 1.0);
        let c = parse_hex_color("#fff").unwrap();
        assert_eq!(c.a, 1.0);
        assert!(parse_hex_color("#12345").is_none());
    }
}
