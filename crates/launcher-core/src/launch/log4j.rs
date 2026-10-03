//! Turns game output into structured log lines.
//!
//! Modern clients are launched with Mojang's log4j config, which prints
//! `<log4j:Event logger=".." timestamp=".." level=".." thread="..">` blocks
//! with the message (and optional throwable) in CDATA. Plain lines (old
//! versions, stderr) are passed through with a best-effort level guess.

use std::sync::LazyLock;

use regex::Regex;

use crate::events::LogLevel;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedLine {
    pub text: String,
    pub level: Option<LogLevel>,
    pub time_ms: Option<u64>,
    pub thread: Option<String>,
}

static ATTR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(\w+)="([^"]*)""#).unwrap());
static PLAIN_LEVEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[[^\]/]*/(TRACE|DEBUG|INFO|WARN|WARNING|ERROR|FATAL|SEVERE)\]|\[(TRACE|DEBUG|INFO|WARN|WARNING|ERROR|FATAL|SEVERE)\]")
        .unwrap()
});

/// Upper bound for a buffered XML event (protects against unterminated blocks).
const MAX_EVENT_BYTES: usize = 1024 * 1024;

#[derive(Default)]
pub struct Log4jParser {
    buf: Option<String>,
}

impl Log4jParser {
    /// Feeds one physical line; returns zero or one logical line.
    pub fn feed(&mut self, line: &str) -> Option<ParsedLine> {
        if let Some(buf) = &mut self.buf {
            buf.push('\n');
            buf.push_str(line);
            if line.contains("</log4j:Event>") || buf.len() > MAX_EVENT_BYTES {
                let event = self.buf.take().unwrap_or_default();
                return Some(parse_event(&event));
            }
            return None;
        }
        let trimmed = line.trim_start();
        if trimmed.starts_with("<log4j:Event") {
            if trimmed.contains("</log4j:Event>") {
                return Some(parse_event(trimmed));
            }
            self.buf = Some(trimmed.to_owned());
            return None;
        }
        Some(plain(line))
    }

    /// Flushes an unterminated event at end of stream.
    pub fn finish(&mut self) -> Option<ParsedLine> {
        self.buf.take().map(|b| plain(&b))
    }
}

fn plain(line: &str) -> ParsedLine {
    let level = PLAIN_LEVEL
        .captures(line)
        .and_then(|c| c.get(1).or_else(|| c.get(2)))
        .and_then(|m| LogLevel::parse(m.as_str()));
    ParsedLine {
        text: line.to_owned(),
        level,
        time_ms: None,
        thread: None,
    }
}

fn cdata<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}><![CDATA[");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find("]]>")? + start;
    Some(&xml[start..end])
}

fn parse_event(xml: &str) -> ParsedLine {
    let header_end = xml.find('>').unwrap_or(xml.len());
    let header = &xml[..header_end];
    let mut level = None;
    let mut time_ms = None;
    let mut thread = None;
    for c in ATTR.captures_iter(header) {
        match &c[1] {
            "level" => level = LogLevel::parse(&c[2]),
            "timestamp" => time_ms = c[2].parse().ok(),
            "thread" => thread = Some(unescape(&c[2])),
            _ => {}
        }
    }
    let mut text = cdata(xml, "log4j:Message").unwrap_or_default().to_owned();
    if let Some(t) = cdata(xml, "log4j:Throwable") {
        text.push('\n');
        text.push_str(t.trim_end());
    }
    ParsedLine {
        text,
        level,
        time_ms,
        thread,
    }
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiline_xml_event() {
        let mut p = Log4jParser::default();
        let lines = [
            r#"<log4j:Event logger="net.minecraft.client.Minecraft" timestamp="1791045458209" level="ERROR" thread="Download-2">"#,
            r#"  <log4j:Message><![CDATA[Failed to fetch user properties]]></log4j:Message>"#,
            r#"  <log4j:Throwable><![CDATA[com.mojang.authlib.exceptions.InvalidCredentialsException: Status: 401"#,
            "\tat com.mojang.Foo.bar(Foo.java:70)",
            r#"]]></log4j:Throwable>"#,
            "</log4j:Event>",
        ];
        let out: Vec<_> = lines.iter().filter_map(|l| p.feed(l)).collect();
        assert_eq!(out.len(), 1);
        let e = &out[0];
        assert_eq!(e.level, Some(LogLevel::Error));
        assert_eq!(e.time_ms, Some(1791045458209));
        assert_eq!(e.thread.as_deref(), Some("Download-2"));
        assert!(
            e.text
                .starts_with("Failed to fetch user properties\ncom.mojang.authlib")
        );
        assert!(e.text.contains("Foo.java:70"));
    }

    #[test]
    fn single_line_event_and_plain_lines() {
        let mut p = Log4jParser::default();
        let e = p
            .feed(r#"<log4j:Event logger="x" timestamp="1" level="INFO" thread="Render thread"><log4j:Message><![CDATA[Sound engine started]]></log4j:Message></log4j:Event>"#)
            .unwrap();
        assert_eq!(e.text, "Sound engine started");
        assert_eq!(e.thread.as_deref(), Some("Render thread"));

        let plain = p
            .feed("[12:00:00] [Client thread/WARN]: Something odd")
            .unwrap();
        assert_eq!(plain.level, Some(LogLevel::Warn));
        assert_eq!(p.feed("just text").unwrap().level, None);
    }

    #[test]
    fn unterminated_event_is_flushed() {
        let mut p = Log4jParser::default();
        assert!(p.feed("<log4j:Event level=\"INFO\">").is_none());
        assert!(p.finish().is_some());
        assert!(p.finish().is_none());
    }
}
