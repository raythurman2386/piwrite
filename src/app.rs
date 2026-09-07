use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    dialog::DialogButtonProps,
    h_flex,
    input::{Editor, EditorState, Input, InputEvent, InputState},
    v_flex, ActiveTheme, IconName, Root, Sizable, Theme, ThemeMode, WindowExt,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use piwrite::document::{app_paths, DocumentStore};
use piwrite::markdown::{
    find_all, insert_link_markdown, normalized_link_url, smart_return, wrap_selection, SearchMatch,
};
use piwrite::recovery::RecoverySlot;
use piwrite::theme::{detect_system_dark, detect_text_scale, omarchy_watch_paths, OmarchyPalette};

actions!(
    piwrite_actions,
    [
        Save,
        SaveAs,
        OpenFile,
        NewWindow,
        Print,
        Find,
        FindReplace,
        FindNext,
        Bold,
        Italic,
        Link,
        Shortcuts,
        ToggleFullscreen,
        Quit,
    ]
);

pub fn init(cx: &mut App) {
    load_fonts(cx);
    cx.bind_keys([
        KeyBinding::new("ctrl-s", Save, None),
        KeyBinding::new("ctrl-shift-s", SaveAs, None),
        KeyBinding::new("ctrl-o", OpenFile, None),
        KeyBinding::new("ctrl-n", NewWindow, None),
        KeyBinding::new("ctrl-p", Print, None),
        KeyBinding::new("ctrl-f", Find, None),
        KeyBinding::new("ctrl-h", FindReplace, None),
        KeyBinding::new("ctrl-g", FindNext, None),
        KeyBinding::new("ctrl-b", Bold, None),
        KeyBinding::new("ctrl-i", Italic, None),
        KeyBinding::new("ctrl-k", Link, None),
        KeyBinding::new("ctrl-shift-slash", Shortcuts, None),
        KeyBinding::new("f11", ToggleFullscreen, None),
        KeyBinding::new("super-f", ToggleFullscreen, None),
        KeyBinding::new("ctrl-q", Quit, None),
    ]);
}

fn load_fonts(cx: &mut App) {
    let fonts: [&'static [u8]; 4] = [
        include_bytes!("../fonts/iAWriterMonoS-Regular.ttf"),
        include_bytes!("../fonts/iAWriterMonoS-Italic.ttf"),
        include_bytes!("../fonts/iAWriterMonoS-Bold.ttf"),
        include_bytes!("../fonts/iAWriterMonoS-BoldItalic.ttf"),
    ];
    let blobs = fonts.into_iter().map(Cow::Borrowed).collect::<Vec<_>>();
    let _ = cx.text_system().add_fonts(blobs);
}

pub fn open_window(path: Option<PathBuf>, cx: &AsyncApp) -> anyhow::Result<WindowHandle<Root>> {
    cx.open_window(window_options(), move |window, cx| {
        let view = cx.new(|cx| Piwrite::new(path.clone(), window, cx));
        cx.new(|cx| Root::new(view, window, cx))
    })
    .map_err(|e| anyhow::anyhow!("{e}"))
}

fn window_options() -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(80.), px(60.)),
            size: size(px(1280.), px(820.)),
        })),
        window_min_size: Some(size(px(720.), px(520.))),
        titlebar: Some(TitlebarOptions {
            title: Some("Piwrite".into()),
            appears_transparent: false,
            traffic_light_position: None,
        }),
        app_id: Some("piwrite".into()),
        ..Default::default()
    }
}

enum PendingAction {
    None,
    Close,
    Open(PathBuf),
}

struct FileWatch {
    events: Arc<Mutex<Vec<PathBuf>>>,
    watcher: Option<RecommendedWatcher>,
    watched: Option<PathBuf>,
}

