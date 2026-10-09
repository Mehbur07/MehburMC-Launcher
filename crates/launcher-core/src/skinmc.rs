//! Browsing skins on SkinMC (skinmc.net, ARCHITECTURE K78).
//!
//! SkinMC has no public API, so this reads the same pages a browser does:
//! a list page (`/skins?latest=1`, `/skins/tagged/<tag>`) gives skin ids and
//! their creators, and each skin's PNG comes from the site's own download
//! link (`/api/v1/renders/skins/<id>/skin`). Nothing is fetched in bulk:
//! one page (about 50 skins) at a time, following the site's own "next"
//! link, and only while the user browses. Skin PNGs are verified like any
//! imported skin and cached on disk.

use std::sync::LazyLock;

use futures_util::{StreamExt, stream};
use regex::Regex;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::auth::avatar::data_uri;
use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::skin::SkinModel;
use crate::skin::image;

/// The public site; "next" links in its pages point here.
const SITE: &str = "https://skinmc.net";
/// Skin PNGs fetched in parallel for one page.
const PARALLEL: usize = 8;
const MAX_TAG_CHARS: usize = 40;
const MAX_AUTHOR_CHARS: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SkinMcSort {
    Latest,
    Today,
    Week,
    Month,
    Random,
}

impl SkinMcSort {
    fn query(self) -> &'static str {
        match self {
            Self::Latest => "latest=1",
            Self::Today => "today=1",
            Self::Week => "this_week=1",
            Self::Month => "this_month=1",
            Self::Random => "random=1",
        }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SkinMcSkin {
    /// SkinMC skin id (UUID).
    pub id: String,
    /// Account name of the creator ("" when the page shows none).
    pub author: String,
    pub model: SkinModel,
    /// The skin PNG as a data URI (verified).
    pub data_uri: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SkinMcPage {
    pub skins: Vec<SkinMcSkin>,
    /// Opaque token for the next page (the site's own link), if any.
    #[ts(optional)]
    pub next: Option<String>,
}

/// A card on a list page.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Card {
    id: String,
    author: String,
}

static CARD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"href="[^"]*/skins/([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})""#,
    )
    .expect("card regex")
});
static AUTHOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"fw-medium"\s*>\s*([^<]{1,64}?)\s*<"#).expect("author regex"));
static NEXT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"href="([^"]+)"\s+rel="next""#).expect("next regex"));

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

/// Skin cards in page order (each id once) and the next-page link.
fn parse_list(html: &str) -> (Vec<Card>, Option<String>) {
    let mut cards: Vec<Card> = Vec::new();
    for m in CARD.captures_iter(html) {
        let id = m[1].to_owned();
        if cards.iter().any(|c| c.id == id) {
            continue;
        }
        // The creator's name follows the card within a few hundred bytes.
        let end = m.get(0).map_or(0, |g| g.end());
        let window = &html[end..html.len().min(end + 3000)];
        let author = AUTHOR
            .captures(window)
            .map(|a| unescape(a[1].trim()))
            .filter(|a| !a.contains('>'))
            .map(|a| a.chars().take(MAX_AUTHOR_CHARS).collect())
            .unwrap_or_default();
        cards.push(Card { id, author });
    }
    let next = NEXT.captures(html).map(|n| unescape(&n[1]));
    (cards, next)
}

/// `skin-name` style slug for a tag search; empty when nothing usable is left.
fn tag_slug(tag: &str) -> String {
    let mut out = String::new();
    for c in tag.trim().to_lowercase().chars() {
        let c = match c {
            'ç' => 'c',
            'ğ' => 'g',
            'ı' => 'i',
            'ö' => 'o',
            'ş' => 's',
            'ü' => 'u',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-')
        .chars()
        .take(MAX_TAG_CHARS)
        .collect()
}

/// The URL of the requested page. A `next` token must be one of the site's
/// own list links (never another host or path).
fn page_url(base: &str, sort: SkinMcSort, tag: Option<&str>, next: Option<&str>) -> Result<String> {
    let base = base.trim_end_matches('/');
    if let Some(n) = next {
        let rest = n
            .strip_prefix(SITE)
            .or_else(|| n.strip_prefix(base))
            .filter(|r| r.starts_with("/skins?") || r.starts_with("/skins/tagged/"))
            .filter(|r| !r.contains("..") && !r.contains('#'))
            .ok_or(CoreError::SkinMc("skinmc.badLink"))?;
        return Ok(format!("{base}{rest}"));
    }
    match tag.map(tag_slug) {
        Some(slug) if slug.is_empty() => Err(CoreError::SkinMc("skinmc.badTag")),
        Some(slug) => Ok(format!("{base}/skins/tagged/{slug}")),
        None => Ok(format!("{base}/skins?{}", sort.query())),
    }
}

/// One skin's PNG: from the cache or the site, verified as a skin.
async fn skin_png(ctx: &Ctx, id: &str) -> Option<(Vec<u8>, SkinModel)> {
    let cached = ctx.paths.cache().join("skinmc").join(format!("{id}.png"));
    let check = |bytes: &[u8]| {
        let img = image::decode(bytes).ok()?;
        image::check_skin(&img).ok()?;
        Some(image::detect_model(&img))
    };
    if let Ok(bytes) = std::fs::read(&cached)
        && let Some(model) = check(&bytes)
    {
        return Some((bytes, model));
    }
    let base = ctx.endpoints.skinmc.trim_end_matches('/');
    let url = format!("{base}/api/v1/renders/skins/{id}/skin");
    let bytes = ctx
        .http
        .get_bytes(&url)
        .await
        .map_err(|e| tracing::debug!(error = %e.detail(), %id, "skinmc skin unavailable"))
        .ok()?;
    let model = check(&bytes)?;
    if let Err(e) = crate::fsutil::write_atomic(&cached, &bytes) {
        tracing::debug!(error = %e.detail(), "could not cache a skinmc skin");
    }
    Some((bytes, model))
}

/// One page of SkinMC skins: by sort order, or by tag when `tag` is given;
/// `next` continues a previous page. Skins whose PNG cannot be fetched or
/// verified are left out.
pub async fn browse(
    ctx: &Ctx,
    sort: SkinMcSort,
    tag: Option<&str>,
    next: Option<&str>,
) -> Result<SkinMcPage> {
    let url = page_url(&ctx.endpoints.skinmc, sort, tag, next)?;
    let html = ctx.http.get_bytes(&url).await?;
    let (cards, next) = parse_list(&String::from_utf8_lossy(&html));
    let skins = stream::iter(cards)
        .map(|c| async move {
            let (png, model) = skin_png(ctx, &c.id).await?;
            Some(SkinMcSkin {
                id: c.id,
                author: c.author,
                model,
                data_uri: data_uri(&png),
            })
        })
        .buffered(PARALLEL)
        .filter_map(|s| async move { s })
        .collect()
        .await;
    Ok(SkinMcPage { skins, next })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two cards and the pager, copied from a real list page (trimmed).
    const PAGE: &str = include_str!("../tests/fixtures/skinmc_list.html");

    #[test]
    fn reads_cards_authors_and_the_next_link() {
        let (cards, next) = parse_list(PAGE);
        assert_eq!(
            cards,
            [
                Card {
                    id: "0220e871-4ee0-4d1a-9064-e870b11b1b3a".into(),
                    author: "thekmi".into()
                },
                Card {
                    id: "02b8a2e5-c49f-4a4b-a64a-f2cceb71ac98".into(),
                    author: "Ann & Co".into()
                },
            ]
        );
        assert_eq!(
            next.as_deref(),
            Some("https://skinmc.net/skins?latest=1&cursor=eyJjcmVhdGVkX2F0Ijp0cnVlfQ")
        );
    }

    #[test]
    fn builds_list_urls_and_only_follows_site_links() {
        let b = "https://skinmc.net";
        assert_eq!(
            page_url(b, SkinMcSort::Week, None, None).unwrap(),
            "https://skinmc.net/skins?this_week=1"
        );
        assert_eq!(
            page_url(b, SkinMcSort::Latest, Some("  Kedi Kulağı! "), None).unwrap(),
            "https://skinmc.net/skins/tagged/kedi-kulagi"
        );
        assert_eq!(
            page_url(b, SkinMcSort::Latest, Some("?!"), None)
                .unwrap_err()
                .code(),
            "skinmc.badTag"
        );
        let next = "https://skinmc.net/skins?latest=1&cursor=abc";
        assert_eq!(
            page_url("http://127.0.0.1:9", SkinMcSort::Latest, None, Some(next)).unwrap(),
            "http://127.0.0.1:9/skins?latest=1&cursor=abc"
        );
        for bad in [
            "https://evil.example/skins?latest=1",
            "https://skinmc.net/auth/login",
            "https://skinmc.net.evil.example/skins?x",
            "https://skinmc.net/skins/tagged/../../auth",
        ] {
            assert!(
                page_url(b, SkinMcSort::Latest, None, Some(bad)).is_err(),
                "{bad}"
            );
        }
    }

    #[tokio::test]
    async fn browse_returns_verified_skins_and_caches_them() {
        use std::sync::Arc;

        use wiremock::matchers::{method, path, query_param};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        use crate::events::NullSink;
        use crate::net::{Allowlist, Http};
        use crate::paths::Paths;

        let server = MockServer::builder().start().await;
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::at(tmp.path().join("MehburMC"));
        paths.ensure_layout().unwrap();
        let mut ctx = Ctx::new(paths, Arc::new(NullSink), 4).unwrap();
        ctx.http = Http::new(Allowlist::with_loopback()).unwrap();
        ctx.endpoints.skinmc = server.uri();

        Mock::given(method("GET"))
            .and(path("/skins"))
            .and(query_param("latest", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_string(PAGE))
            .mount(&server)
            .await;
        // A slim skin (outer arm column left empty) …
        let slim = image::tests::png(64, 64, |x, y| {
            (50..52).contains(&x) && (16..20).contains(&y)
                || (54..56).contains(&x) && (20..32).contains(&y)
        });
        Mock::given(method("GET"))
            .and(path(
                "/api/v1/renders/skins/0220e871-4ee0-4d1a-9064-e870b11b1b3a/skin",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(slim))
            .expect(1)
            .mount(&server)
            .await;
        // … and one that is not a skin at all.
        Mock::given(method("GET"))
            .and(path(
                "/api/v1/renders/skins/02b8a2e5-c49f-4a4b-a64a-f2cceb71ac98/skin",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"<html>".to_vec()))
            .mount(&server)
            .await;

        let page = browse(&ctx, SkinMcSort::Latest, None, None).await.unwrap();
        assert_eq!(page.skins.len(), 1);
        let s = &page.skins[0];
        assert_eq!(s.author, "thekmi");
        assert_eq!(s.model, SkinModel::Slim);
        assert!(s.data_uri.starts_with("data:image/png;base64,"));
        assert!(page.next.as_deref().unwrap().contains("cursor="));
        // The second load reads the skin from the cache (the mock expects one call).
        assert_eq!(
            browse(&ctx, SkinMcSort::Latest, None, None)
                .await
                .unwrap()
                .skins
                .len(),
            1
        );
    }
}
