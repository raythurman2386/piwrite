//! Custom icon and asset plumbing.
//!
//! The icon set bundled with gpui-kit-assets 0.6.0 has no save/floppy icon,
//! so the Save button used `IconName::File` as a stand-in. We ship the
//! official Lucide `save` icon ourselves and serve it alongside the bundled
//! set: `SaveIcon` implements `IconNamed` (the documented drop-in extension
//! point for `IconName`) and `PiwriteAssets` delegates to gpui-kit's asset
//! source, answering `icons/save.svg` from the embedded copy.

use gpui_kit::component::IconNamed;
use gpui_kit::{AssetSource, SharedString};
use std::borrow::Cow;

/// The Lucide floppy-disk save icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveIcon {
    Save,
}

impl IconNamed for SaveIcon {
    fn path(self) -> SharedString {
        "icons/save.svg".into()
    }
}

/// Asset source that serves the bundled gpui-kit icons plus `icons/save.svg`.
#[derive(rust_embed::RustEmbed)]
#[folder = "assets/"]
#[include = "icons/*.svg"]
pub struct PiwriteAssets;

impl AssetSource for PiwriteAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if path == "icons/save.svg" {
            if let Some(data) = Self::get(path) {
                return Ok(Some(data.data));
            }
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        gpui_kit::assets::Assets.list(path)
    }
}