impl FileWatch {
    fn new() -> Self {
        let events = Arc::new(Mutex::new(Vec::new()));
        let tx = events.clone();
        let watcher = RecommendedWatcher::new(
            move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    if matches!(
                        event.kind,
                        EventKind::Modify(_) | EventKind::Remove(_) | EventKind::Create(_)
                    ) {
                        if let Ok(mut queue) = tx.lock() {
                            queue.extend(event.paths);
                        }
                    }
                }
            },
            notify::Config::default(),
        )
        .ok();
        Self {
            events,
            watcher,
            watched: None,
        }
    }

    fn watch(&mut self, path: Option<&Path>) {
        if let (Some(watcher), Some(old)) = (self.watcher.as_mut(), self.watched.take()) {
            let _ = watcher.unwatch(&old);
        }
        if let (Some(watcher), Some(path)) = (self.watcher.as_mut(), path) {
            if path.exists() {
                let _ = watcher.watch(path, RecursiveMode::NonRecursive);
                self.watched = Some(path.to_path_buf());
            }
        }
    }

    fn drain(&self) -> Vec<PathBuf> {
        self.events
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }
}

pub struct Piwrite {
    editor: Entity<EditorState>,
    search: Entity<InputState>,
    replace: Entity<InputState>,
    document: DocumentStore,
    recovery: Option<RecoverySlot>,
    palette: OmarchyPalette,
    text_scale: f32,
    search_open: bool,
    replace_open: bool,
    search_matches: Vec<SearchMatch>,
    search_index: isize,
    pending: PendingAction,
    file_watch: FileWatch,
    theme_watch: FileWatch,
    last_theme_check: Instant,
    last_recovery: Instant,
    _subscriptions: Vec<Subscription>,
}

