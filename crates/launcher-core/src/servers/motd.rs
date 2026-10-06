//! Server description ("MOTD") → styled spans. Handles both chat
//! components (`{"text", "color", "bold", "extra": [...]}`) and legacy `§`
//! formatting codes inside strings.

use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

/// Hard cap so a hostile server cannot flood the UI.
const MAX_CHARS: usize = 512;
const MAX_SPANS: usize = 128;
const MAX_DEPTH: usize = 16;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MotdSpan {
    pub text: String,
    /// `#rrggbb`; `None` = default text colour.
    pub color: Option<String>,
    pub bold: bool,
    pub italic: bool,
    pub underlined: bool,
    pub strikethrough: bool,
}

#[derive(Debug, Clone, Default)]
struct Style {
    color: Option<String>,
    bold: bool,
    italic: bool,
    underlined: bool,
    strikethrough: bool,
}

const NAMED: [(&str, &str); 16] = [
    ("black", "#000000"),
    ("dark_blue", "#0000aa"),
    ("dark_green", "#00aa00"),
    ("dark_aqua", "#00aaaa"),
    ("dark_red", "#aa0000"),
    ("dark_purple", "#aa00aa"),
    ("gold", "#ffaa00"),
    ("gray", "#aaaaaa"),
    ("dark_gray", "#555555"),
    ("blue", "#5555ff"),
    ("green", "#55ff55"),
    ("aqua", "#55ffff"),
    ("red", "#ff5555"),
    ("light_purple", "#ff55ff"),
    ("yellow", "#ffff55"),
    ("white", "#ffffff"),
];

fn color(name: &str) -> Option<String> {
    let name = name.trim().to_ascii_lowercase();
    if let Some(hex) = name.strip_prefix('#')
        && hex.len() == 6
        && hex.chars().all(|c| c.is_ascii_hexdigit())
    {
        return Some(name);
    }
    NAMED
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, c)| (*c).to_owned())
}

struct Builder {
    spans: Vec<MotdSpan>,
    chars: usize,
}

impl Builder {
    fn push(&mut self, text: &str, s: &Style) {
        if text.is_empty() || self.chars >= MAX_CHARS {
            return;
        }
        let text: String = text
            .chars()
            .filter(|c| *c == '\n' || !c.is_control())
            .take(MAX_CHARS - self.chars)
            .collect();
        self.chars += text.chars().count();
        if let Some(last) = self.spans.last_mut()
            && last.color == s.color
            && (last.bold, last.italic, last.underlined, last.strikethrough)
                == (s.bold, s.italic, s.underlined, s.strikethrough)
        {
            last.text.push_str(&text);
            return;
        }
        if self.spans.len() < MAX_SPANS {
            self.spans.push(MotdSpan {
                text,
                color: s.color.clone(),
                bold: s.bold,
                italic: s.italic,
                underlined: s.underlined,
                strikethrough: s.strikethrough,
            });
        }
    }

