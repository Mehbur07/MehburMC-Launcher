//! Java Edition news from the official launcher content feed.
//!
//! The feed is cached for an hour (stale copy used offline). Any failure
//! yields an empty list: news is decoration and must never block the UI.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ctx::Ctx;
use crate::net::cache::get_json_cached;

const TTL: Duration = Duration::from_secs(60 * 60);
const MAX_ITEMS: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NewsItem {
    pub id: String,
    pub title: String,
    pub text: String,
    /// `YYYY-MM-DD`
    pub date: String,
    /// Absolute image URL (fetch through `content_icon`; CSP blocks it).
    #[ts(optional)]
    pub image: Option<String>,
    /// minecraft.net article.
    #[ts(optional)]
    pub link: Option<String>,
}

#[derive(Deserialize)]
struct Feed {
    #[serde(default)]
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    id: String,
    title: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    news_type: Vec<String>,
    play_page_image: Option<Image>,
    news_page_image: Option<Image>,
    read_more_link: Option<String>,
}

#[derive(Deserialize)]
struct Image {
    url: String,
}

fn is_java(e: &Entry) -> bool {
    e.category == "Minecraft: Java Edition" || e.news_type.iter().any(|t| t == "Java")
}

fn absolute(base: &str, url: &str) -> Option<String> {
    if url.starts_with("https://") {
        Some(url.to_owned())
    } else if url.starts_with('/') {
        Some(format!("{}{url}", base.trim_end_matches('/')))
    } else {
        None
    }
}

fn convert(base: &str, feed: Feed) -> Vec<NewsItem> {
    let mut items: Vec<NewsItem> = feed
        .entries
        .into_iter()
        .filter(is_java)
        .map(|e| NewsItem {
            image: e
                .play_page_image
                .or(e.news_page_image)
                .and_then(|i| absolute(base, &i.url)),
            // Only https links; the opener allowlist checks the host again.
            link: e.read_more_link.filter(|l| l.starts_with("https://")),
            id: e.id,
            title: e.title,
            text: e.text,
            date: e.date,
        })
        .collect();
    items.sort_by(|a, b| b.date.cmp(&a.date));
    items.truncate(MAX_ITEMS);
    items
}

pub async fn java_news(ctx: &Ctx) -> Vec<NewsItem> {
    let base = &ctx.endpoints.launcher_content;
    let url = format!("{}/v2/news.json", base.trim_end_matches('/'));
    let file = ctx.paths.cache().join("news.json");
    match get_json_cached::<Feed>(&ctx.http, &url, &file, TTL).await {
        Ok(feed) => convert(base, feed),
        Err(e) => {
            tracing::warn!(error = %e.detail(), "news unavailable");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_java_entries_newest_first() {
        let feed: Feed = serde_json::from_value(serde_json::json!({"entries": [
            {"id": "a", "title": "Dungeons", "date": "2026-09-30", "category": "Minecraft Dungeons",
             "newsType": ["Dungeons"]},
            {"id": "b", "title": "Old", "date": "2026-08-01", "category": "Minecraft: Java Edition",
             "playPageImage": {"url": "/v2/images/x.jpg"}, "readMoreLink": "https://www.minecraft.net/a"},
            {"id": "c", "title": "New", "date": "2026-09-28", "category": "Minecraft for Windows",
             "newsType": ["Java", "Bedrock"], "readMoreLink": "javascript:alert(1)"}
        ]}))
        .unwrap();
        let items = convert("https://launchercontent.mojang.com", feed);
        assert_eq!(
            items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
            ["c", "b"]
        );
        assert_eq!(
            items[1].image.as_deref(),
            Some("https://launchercontent.mojang.com/v2/images/x.jpg")
        );
        assert_eq!(items[0].link, None, "non-https links are dropped");
    }
}