impl Piwrite {
    pub fn new(path: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (data_dir, settings_path) = app_paths();
        let recovery = RecoverySlot::claim(&data_dir).ok();
        let mut document = DocumentStore::new(settings_path);
        let dark = detect_system_dark();
        let palette = OmarchyPalette::load(dark);
        let text_scale = detect_text_scale();

        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("markdown")
                .line_number(false)
                .folding(false)
                .soft_wrap(true)
                .placeholder("# Start writing")
        });
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Find"));
        let replace = cx.new(|cx| InputState::new(window, cx).placeholder("Replace with"));

        let mut file_watch = FileWatch::new();
        let mut theme_watch = FileWatch::new();
        for path in omarchy_watch_paths() {
            theme_watch.watch(Some(&path));
        }

        if let Some(snapshot) = recovery.as_ref().and_then(|slot| slot.read()) {
            document.restore_recovery(snapshot);
            let text = document.text.clone();
            editor.update(cx, |state, cx| {
                state.set_value(text, window, cx);
            });
        } else if let Some(path) = path {
            if document.open_path(&path).is_ok() {
                let text = document.text.clone();
                editor.update(cx, |state, cx| {
                    state.set_value(text, window, cx);
                });
                file_watch.watch(Some(&path));
            }
        }

        let editor_sub = cx.subscribe_in(&editor, window, |this, state, event, window, cx| {
            if matches!(event, InputEvent::Change) {
                let text = state.read(cx).value().to_string();
                if this.document.set_text_from_editor(text) {
                    this.schedule_recovery();
                    this.refresh_search(window, cx);
                    this.sync_title(window);
                    cx.notify();
                }
            }
            if matches!(event, InputEvent::PressEnter { shift: true, .. }) {
                this.apply_smart_return(true, window, cx);
            }
        });
        let search_sub = cx.subscribe_in(&search, window, |this, _, event, window, cx| {
            if matches!(event, InputEvent::Change) {
                this.refresh_search(window, cx);
            }
            if let InputEvent::PressEnter {
                secondary, shift, ..
            } = event
            {
                let dir = if *shift || *secondary { -1 } else { 1 };
                this.move_search(dir, window, cx);
            }
        });

        apply_palette(&palette, text_scale, Some(window), cx);
        window.set_window_title(&document.window_title());

        let focus = editor.focus_handle(cx);
        window.defer(cx, move |window, cx| {
            focus.focus(window, cx);
        });

        Self {
            editor,
            search,
            replace,
            document,
            recovery,
            palette,
            text_scale,
            search_open: false,
            replace_open: false,
            search_matches: Vec::new(),
            search_index: -1,
            pending: PendingAction::None,
            file_watch,
            theme_watch,
            last_theme_check: Instant::now(),
            last_recovery: Instant::now(),
            _subscriptions: vec![editor_sub, search_sub],
        }
    }

    fn schedule_recovery(&mut self) {
        self.last_recovery = Instant::now();
    }

    fn sync_title(&self, window: &mut Window) {
        window.set_window_title(&self.document.window_title());
    }

    fn editor_text(&self, cx: &App) -> String {
        self.editor.read(cx).value().to_string()
    }

    fn apply_smart_return(&mut self, soft: bool, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.editor_text(cx);
        let cursor = self.editor.read(cx).cursor();
        let (_start, _end, replacement) = smart_return(&text, cursor, soft);
        self.editor.update(cx, |state, cx| {
            state.replace(&replacement, window, cx);
        });
    }

    fn wrap(&mut self, before: &str, after: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |state, cx| {
            let selected = state.selected_value().to_string();
            let replacement = wrap_selection(&selected, before, after);
            state.replace(&replacement, window, cx);
        });
    }

    fn insert_link(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let clipboard_url = cx
            .read_from_clipboard()
            .and_then(|item| item.text().map(|t| t.to_string()))
            .and_then(|text| normalized_link_url(&text));
        self.editor.update(cx, |state, cx| {
            let selected = state.selected_value().to_string();
            let (markdown, _, _) = insert_link_markdown(&selected, clipboard_url.as_deref());
            state.replace(&markdown, window, cx);
        });
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.document.text = self.editor_text(cx);
        if self.document.path.is_none() {
            self.save_as(window, cx);
            return;
        }
        let path = self.document.path.clone().unwrap();
        self.commit_save(&path, window, cx);
    }

    fn save_as(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.document.text = self.editor_text(cx);
        let suggested = self.document.suggested_save_path();
        let directory = suggested
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let name = suggested
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_string);
        let rx = cx.prompt_for_new_path(&directory, name.as_deref());
        cx.spawn_in(window, async move |this, cx| {
            let path = rx.await.ok()?.ok()??;
            this.update_in(cx, |this, window, cx| this.commit_save(&path, window, cx))
                .ok();
            Some(())
        })
        .detach();
    }

    fn commit_save(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        self.file_watch.watch(None);
        match self.document.save_to(path) {
            Ok(()) => {
                if let Some(slot) = &self.recovery {
                    slot.clear();
                }
                self.file_watch.watch(self.document.path.as_deref());
                self.sync_title(window);
                if matches!(self.pending, PendingAction::Close) {
                    window.remove_window();
                } else if let PendingAction::Open(open_path) =
                    std::mem::replace(&mut self.pending, PendingAction::None)
                {
                    self.open_path(&open_path, window, cx);
                }
                cx.notify();
            }
            Err(err) => {
                self.file_watch.watch(self.document.path.as_deref());
                self.document.status = err.message;
                cx.notify();
            }
        }
    }

    fn open_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open Markdown".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let path = rx.await.ok()?.ok()??.into_iter().next()?;
            this.update_in(cx, |this, window, cx| this.request_open(path, window, cx))
                .ok();
            Some(())
        })
        .detach();
    }

    fn request_open(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.document.modified {
            self.pending = PendingAction::Open(path);
            self.prompt_unsaved(window, cx);
        } else {
            self.open_path(&path, window, cx);
        }
    }

    fn open_path(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        match self.document.open_path(path) {
            Ok(()) => {
                let text = self.document.text.clone();
                self.editor.update(cx, |state, cx| {
                    state.set_value(text, window, cx);
                });
                if let Some(slot) = &self.recovery {
                    slot.clear();
                }
                self.file_watch.watch(self.document.path.as_deref());
                self.sync_title(window);
                cx.notify();
            }
            Err(err) => {
                self.document.status = err.message;
                cx.notify();
            }
        }
    }

    fn prompt_unsaved(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.document.file_name();
        let view = cx.entity();
        window.open_alert_dialog(cx, move |alert, _, _| {
            alert
                .confirm()
                .title("Unsaved changes")
                .description(format!("Save changes to {name} before continuing?"))
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("Save")
                        .cancel_text("Don't Save")
                        .show_cancel(true),
                )
                .on_ok({
                    let view = view.clone();
                    move |_, window, cx| {
                        view.update(cx, |this, cx| this.save(window, cx));
                        true
                    }
                })
                .on_cancel({
                    let view = view.clone();
                    move |_, window, cx| {
                        view.update(cx, |this, cx| this.discard_pending(window, cx));
                        true
                    }
                })
        });
    }

    fn discard_pending(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(slot) = &self.recovery {
            slot.clear();
        }
        match std::mem::replace(&mut self.pending, PendingAction::None) {
            PendingAction::Close => window.remove_window(),
            PendingAction::Open(path) => self.open_path(&path, window, cx),
            PendingAction::None => {}
        }
    }

    fn new_window(&mut self, cx: &mut Context<Self>) {
        let exe = std::env::current_exe().ok();
        if let Some(exe) = exe {
            if std::process::Command::new(exe).spawn().is_err() {
                self.document.status = "Could not open a new window.".into();
                cx.notify();
            }
        }
    }

    fn print(&mut self, cx: &mut Context<Self>) {
        self.document.text = self.editor_text(cx);
        let name = self.document.file_name();
        let tmp = std::env::temp_dir().join(format!("{name}.print.md"));
        if std::fs::write(&tmp, &self.document.text).is_err() {
            self.document.status = "There is no document to print.".into();
            cx.notify();
            return;
        }
        let printed = std::process::Command::new("lp")
            .arg("-t")
            .arg(&name)
            .arg(&tmp)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if printed {
            self.document.status = format!("Printed {name}");
        } else if open::that(&tmp).is_ok() {
            self.document.status = format!("Opened print preview for {name}");
        } else {
            self.document.status = "Could not print.".into();
        }
        cx.notify();
    }

    fn toggle_find(&mut self, replace: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.search_open = true;
        self.replace_open = replace;
        self.search.update(cx, |state, cx| {
            state.focus(window, cx);
        });
        self.refresh_search(window, cx);
        cx.notify();
    }

    fn refresh_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.search_open {
            return;
        }
        let query = self.search.read(cx).value().to_string();
        let text = self.editor_text(cx);
        self.search_matches = find_all(&text, &query);
        self.search_index = if self.search_matches.is_empty() {
            -1
        } else {
            0
        };
        self.show_search_match(window, cx);
    }

    fn move_search(&mut self, direction: isize, window: &mut Window, cx: &mut Context<Self>) {
        if self.search_matches.is_empty() {
            return;
        }
        let len = self.search_matches.len() as isize;
        self.search_index = (self.search_index + direction).rem_euclid(len);
        self.show_search_match(window, cx);
    }

    fn show_search_match(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.search_index < 0 {
            return;
        }
        let Some(m) = self.search_matches.get(self.search_index as usize).copied() else {
            return;
        };
        self.editor.update(cx, |state, cx| {
            state.set_selected_range(m.start..m.end, cx);
            state.focus(window, cx);
        });
        cx.notify();
    }

    fn replace_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.search_index < 0 {
            return;
        }
        let Some(m) = self.search_matches.get(self.search_index as usize).copied() else {
            return;
        };
        let replacement = self.replace.read(cx).value().to_string();
        let mut text = self.editor_text(cx);
        if m.end <= text.len() {
            text.replace_range(m.start..m.end, &replacement);
            self.editor.update(cx, |state, cx| {
                state.set_value(text, window, cx);
            });
            self.refresh_search(window, cx);
        }
    }

    fn replace_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.search.read(cx).value().to_string();
        if query.is_empty() {
            return;
        }
        let replacement = self.replace.read(cx).value().to_string();
        let text = self.editor_text(cx);
        let updated = replace_all_ci(&text, &query, &replacement);
        self.editor.update(cx, |state, cx| {
            state.set_value(updated, window, cx);
        });
        self.refresh_search(window, cx);
    }

    fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_open = false;
        self.replace_open = false;
        self.editor.update(cx, |state, cx| {
            state.focus(window, cx);
        });
        cx.notify();
    }

    fn show_shortcuts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.open_dialog(cx, |dialog, _, _| {
            dialog.title("Keyboard shortcuts").child(
                "Ctrl+S  Save\nCtrl+Shift+S  Save As\nCtrl+O  Open\nCtrl+N  New Window\nCtrl+F  Find\nCtrl+H  Find and Replace\nCtrl+B  Bold\nCtrl+I  Italic\nCtrl+K  Link\nCtrl+P  Print\nF11 / Super+F  Fullscreen\nCtrl+?  Shortcuts",
            )
        });
    }

    fn toggle_fullscreen(&mut self, window: &mut Window, _cx: &mut Context<Self>) {
        window.toggle_fullscreen();
    }

    fn poll_external(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.last_theme_check.elapsed() > Duration::from_millis(400) {
            self.last_theme_check = Instant::now();
            if !self.theme_watch.drain().is_empty() {
                let palette = OmarchyPalette::load(self.palette.dark);
                if palette != self.palette {
                    self.palette = palette;
                    apply_palette(&self.palette, self.text_scale, Some(window), cx);
                    cx.notify();
                }
            }
            let scale = detect_text_scale();
            if (scale - self.text_scale).abs() > f32::EPSILON {
                self.text_scale = scale;
                apply_palette(&self.palette, self.text_scale, Some(window), cx);
                cx.notify();
            }
        }
        if self.document.modified && self.last_recovery.elapsed() > Duration::from_millis(750) {
            self.document.text = self.editor_text(cx);
            if let Some(slot) = &self.recovery {
                self.document.write_recovery(slot);
            }
            self.last_recovery = Instant::now();
        }
        let events = self.file_watch.drain();
        if let Some(path) = self.document.path.clone() {
            if events.iter().any(|p| p == &path) {
                let deleted = !path.exists();
                if !deleted && self.document.contents_match_disk(&path) {
                    self.file_watch.watch(Some(&path));
                    return;
                }
                let locally_modified = self.document.modified;
                self.prompt_external(deleted, locally_modified, window, cx);
            }
        }
    }

    fn prompt_external(
        &mut self,
        deleted: bool,
        locally_modified: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let description = if deleted {
            "This file was deleted on disk.".to_string()
        } else if locally_modified {
            "This file changed on disk and you have unsaved edits.".to_string()
        } else {
            "This file changed on disk.".to_string()
        };
        let view = cx.entity();
        let path = self.document.path.clone();
        window.open_alert_dialog(cx, move |alert, _, _| {
            alert
                .confirm()
                .title("File changed")
                .description(description.clone())
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("Reload")
                        .cancel_text("Keep mine")
                        .show_cancel(true),
                )
                .on_ok({
                    let view = view.clone();
                    let path = path.clone();
                    move |_, window, cx| {
                        if let Some(path) = &path {
                            view.update(cx, |this, cx| this.open_path(path, window, cx));
                        }
                        true
                    }
                })
                .on_cancel({
                    let view = view.clone();
                    move |_, _, cx| {
                        view.update(cx, |this, _cx| this.document.keep_external_version());
                        true
                    }
                })
        });
    }

    fn on_save(&mut self, _: &Save, window: &mut Window, cx: &mut Context<Self>) {
        self.save(window, cx);
    }
    fn on_save_as(&mut self, _: &SaveAs, window: &mut Window, cx: &mut Context<Self>) {
        self.save_as(window, cx);
    }
    fn on_open(&mut self, _: &OpenFile, window: &mut Window, cx: &mut Context<Self>) {
        self.open_dialog(window, cx);
    }
    fn on_new_window(&mut self, _: &NewWindow, _: &mut Window, cx: &mut Context<Self>) {
        self.new_window(cx);
    }
    fn on_print(&mut self, _: &Print, _: &mut Window, cx: &mut Context<Self>) {
        self.print(cx);
    }
    fn on_find(&mut self, _: &Find, window: &mut Window, cx: &mut Context<Self>) {
        self.toggle_find(false, window, cx);
    }
    fn on_find_replace(&mut self, _: &FindReplace, window: &mut Window, cx: &mut Context<Self>) {
        self.toggle_find(true, window, cx);
    }
    fn on_find_next(&mut self, _: &FindNext, window: &mut Window, cx: &mut Context<Self>) {
        self.move_search(1, window, cx);
    }
    fn on_bold(&mut self, _: &Bold, window: &mut Window, cx: &mut Context<Self>) {
        self.wrap("**", "**", window, cx);
    }
    fn on_italic(&mut self, _: &Italic, window: &mut Window, cx: &mut Context<Self>) {
        self.wrap("*", "*", window, cx);
    }
    fn on_link(&mut self, _: &Link, window: &mut Window, cx: &mut Context<Self>) {
        self.insert_link(window, cx);
    }
    fn on_shortcuts(&mut self, _: &Shortcuts, window: &mut Window, cx: &mut Context<Self>) {
        self.show_shortcuts(window, cx);
    }
    fn on_fullscreen(&mut self, _: &ToggleFullscreen, window: &mut Window, cx: &mut Context<Self>) {
        self.toggle_fullscreen(window, cx);
    }
    fn on_quit(&mut self, _: &Quit, window: &mut Window, cx: &mut Context<Self>) {
        if self.document.modified && !matches!(self.pending, PendingAction::Close) {
            self.pending = PendingAction::Close;
            self.prompt_unsaved(window, cx);
            return;
        }
        window.remove_window();
    }

    fn scaled(&self, px_value: f32) -> Pixels {
        px(px_value * self.text_scale)
    }
}

