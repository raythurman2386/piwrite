//! Testable writing-app logic shared by the GPUI Kit UI.

pub mod document;
pub mod markdown;
pub mod recovery;
pub mod theme;

pub use document::{count_words, suggested_file_name, DocumentStore, SaveError};
pub use markdown::{
    escape_link_destination, escape_link_text, find_all, hidden_ranges_at, inline_markup,
    insert_link_markdown, normalize_plain_text, normalized_link_url, smart_return, wrap_selection,
    InlineKind, InlineMarkup, SearchMatch, Span,
};
pub use recovery::{RecoverySlot, RecoverySnapshot};
pub use theme::{parse_hex_color, OmarchyPalette, RgbaColor};