    /// A string that may contain `§x` codes; they start from `base`.
    fn legacy(&mut self, text: &str, base: &Style) {
        let mut style = base.clone();
        let mut buf = String::new();
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            if c != '§' {
                buf.push(c);
                continue;
            }
            let Some(code) = chars.next() else { break };
            self.push(&std::mem::take(&mut buf), &style);
            let code = code.to_ascii_lowercase();
            match code {
                '0'..='9' | 'a'..='f' => {
                    let i = code.to_digit(16).expect("hex digit") as usize;
                    // A colour code resets the formatting, as in the game.
                    style = Style {
                        color: Some(NAMED[i].1.to_owned()),
                        ..Style::default()
                    };
                }
                'l' => style.bold = true,
                'o' => style.italic = true,
                'n' => style.underlined = true,
                'm' => style.strikethrough = true,
                'r' => style = base.clone(),
                _ => {} // `k` (obfuscated) and unknown codes
            }
        }
        self.push(&buf, &style);
    }

    fn component(&mut self, v: &Value, parent: &Style, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        match v {
            Value::String(s) => self.legacy(s, parent),
            Value::Array(items) => {
                // An array is "first element + rest as its extras".
                for item in items {
                    self.component(item, parent, depth + 1);
                }
            }
            Value::Object(o) => {
                let mut s = parent.clone();
                if let Some(c) = o.get("color").and_then(Value::as_str).and_then(color) {
                    s.color = Some(c);
                }
                let flag = |k: &str, cur: bool| o.get(k).and_then(Value::as_bool).unwrap_or(cur);
                s.bold = flag("bold", s.bold);
                s.italic = flag("italic", s.italic);
                s.underlined = flag("underlined", s.underlined);
                s.strikethrough = flag("strikethrough", s.strikethrough);
                if let Some(t) = o.get("text") {
                    match t {
                        Value::String(t) => self.legacy(t, &s),
                        other => self.component(other, &s, depth + 1),
                    }
                } else if let Some(t) = o.get("translate").and_then(Value::as_str) {
                    self.legacy(t, &s);
                }
                if let Some(Value::Array(extra)) = o.get("extra") {
                    for e in extra {
                        self.component(e, &s, depth + 1);
                    }
                }
            }
            Value::Number(n) => self.push(&n.to_string(), parent),
            Value::Bool(b) => self.push(&b.to_string(), parent),
            Value::Null => {}
        }
    }
}

/// Styled spans of a status `description`.
pub fn parse(description: &Value) -> Vec<MotdSpan> {
    let mut b = Builder {
        spans: Vec::new(),
        chars: 0,
    };
    b.component(description, &Style::default(), 0);
    // Trim trailing whitespace lines the way the server list shows them.
    while let Some(last) = b.spans.last_mut() {
        let trimmed = last.text.trim_end().len();
        if trimmed == 0 {
            b.spans.pop();
        } else {
            last.text.truncate(trimmed);
            break;
        }
    }
    b.spans
}

/// Plain text without styling (search, tooltips).
pub fn plain(spans: &[MotdSpan]) -> String {
    spans.iter().map(|s| s.text.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn legacy_codes() {
        let s = parse(&json!("§aGreen §lBold§r plain\n§cRed"));
        assert_eq!(plain(&s), "Green Bold plain\nRed");
        assert_eq!(s[0].color.as_deref(), Some("#55ff55"));
        assert!(!s[0].bold);
        assert!(s[1].bold && s[1].color.as_deref() == Some("#55ff55"));
        assert_eq!(s[2].color, None);
        assert_eq!(s.last().unwrap().color.as_deref(), Some("#ff5555"));
    }

    #[test]
    fn components_inherit_style() {
        let v = json!({
            "text": "",
            "extra": [
                {"text": "Mehbur", "color": "gold", "bold": true,
                 "extra": [{"text": "MC", "color": "#12AB34"}]},
                " ",
                {"text": "Server", "italic": true}
            ]
        });
        let s = parse(&v);
        assert_eq!(plain(&s), "MehburMC Server");
        assert_eq!(s[0].color.as_deref(), Some("#ffaa00"));
        assert!(s[1].bold);
        assert_eq!(s[1].color.as_deref(), Some("#12ab34"));
        assert!(s.last().unwrap().italic);
    }

    #[test]
    fn hostile_input_is_bounded() {
        let long = "x".repeat(10_000);
        assert_eq!(plain(&parse(&json!(long))).len(), MAX_CHARS);
        let mut deep = json!("bottom");
        for _ in 0..100 {
            deep = json!({ "text": "", "extra": [deep] });
        }
        assert_eq!(plain(&parse(&deep)), "");
        assert_eq!(plain(&parse(&json!("a\u{7}b\u{1b}c   \n  "))), "abc");
        assert!(
            parse(&json!({"color": "not-a-colour", "text": "t"}))[0]
                .color
                .is_none()
        );
    }
}