fn replace_all_ci(haystack: &str, query: &str, replacement: &str) -> String {
    let matches = find_all(haystack, query);
    let mut out = String::with_capacity(haystack.len());
    let mut last = 0;
    for m in matches {
        out.push_str(&haystack[last..m.start]);
        out.push_str(replacement);
        last = m.end;
    }
    out.push_str(&haystack[last..]);
    out
}

fn apply_palette(
    palette: &OmarchyPalette,
    text_scale: f32,
    window: Option<&mut Window>,
    cx: &mut App,
) {
    Theme::change(
        if palette.dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        window,
        cx,
    );
    let theme = Theme::global_mut(cx);
    if let Some(bg) = hex_to_hsla(&palette.background) {
        theme.background = bg;
    }
    if let Some(fg) = hex_to_hsla(&palette.foreground) {
        theme.foreground = fg;
    }
    if let Some(accent) = hex_to_hsla(&palette.accent) {
        theme.primary = accent;
        theme.accent = accent;
    }
    theme.mono_font_family = "iA Writer Mono S".into();
    theme.font_family = "iA Writer Mono S".into();
    theme.mono_font_size = px(20. * text_scale);
    theme.font_size = px(16. * text_scale);
    Theme::sync_base(cx);
}

fn hex_to_hsla(value: &str) -> Option<Hsla> {
    let hex = value.trim().trim_start_matches('#');
    let expanded = if hex.len() == 3 {
        hex.chars().flat_map(|c| [c, c]).collect::<String>()
    } else {
        hex.to_string()
    };
    let n = u32::from_str_radix(&expanded, 16).ok()?;
    Some(rgb(n).into())
}

