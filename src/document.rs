use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::recovery::{RecoverySlot, RecoverySnapshot};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub last_save_directory: Option<String>,
    pub window_x: Option<i32>,
    pub window_y: Option<i32>,
    pub window_width: u32,
    pub window_height: u32,
    pub maximized: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            last_save_directory: None,
            window_x: None,
            window_y: None,
            window_width: 1280,
            window_height: 820,
            maximized: false,
        }
    }
}

impl AppSettings {
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_vec_pretty(self).unwrap_or_else(|_| b"{}".to_vec());
        atomic_write(path, &json)
    }
}

/// Count words as letters/digits with optional internal apostrophes or
/// hyphens (`don't`, `two-three`).
pub fn count_words(text: &str) -> usize {
    static WORD_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = WORD_RE
        .get_or_init(|| Regex::new(r"[\p{L}\p{N}]+(?:['-][\p{L}\p{N}]+)*").expect("word regex"));
    re.find_iter(text).count()
}

pub fn suggested_file_name(text: &str) -> String {
    let mut name = text
        .split('\n')
        .next()
        .unwrap_or("")
        .trim()
        .chars()
        .map(|ch| {
            if ch == '/' || ch.is_control() {
                '-'
            } else {
                ch
            }
        })
        .collect::<String>();
    if name.chars().count() > 120 {
        name = name.chars().take(120).collect();
        name = name.trim().to_string();
    }
    if name.is_empty() || name == "." || name == ".." {
        name = "Untitled".into();
    }
    if !name.to_ascii_lowercase().ends_with(".md") {
        name.push_str(".md");
    }
    name
}

#[derive(Debug, Clone)]
pub struct SaveError {
    pub message: String,
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SaveError {}

#[derive(Debug, Clone)]
pub struct DocumentStore {
    pub path: Option<PathBuf>,
    pub text: String,
    pub modified: bool,
    pub status: String,
    pub last_known_file_contents: Option<Vec<u8>>,
    settings_path: PathBuf,
    settings: AppSettings,
}

impl DocumentStore {
    pub fn new(settings_path: PathBuf) -> Self {
        let settings = AppSettings::load(&settings_path);
        Self {
            path: None,
            text: String::new(),
            modified: false,
            status: String::new(),
            last_known_file_contents: None,
            settings_path,
            settings,
        }
    }

    pub fn file_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .filter(|n| !n.is_empty())
            .unwrap_or("Untitled.md")
            .to_string()
    }

    pub fn window_title(&self) -> String {
        let mark = if self.modified { "* " } else { "" };
        format!("{}{} - Piwrite", mark, self.file_name())
    }

    pub fn word_count(&self) -> usize {
        count_words(&self.text)
    }

    pub fn settings(&self) -> &AppSettings {
        &self.settings
    }

    pub fn persist_window_geometry(
        &mut self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        maximized: bool,
    ) {
        if !maximized {
            self.settings.window_x = Some(x);
            self.settings.window_y = Some(y);
            self.settings.window_width = width;
            self.settings.window_height = height;
        }
        self.settings.maximized = maximized;
        let _ = self.settings.save(&self.settings_path);
    }

    pub fn set_text_from_editor(&mut self, text: String) -> bool {
        let text = crate::markdown::normalize_plain_text(&text);
        if text == self.text {
            return false;
        }
        self.text = text;
        self.modified = true;
        self.status = "Unsaved".into();
        true
    }

    pub fn open_path(&mut self, path: &Path) -> Result<(), SaveError> {
        let contents = fs::read(path).map_err(|_| SaveError {
            message: format!(
                "Could not open {}.",
                path.file_name().and_then(|n| n.to_str()).unwrap_or("file")
            ),
        })?;
        let text = String::from_utf8_lossy(&contents).into_owned();
        self.text = crate::markdown::normalize_plain_text(&text);
        self.last_known_file_contents = Some(contents);
        self.path = Some(path.to_path_buf());
        self.modified = false;
        self.status = format!("Opened {}", self.file_name());
        Ok(())
    }

    pub fn suggested_save_path(&self) -> PathBuf {
        if let Some(path) = &self.path {
            return path.clone();
        }
        let directory = self
            .settings
            .last_save_directory
            .as_deref()
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
            .unwrap_or_else(home_dir);
        directory.join(suggested_file_name(&self.text))
    }

    pub fn save_to(&mut self, path: &Path) -> Result<(), SaveError> {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();
        let contents = self.text.as_bytes();
        atomic_write(path, contents).map_err(|_| SaveError {
            message: format!("Could not save {name}."),
        })?;
        self.last_known_file_contents = Some(contents.to_vec());
        self.path = Some(path.to_path_buf());
        if let Some(parent) = path.parent() {
            self.settings.last_save_directory = Some(parent.to_string_lossy().into_owned());
            let _ = self.settings.save(&self.settings_path);
        }
        self.modified = false;
        self.status = format!("Saved {name}");
        Ok(())
    }

    pub fn keep_external_version(&mut self) {
        if let Some(path) = &self.path {
            match fs::read(path) {
                Ok(bytes) => self.last_known_file_contents = Some(bytes),
                Err(_) => self.last_known_file_contents = None,
            }
        }
        self.modified = true;
        self.status = "Kept your version".into();
    }

    pub fn contents_match_disk(&self, path: &Path) -> bool {
        match (&self.last_known_file_contents, fs::read(path)) {
            (Some(known), Ok(on_disk)) => known == &on_disk,
            _ => false,
        }
    }

    pub fn restore_recovery(&mut self, snapshot: RecoverySnapshot) {
        self.text = crate::markdown::normalize_plain_text(&snapshot.text);
        self.path = snapshot.file_url.as_deref().and_then(file_url_to_path);
        if let Some(path) = &self.path {
            self.last_known_file_contents = fs::read(path).ok();
        } else {
            self.last_known_file_contents = None;
        }
        self.modified = true;
        self.status = "Recovered unsaved changes".into();
    }

    pub fn write_recovery(&self, slot: &RecoverySlot) {
        if !self.modified {
            return;
        }
        let snapshot = RecoverySnapshot {
            file_url: self
                .path
                .as_ref()
                .map(|p| format!("file://{}", p.display())),
            text: self.text.clone(),
        };
        let _ = slot.write(&snapshot);
    }
}

pub fn file_url_to_path(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    Some(PathBuf::from(rest))
}

fn home_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn app_paths() -> (PathBuf, PathBuf) {
    let base = directories::ProjectDirs::from("dev", "piwrite", "piwrite")
        .map(|p| p.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".").join("piwrite-data"));
    let config = directories::ProjectDirs::from("dev", "piwrite", "piwrite")
        .map(|p| p.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".").join("piwrite-config"));
    let _ = fs::create_dir_all(&base);
    let _ = fs::create_dir_all(&config);
    (base, config.join("settings.json"))
}

fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(contents)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_words() {
        assert_eq!(count_words("one two-three don't 42"), 4);
        assert_eq!(count_words("你好 世界"), 2);
        assert_eq!(count_words(""), 0);
    }

    #[test]
    fn suggests_safe_names() {
        assert_eq!(
            suggested_file_name("My first draft\nBody"),
            "My first draft.md"
        );
        assert_eq!(suggested_file_name("A/B"), "A-B.md");
        assert_eq!(suggested_file_name(""), "Untitled.md");
        assert_eq!(suggested_file_name("Already.md"), "Already.md");
    }
}
