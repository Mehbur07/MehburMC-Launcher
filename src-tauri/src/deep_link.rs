//! `mehburmc://` links from the MehburMC Library website (K112).
//!
//! Any web page can open such a link, so it only ever *shows* something: the
//! library item is focused in the Library tab, nothing is installed. Only
//! `mehburmc://library/<id>` is accepted; anything else is ignored.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_deep_link::DeepLinkExt;

use crate::commands::play;

/// The scheme registered for the launcher (also in `tauri.conf.json`).
pub const SCHEME: &str = "mehburmc";
/// Tells the UI that a link is waiting in [`PendingLink`].
pub const EVENT: &str = "app://deep-link";

/// Largest id accepted: ids are Postgres `bigint`s, but the UI keeps them as
/// JavaScript numbers, which are exact only up to 2^53 - 1.
const MAX_ID: i64 = (1 << 53) - 1;

/// What a link asks the launcher to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkTarget {
    LibraryItem(i64),
}

/// The last link not yet picked up by the UI. Kept in Rust (not only sent as
/// an event) so a link survives the UI loading, the sign-in screen and a
/// window re-created after a game (K93).
#[derive(Default)]
pub struct PendingLink(Mutex<Option<LinkTarget>>);

/// Parses `mehburmc://library/<id>` (a trailing slash is allowed). The id is
/// plain decimal digits: no sign, no leading zero, at most [`MAX_ID`].
pub fn parse(url: &str) -> Option<LinkTarget> {
    let rest = url.strip_prefix(SCHEME)?.strip_prefix("://")?;
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    let (host, id) = rest.split_once('/')?;
    if !host.eq_ignore_ascii_case("library") {
        return None;
    }
    if id.is_empty() || id.len() > 16 || id.starts_with('0') {
        return None;
    }
    if !id.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let id: i64 = id.parse().ok()?;
    (id <= MAX_ID).then_some(LinkTarget::LibraryItem(id))
}

/// Wires the deep-link plugin up: registers the scheme for portable copies,
/// then handles the link the launcher was started with and every later one
/// (a second start is forwarded here by the single-instance plugin).
pub fn setup(app: &AppHandle) {
    app.manage(PendingLink::default());

    // The installer registers the scheme; a portable copy (or one started
    // from a moved folder) registers itself. The last launcher started wins.
    #[cfg(all(windows, not(debug_assertions)))]
    if let Err(e) = app.deep_link().register_all() {
        tracing::warn!("could not register the {SCHEME}:// links: {e}");
    }

    if let Ok(Some(urls)) = app.deep_link().get_current() {
        receive(app, urls.iter().map(|u| u.as_str()));
    }
    let handle = app.clone();
    app.deep_link().on_open_url(move |event| {
        let urls = event.urls();
        if receive(&handle, urls.iter().map(|u| u.as_str())) {
            // Also when the window was closed for a game (K93).
            play::reopen_window(&handle);
        }
    });
}

/// Keeps the last valid link and tells the UI. Returns whether one was valid.
fn receive<'a>(app: &AppHandle, urls: impl Iterator<Item = &'a str>) -> bool {
    let Some(target) = urls.filter_map(parse).last() else {
        tracing::info!("ignored a {SCHEME}:// link that is not a library item");
        return false;
    };
    if let Some(pending) = app.try_state::<PendingLink>() {
        *pending.0.lock().expect("pending link lock") = Some(target);
    }
    let _ = app.emit(EVENT, ());
    true
}

/// Hands the waiting link to the UI once: the library item id, if any.
#[tauri::command]
pub fn take_deep_link(pending: State<'_, PendingLink>) -> Option<i64> {
    let taken = pending.0.lock().expect("pending link lock").take();
    taken.map(|LinkTarget::LibraryItem(id)| id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_links() {
        assert_eq!(
            parse("mehburmc://library/42"),
            Some(LinkTarget::LibraryItem(42))
        );
        assert_eq!(
            parse("mehburmc://library/42/"),
            Some(LinkTarget::LibraryItem(42))
        );
        assert_eq!(
            parse("mehburmc://LIBRARY/7"),
            Some(LinkTarget::LibraryItem(7))
        );
        assert_eq!(
            parse("mehburmc://library/9007199254740991"),
            Some(LinkTarget::LibraryItem(MAX_ID))
        );
    }

    #[test]
    fn everything_else_is_ignored() {
        for bad in [
            "",
            "mehburmc://",
            "mehburmc://library",
            "mehburmc://library/",
            "mehburmc://library/0",
            "mehburmc://library/007",
            "mehburmc://library/-1",
            "mehburmc://library/+1",
            "mehburmc://library/1.5",
            "mehburmc://library/12abc",
            "mehburmc://library/1/2",
            "mehburmc://library/1?install=1",
            "mehburmc://library/1#x",
            "mehburmc://library/%31",
            "mehburmc://library/9007199254740992",
            "mehburmc://library/99999999999999999999",
            "mehburmc://install/1",
            "mehburmc:library/1",
            "https://library/1",
            "MEHBURMC://library/1x",
        ] {
            assert_eq!(parse(bad), None, "{bad}");
        }
    }
}