impl Render for Piwrite {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.poll_external(window, cx);

        let muted = if self.palette.dark {
            rgb(0x909191)
        } else {
            rgb(0xaeb1b5)
        };
        let page = hex_to_hsla(&self.palette.background).unwrap_or(cx.theme().background);
        let editor_width = px((65.0_f32 * 12.0 * self.text_scale)
            .min(window.bounds().size.width.as_f32() - 80.0)
            .max(360.0));

        v_flex()
            .id("piwrite")
            .size_full()
            .bg(page)
            .font_family("iA Writer Mono S")
            .text_size(self.scaled(20.))
            .text_color(hex_to_hsla(&self.palette.foreground).unwrap_or(cx.theme().foreground))
            .on_action(cx.listener(Self::on_save))
            .on_action(cx.listener(Self::on_save_as))
            .on_action(cx.listener(Self::on_open))
            .on_action(cx.listener(Self::on_new_window))
            .on_action(cx.listener(Self::on_print))
            .on_action(cx.listener(Self::on_find))
            .on_action(cx.listener(Self::on_find_replace))
            .on_action(cx.listener(Self::on_find_next))
            .on_action(cx.listener(Self::on_bold))
            .on_action(cx.listener(Self::on_italic))
            .on_action(cx.listener(Self::on_link))
            .on_action(cx.listener(Self::on_shortcuts))
            .on_action(cx.listener(Self::on_fullscreen))
            .on_action(cx.listener(Self::on_quit))
            .when(self.search_open, |this| this.child(self.render_search(cx)))
            .child(
                v_flex()
                    .id("editor-wrap")
                    .flex_1()
                    .w_full()
                    .items_center()
                    .px(self.scaled(24.))
                    .pt(self.scaled(42.))
                    .pb(self.scaled(64.))
                    .child(
                        Editor::new(&self.editor)
                            .bordered(false)
                            .p_0()
                            .w(editor_width)
                            .h_full()
                            .bg(page)
                            .font_family("iA Writer Mono S")
                            .text_size(self.scaled(20.))
                            .line_height(relative(1.4)),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .px(self.scaled(12.))
                    .pb(self.scaled(10.))
                    .gap(self.scaled(12.))
                    .opacity(0.7)
                    .child(
                        Button::new("save")
                            .ghost()
                            .xsmall()
                            .icon(IconName::File)
                            .tooltip("Save")
                            .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                    )
                    .child(
                        Button::new("open")
                            .ghost()
                            .xsmall()
                            .icon(IconName::FolderOpen)
                            .tooltip("Open")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_dialog(window, cx)),
                            ),
                    )
                    .child(
                        div()
                            .text_size(self.scaled(11.))
                            .text_color(muted)
                            .child(self.document.status.clone()),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(self.scaled(11.))
                            .text_color(muted)
                            .child(word_label(self.document.word_count())),
                    ),
            )
    }
}

