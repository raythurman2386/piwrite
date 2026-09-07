use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineKind {
    Bold,
    Italic,
    Link,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub length: usize,
}

impl Span {
    pub fn end(self) -> usize {
        self.start + self.length
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineMarkup {
    pub kind: InlineKind,
    pub content: Span,
    pub markers: Vec<Span>,
}

pub fn normalize_plain_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Normalize a clipboard string to an http(s)/ftp/mailto link destination.
pub fn normalized_link_url(clipboard_text: &str) -> Option<String> {
    let mut candidate = clipboard_text.trim().to_string();
    if let Some(idx) = candidate.find(['\r', '\n']) {
        candidate = candidate[..idx].trim().to_string();
    }
    if candidate.is_empty() {
        return None;
    }
    if candidate.to_ascii_lowercase().starts_with("www.") {
        candidate.insert_str(0, "https://");
    }
    static SCHEME_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let scheme_re = SCHEME_RE.get_or_init(|| Regex::new(r"^[A-Za-z][A-Za-z0-9+.-]*:").unwrap());
    if !scheme_re.is_match(&candidate) {
        return None;
    }
    let url = url_parts(&candidate)?;
    let scheme = url.scheme.to_ascii_lowercase();
    let web = matches!(scheme.as_str(), "http" | "https" | "ftp");
    if web && url.host.is_empty() {
        return None;
    }
    if !web && scheme != "mailto" {
        return None;
    }
    Some(candidate)
}

struct UrlParts {
    scheme: String,
    host: String,
}

fn url_parts(value: &str) -> Option<UrlParts> {
    let (scheme, rest) = value.split_once(':')?;
    if scheme.is_empty() {
        return None;
    }
    let rest = rest.trim_start_matches("//");
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .split('@')
        .next_back()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_string();
    Some(UrlParts {
        scheme: scheme.to_string(),
        host,
    })
}

pub fn escape_link_text(link_text: &str) -> String {
    link_text
        .replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

pub fn escape_link_destination(link_url: &str) -> String {
    link_url
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

pub fn wrap_selection(selected: &str, before: &str, after: &str) -> String {
    format!("{before}{selected}{after}")
}

/// Build the markdown for Ctrl+K. Returns `(markdown, select_start, select_end)`
/// offsets relative to the insertion start.
pub fn insert_link_markdown(selected: &str, clipboard_url: Option<&str>) -> (String, usize, usize) {
    let label = if selected.is_empty() {
        "link text"
    } else {
        selected
    };
    let destination = clipboard_url.unwrap_or("https://");
    let escaped_label = escape_link_text(label);
    let markdown = format!(
        "[{}]({})",
        escaped_label,
        escape_link_destination(destination)
    );
    if selected.is_empty() {
        (markdown, 1, 1 + escaped_label.len())
    } else if clipboard_url.is_none() {
        let start = escaped_label.len() + 3;
        let end = markdown.len().saturating_sub(1);
        (markdown, start, end)
    } else {
        let len = markdown.len();
        (markdown, 0, len)
    }
}

pub fn smart_return(text: &str, cursor: usize, soft_break: bool) -> (usize, usize, String) {
    if soft_break {
        return (cursor, cursor, "\n".into());
    }
    let line_start = text[..cursor].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line = &text[line_start..cursor];
    let before = &text[..cursor];
    let fences = before
        .lines()
        .filter(|l| l.trim_start().starts_with("```"))
        .count();
    if fences % 2 == 1 {
        return (cursor, cursor, "\n".into());
    }
    static LIST_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let list_re = LIST_RE.get_or_init(|| Regex::new(r"^(\s*)([-+*]|\d+[.)]|>+)\s+(.*)$").unwrap());
    if let Some(caps) = list_re.captures(line) {
        let indent = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let marker = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        let rest = caps.get(3).map(|m| m.as_str()).unwrap_or("");
        if rest.is_empty() {
            return (line_start, cursor, "\n".into());
        }
        let next_marker = if marker.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            let digits: String = marker.chars().take_while(|c| c.is_ascii_digit()).collect();
            let n: u32 = digits.parse().unwrap_or(1);
            format!("{}{}", n + 1, &marker[digits.len()..])
        } else {
            marker.to_string()
        };
        return (cursor, cursor, format!("\n{indent}{next_marker} "));
    }
    (cursor, cursor, "\n\n".into())
}

pub fn inline_markup(text: &str) -> Vec<InlineMarkup> {
    let mut markup = Vec::new();
    if !text.contains(['*', '_', '[']) {
        return markup;
    }

    static BOLD_STAR: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    static BOLD_UNDER: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let bold_star = BOLD_STAR.get_or_init(|| Regex::new(r"\*\*(.+?)\*\*").unwrap());
    let bold_under = BOLD_UNDER.get_or_init(|| Regex::new(r"__(.+?)__").unwrap());
    for (re, marker_len) in [(bold_star, 2), (bold_under, 2)] {
        for caps in re.captures_iter(text) {
            let whole = caps.get(0).unwrap();
            let content = caps.get(1).unwrap();
            markup.push(InlineMarkup {
                kind: InlineKind::Bold,
                content: Span {
                    start: content.start(),
                    length: content.len(),
                },
                markers: vec![
                    Span {
                        start: whole.start(),
                        length: marker_len,
                    },
                    Span {
                        start: whole.end() - marker_len,
                        length: marker_len,
                    },
                ],
            });
        }
    }

    push_italic_markers(text, '*', &mut markup);
    push_italic_markers(text, '_', &mut markup);

    static LINK_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let link_re = LINK_RE.get_or_init(|| Regex::new(r"\[([^\]]+)\]\(((?:\\.|[^)])+)\)").unwrap());
    for caps in link_re.captures_iter(text) {
        let whole = caps.get(0).unwrap();
        let content = caps.get(1).unwrap();
        let content_end = content.end();
        markup.push(InlineMarkup {
            kind: InlineKind::Link,
            content: Span {
                start: content.start(),
                length: content.len(),
            },
            markers: vec![
                Span {
                    start: whole.start(),
                    length: 1,
                },
                Span {
                    start: content_end,
                    length: whole.end() - content_end,
                },
            ],
        });
    }

    markup
}

fn push_italic_markers(text: &str, marker: char, markup: &mut Vec<InlineMarkup>) {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != marker as u8 {
            i += 1;
            continue;
        }
        let doubled = i + 1 < bytes.len() && bytes[i + 1] == marker as u8;
        if doubled {
            i += 2;
            continue;
        }
        let start = i;
        i += 1;
        while i < bytes.len() && bytes[i] != marker as u8 && bytes[i] != b'\n' {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == marker as u8 {
            let doubled_close = i + 1 < bytes.len() && bytes[i + 1] == marker as u8;
            if !doubled_close && i > start + 1 {
                markup.push(InlineMarkup {
                    kind: InlineKind::Italic,
                    content: Span {
                        start: start + 1,
                        length: i - start - 1,
                    },
                    markers: vec![
                        Span { start, length: 1 },
                        Span {
                            start: i,
                            length: 1,
                        },
                    ],
                });
            }
            i += 1;
        }
    }
}

pub fn hidden_ranges_at(text: &str, position: usize) -> Vec<(usize, usize)> {
    let line_start = text[..position.min(text.len())]
        .rfind('\n')
        .map(|i| i + 1)
        .unwrap_or(0);
    let line_end = text[line_start..]
        .find('\n')
        .map(|i| line_start + i)
        .unwrap_or(text.len());
    let line = &text[line_start..line_end];
    let mut spans: Vec<(usize, usize)> = inline_markup(line)
        .into_iter()
        .flat_map(|item| {
            item.markers
                .into_iter()
                .map(move |m| (line_start + m.start, line_start + m.end()))
        })
        .collect();
    spans.sort_unstable();
    spans
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchMatch {
    pub start: usize,
    pub end: usize,
}

pub fn find_all(haystack: &str, query: &str) -> Vec<SearchMatch> {
    if query.is_empty() {
        return Vec::new();
    }
    let lower_hay = haystack.to_lowercase();
    let lower_needle = query.to_lowercase();
    let mut matches = Vec::new();
    let mut pos = 0;
    while let Some(found) = lower_hay[pos..].find(&lower_needle) {
        let start = pos + found;
        let end = start + query.len();
        matches.push(SearchMatch { start, end });
        pos = start + query.len().max(1);
        if pos > lower_hay.len() {
            break;
        }
    }
    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_links() {
        assert_eq!(
            normalized_link_url("www.example.com/path"),
            Some("https://www.example.com/path".into())
        );
        assert_eq!(
            normalized_link_url("mailto:writer@example.com"),
            Some("mailto:writer@example.com".into())
        );
        assert_eq!(normalized_link_url("example.com"), None);
        assert_eq!(normalized_link_url("file:///tmp/private"), None);
    }

    #[test]
    fn finds_inline_markdown_ranges() {
        let markup = inline_markup("**bold** and *italic* and [site](https://example.com)");
        assert_eq!(markup.len(), 3);
        assert_eq!(markup[0].content.start, 2);
        assert_eq!(markup[0].content.length, 4);
        assert_eq!(markup[2].content.length, 4);
        assert_eq!(markup[2].markers[0].length, 1);
    }

    #[test]
    fn replace_range_keeps_selection_offsets() {
        let inserted = normalize_plain_text("one\r\ntwo");
        assert_eq!(inserted, "one\ntwo");
        let wrapped = wrap_selection("beta", "**", "**");
        assert_eq!(wrapped, "**beta**");
    }

    #[test]
    fn smart_return_continues_lists() {
        let text = "- item";
        let (start, end, replacement) = smart_return(text, text.len(), false);
        assert_eq!((start, end, replacement.as_str()), (6, 6, "\n- "));
    }
}
