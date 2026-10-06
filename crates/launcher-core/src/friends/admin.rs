//! Admin tools (ARCHITECTURE K73). Every right is checked by the server
//! (`supabase/phase20.sql`); this module only calls the RPCs. Before rank 1
//! approves a library mod, this launcher downloads it, verifies its SHA-1
//! and scans it again — the uploader's report is never the only check.

use serde::{Deserialize, Serialize};
use serde_json::json;
use ts_rs::TS;

use super::FriendsClient;
use super::library::{LibraryMod, Row};
use crate::content::scan::{self, ScanReport, Verdict};
use crate::error::{CoreError, Result};

/// Codes the admin RPCs raise; each has an `errors.admin.*` translation.
pub const ADMIN_ERRORS: &[&str] = &[
    "admin.forbidden",
    "admin.self",
    "admin.targetAdmin",
    "admin.userNotFound",
    "admin.badUntil",
];

const MAX_NOTE_CHARS: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AdminModStatus {
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AdminMod {
    #[serde(rename = "mod")]
    #[ts(rename = "mod")]
    pub item: LibraryMod,
    /// Uploader's user id.
    pub owner: String,
    #[ts(type = "number")]
    pub reports: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ReportKind {
    Mod,
    Texture,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AdminReport {
    pub kind: ReportKind,
    #[ts(type = "number")]
    pub item_id: i64,
    pub name: String,
    pub author: String,
    pub owner: String,
    pub sha1: String,
    #[ts(type = "number")]
    pub reports: i64,
    pub reasons: Vec<String>,
    pub notes: Vec<String>,
    pub last_report: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AdminUser {
    pub user_id: String,
    pub names: Vec<String>,
    pub rank: u8,
    pub banned: bool,
    pub ban_until: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AdminBan {
    pub user_id: String,
    pub names: Vec<String>,
    /// `None` = permanent.
    pub until: Option<String>,
    pub reason: String,
    pub banned_by: Vec<String>,
    pub created_at: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AdminEntry {
    pub user_id: String,
    pub names: Vec<String>,
    pub rank: u8,
    pub created_at: String,
}

#[derive(Deserialize)]
struct ModRow {
    #[serde(flatten)]
    row: Row,
    reports: i64,
}

#[derive(Deserialize)]
struct ReportRow {
    kind: ReportKind,
    item_id: i64,
    name: String,
    author: String,
    owner: String,
    sha1: String,
    reports: i64,
    #[serde(default)]
    reasons: Option<Vec<String>>,
    #[serde(default)]
    notes: Option<Vec<String>>,
    last_report: String,
}

#[derive(Deserialize)]
struct UserRow {
    user_id: String,
    names: Option<Vec<String>>,
    rank: i32,
    banned: bool,
    ban_until: Option<String>,
}

#[derive(Deserialize)]
struct BanRow {
    user_id: String,
    names: Option<Vec<String>>,
    until: Option<String>,
    reason: Option<String>,
    banned_by: Option<Vec<String>>,
    created_at: String,
    active: bool,
}

#[derive(Deserialize)]
struct EntryRow {
    user_id: String,
    names: Option<Vec<String>>,
    rank: i32,
    created_at: String,
}

fn note(s: &str) -> String {
    s.trim()
        .chars()
        .filter(|c| *c == '\n' || !c.is_control())
        .take(MAX_NOTE_CHARS)
        .collect()
}

fn status_str(s: AdminModStatus) -> &'static str {
    match s {
        AdminModStatus::Pending => "pending",
        AdminModStatus::Approved => "approved",
        AdminModStatus::Rejected => "rejected",
    }
}

fn rank(r: i32) -> u8 {
    r.clamp(0, 2) as u8
}

impl FriendsClient {
    pub async fn admin_library(&self, status: AdminModStatus) -> Result<Vec<AdminMod>> {
        let rows: Vec<ModRow> = self
            .rpc("admin_library", json!({ "p_status": status_str(status) }))
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                let owner = r.row.owner.clone();
                Some(AdminMod {
                    item: r.row.into_mod()?,
                    owner,
                    reports: r.reports,
                })
            })
            .collect())
    }

    /// Downloads mod `id` (SHA-1 checked) and scans it in this launcher.
    pub async fn admin_scan_mod(&self, id: i64, status: AdminModStatus) -> Result<ScanReport> {
        let m = self
            .admin_library(status)
            .await?
            .into_iter()
            .find(|m| m.item.id == id)
            .ok_or(CoreError::Friends("library.notFound"))?;
        let bytes = self.library_jar(&m.owner, &m.item.sha1).await?;
        tokio::task::spawn_blocking(move || scan::scan(&bytes))
            .await
            .map_err(|e| CoreError::FriendsServer {
                status: 0,
                reason: e.to_string(),
            })
    }

    /// Rank 1: approves (after a fresh local scan) or rejects a mod.
    pub async fn admin_review_mod(
        &self,
        id: i64,
        status: AdminModStatus,
        approve: bool,
        review_note: &str,
    ) -> Result<()> {
        let scan = if approve {
            let r = self.admin_scan_mod(id, status).await?;
            if r.verdict == Verdict::Block {
                return Err(CoreError::Friends("library.blocked"));
            }
            serde_json::to_value(&r).ok()
        } else {
            None
        };
        self.rpc_void(
            "admin_review_library_mod",
            json!({ "p_id": id, "p_approve": approve, "p_note": note(review_note), "p_scan": scan }),
        )
        .await
    }

    /// Any admin: takes a published mod down.
    pub async fn admin_remove_mod(&self, id: i64, review_note: &str) -> Result<()> {
        self.rpc_void(
            "admin_remove_library_mod",
            json!({ "p_id": id, "p_note": note(review_note) }),
        )
        .await
    }

    pub async fn admin_reports(&self) -> Result<Vec<AdminReport>> {
        let rows: Vec<ReportRow> = self.rpc("admin_reports", json!({})).await?;
        Ok(rows
            .into_iter()
            .map(|r| AdminReport {
                kind: r.kind,
                item_id: r.item_id,
                name: r.name,
                author: r.author,
                owner: r.owner,
                sha1: r.sha1,
                reports: r.reports,
                reasons: r.reasons.unwrap_or_default(),
                notes: r.notes.unwrap_or_default(),
                last_report: r.last_report,
            })
            .collect())
    }

    pub async fn admin_dismiss_reports(&self, kind: ReportKind, id: i64) -> Result<()> {
        self.rpc_void(
            "admin_dismiss_reports",
            json!({ "p_kind": kind, "p_id": id }),
        )
        .await
    }

    /// Rank 1: hides a shared skin/cape.
    pub async fn admin_hide_texture(&self, id: i64) -> Result<()> {
        self.rpc_void("admin_hide_texture", json!({ "p_id": id }))
            .await
    }

    /// Accounts by name prefix or friend code.
    pub async fn admin_find_users(&self, query: &str) -> Result<Vec<AdminUser>> {
        let q: String = query.trim().chars().take(32).collect();
        if q.chars().count() < 2 {
            return Ok(Vec::new());
        }
        let rows: Vec<UserRow> = self
            .rpc("admin_find_users", json!({ "p_query": q }))
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| AdminUser {
                user_id: r.user_id,
                names: r.names.unwrap_or_default(),
                rank: rank(r.rank),
                banned: r.banned,
                ban_until: r.ban_until,
            })
            .collect())
    }

    /// Bans `user_id` for `hours` (`None` = permanent).
    pub async fn admin_ban(&self, user_id: &str, hours: Option<u32>, reason: &str) -> Result<()> {
        super::check_uuid(user_id)?;
        self.rpc_void(
            "admin_ban",
            json!({ "p_user": user_id, "p_hours": hours, "p_reason": note(reason) }),
        )
        .await
    }

    pub async fn admin_unban(&self, user_id: &str) -> Result<()> {
        super::check_uuid(user_id)?;
        self.rpc_void("admin_unban", json!({ "p_user": user_id }))
            .await
    }

    pub async fn admin_bans(&self) -> Result<Vec<AdminBan>> {
        let rows: Vec<BanRow> = self.rpc("admin_bans", json!({})).await?;
        Ok(rows
            .into_iter()
            .map(|r| AdminBan {
                user_id: r.user_id,
                names: r.names.unwrap_or_default(),
                until: r.until,
                reason: r.reason.unwrap_or_default(),
                banned_by: r.banned_by.unwrap_or_default(),
                created_at: r.created_at,
                active: r.active,
            })
            .collect())
    }

    pub async fn admin_list(&self) -> Result<Vec<AdminEntry>> {
        let rows: Vec<EntryRow> = self.rpc("admin_list", json!({})).await?;
        Ok(rows
            .into_iter()
            .map(|r| AdminEntry {
                user_id: r.user_id,
                names: r.names.unwrap_or_default(),
                rank: rank(r.rank),
                created_at: r.created_at,
            })
            .collect())
    }

    /// Rank 1: makes `user_id` a rank 2 admin (`true`) or removes it.
    pub async fn admin_set_rank(&self, user_id: &str, admin: bool) -> Result<()> {
        super::check_uuid(user_id)?;
        self.rpc_void(
            "admin_set_rank",
            json!({ "p_user": user_id, "p_rank": if admin { 2 } else { 0 } }),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::tests::{FRIEND, signed_in};
    use super::*;
    use crate::content::scan::tests::{class, fabric_json, jar};
    use crate::friends::avatar::sha1_hex;

    fn mod_row(id: i64, bytes: &[u8]) -> Value {
        json!({
            "id": id, "owner": FRIEND, "author": "Alex", "name": "Mod", "description": "",
            "mod_id": "coolmod", "version": "1.0", "loaders": ["fabric"], "game_versions": "",
            "sha1": sha1_hex(bytes), "size": bytes.len(), "file_name": "m.jar",
            "status": "pending", "scan": { "verdict": "pass", "findings": [] },
            "review_note": null, "created_at": "2026-10-06T10:00:00Z", "reports": 2
        })
    }

    async fn serve_pending(server: &MockServer, rows: Value, files: &[&Vec<u8>]) {
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/admin_library"))
            .and(body_json(json!({ "p_status": "pending" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(rows))
            .mount(server)
            .await;
        for f in files {
            Mock::given(method("GET"))
                .and(path(format!(
                    "/storage/v1/object/authenticated/library/{FRIEND}/{}.jar",
                    sha1_hex(f)
                )))
                .respond_with(ResponseTemplate::new(200).set_body_bytes((*f).clone()))
                .mount(server)
                .await;
        }
    }

    #[tokio::test]
    async fn approval_sends_this_launchers_scan() {
        let server = MockServer::builder().start().await;
        let good = jar(&[("fabric.mod.json", fabric_json())]);
        let evil = jar(&[
            ("fabric.mod.json", fabric_json()),
            (
                "a/B.class",
                class(&[], &["https://discord.com/api/webhooks/1/x"]),
            ),
        ]);
        serve_pending(
            &server,
            json!([mod_row(1, &good), mod_row(2, &evil)]),
            &[&good, &evil],
        )
        .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/admin_review_library_mod"))
            .and(wiremock::matchers::body_partial_json(json!({
                "p_id": 1, "p_approve": true, "p_note": "ok",
                "p_scan": { "verdict": "pass", "sha1": sha1_hex(&good) }
            })))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/admin_review_library_mod"))
            .and(body_json(
                json!({ "p_id": 2, "p_approve": false, "p_note": "zararlı", "p_scan": null }),
            ))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let list = c.admin_library(AdminModStatus::Pending).await.unwrap();
        assert_eq!((list.len(), list[0].reports), (2, 2));
        c.admin_review_mod(1, AdminModStatus::Pending, true, " ok ")
            .await
            .unwrap();
        // A file that fails this launcher's scan cannot be approved.
        assert_eq!(
            c.admin_review_mod(2, AdminModStatus::Pending, true, "")
                .await
                .unwrap_err()
                .code(),
            "library.blocked"
        );
        assert_eq!(
            c.admin_scan_mod(2, AdminModStatus::Pending)
                .await
                .unwrap()
                .verdict,
            Verdict::Block
        );
        c.admin_review_mod(2, AdminModStatus::Pending, false, "zararlı")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn users_bans_and_ranks() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/admin_find_users"))
            .and(body_json(json!({ "p_query": "mehb" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "user_id": FRIEND, "names": ["Mehbur"], "rank": 0, "banned": false, "ban_until": null }
            ])))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/admin_ban"))
            .and(body_json(
                json!({ "p_user": FRIEND, "p_hours": 168, "p_reason": "hile" }),
            ))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/admin_ban"))
            .and(body_json(
                json!({ "p_user": FRIEND, "p_hours": null, "p_reason": "" }),
            ))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(json!({ "code": "P0001", "message": "admin.targetAdmin" })),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/admin_set_rank"))
            .and(body_json(json!({ "p_user": FRIEND, "p_rank": 2 })))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        // Too short a query never reaches the server.
        assert!(c.admin_find_users(" m ").await.unwrap().is_empty());
        let users = c.admin_find_users("mehb").await.unwrap();
        assert_eq!(users[0].names, ["Mehbur"]);
        c.admin_ban(FRIEND, Some(168), " hile ").await.unwrap();
        assert_eq!(
            c.admin_ban(FRIEND, None, "").await.unwrap_err().code(),
            "admin.targetAdmin"
        );
        assert!(c.admin_ban("../x", None, "").await.is_err());
        c.admin_set_rank(FRIEND, true).await.unwrap();
    }

    #[tokio::test]
    async fn reports_parse_null_arrays() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/admin_reports"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "kind": "texture", "item_id": 3, "name": "Skin", "author": "Alex", "owner": FRIEND,
                "sha1": "a".repeat(40), "reports": 1, "reasons": ["spam"], "notes": null,
                "last_report": "2026-10-06T10:00:00Z"
            }])))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let r = c.admin_reports().await.unwrap();
        assert_eq!(r[0].kind, ReportKind::Texture);
        assert!(r[0].notes.is_empty());
    }
}