impl Piwrite {
    fn render_search(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let count = if self.search_matches.is_empty() {
            String::new()
        } else {
            format!(
                "{} / {}",
                self.search_index.max(0) + 1,
                self.search_matches.len()
            )
        };
        let bar_bg = if self.palette.dark {
            rgb(0x22221f)
        } else {
            rgb(0xfffef2)
        };
        v_flex()
            .w_full()
            .px(self.scaled(12.))
            .pt(self.scaled(12.))
            .child(
                v_flex()
                    .w_full()
                    .rounded_xl()
                    .px(self.scaled(16.))
                    .py(self.scaled(8.))
                    .bg(bar_bg)
                    .shadow_md()
                    .gap(self.scaled(6.))
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .gap(self.scaled(8.))
                            .child(Input::new(&self.search).appearance(false).flex_1())
                            .child(div().text_size(self.scaled(13.)).child(count))
                            .child(
                                Button::new("search-prev")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::ArrowUp)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.move_search(-1, window, cx)
                                    })),
                            )
                            .child(
                                Button::new("search-next")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::ArrowDown)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.move_search(1, window, cx)
                                    })),
                            )
                            .when(self.replace_open, |this| {
                                this.child(
                                    Button::new("replace-one")
                                        .ghost()
                                        .xsmall()
                                        .label("Replace")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.replace_current(window, cx)
                                        })),
                                )
                                .child(
                                    Button::new("replace-all")
                                        .ghost()
                                        .xsmall()
                                        .label("All")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.replace_all(window, cx)
                                        })),
                                )
                            })
                            .child(
                                Button::new("search-close")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Close)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.close_search(window, cx)
                                    })),
                            ),
                    )
                    .when(self.replace_open, |this| {
                        this.child(Input::new(&self.replace).appearance(false))
                    }),
            )
    }
}

fn word_label(count: usize) -> String {
    if count == 1 {
        "1 Word".into()
    } else {
        format!("{count} Words")
    }
}
