use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RgbaColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl RgbaColor {
    pub fn luminance(self) -> f32 {
        0.299 * self.r + 0.587 * self.g + 0.114 * self.b
    }
}

pub fn parse_hex_color(value: &str) -> Option<RgbaColor> {
    let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
    let hex = value.strip_prefix('#')?;
    let (r, g, b) = match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            (r, g, b)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b)
        }
        _ => return None,
    };
    Some(RgbaColor {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct OmarchyPalette {
    pub dark: bool,
    pub background: String,
    pub foreground: String,
    pub accent: String,
    pub selection: String,
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
            }
        } else {
            Self {
                dark: false,
                background: "#ffffff".into(),
                foreground: "#222324".into(),
                accent: "#2077b2".into(),
                selection: "#2077b2".into(),
            }
        }
    }

    pub fn muted(&self) -> &'static str {
        if self.dark {
            "#909191"
        } else {
            "#aeb1b5"
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
    use std::io::Write;

    #[test]
    fn loads_omarchy_theme() {
        let dir = std::env::temp_dir().join(format!("piwrite-theme-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("colors.toml");
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(
            b"mode = \"light\"\naccent = \"#112233\"\nselection = \"#445566\"\nbackground = \"#fefefe\"\nforeground = \"#101010\"\n",
        )
        .unwrap();
        let palette = OmarchyPalette::from_colors_file(&path, true);
        assert_eq!(palette.background, "#fefefe");
        assert_eq!(palette.foreground, "#101010");
        assert_eq!(palette.accent, "#112233");
        assert_eq!(palette.selection, "#445566");
        assert!(!palette.dark);
        let _ = fs::remove_dir_all(&dir);
    }
}
