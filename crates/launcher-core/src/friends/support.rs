//! Admine mesaj (ARCHITECTURE K101). Oyuncu konu açar, bütün adminler görüp
//! cevaplar; banlı oyuncular da itiraz için yazabilir. Her kural sunucuda
//! (`supabase/support.sql`); bu modül yalnızca RPC'leri çağırır.

use serde::{Deserialize, Serialize};
use serde_json::json;
use ts_rs::TS;

use super::FriendsClient;
use crate::error::{CoreError, Result};

/// Sunucunun fırlattığı kodlar; her birinin `errors.support.*` çevirisi var.
pub const SUPPORT_ERRORS: &[&str] = &[
    "support.membersOnly",
    "support.subjectInvalid",
    "support.tooManyOpen",
    "support.closed",
    "support.notFound",
    "support.serverNotReady",
];

pub const MAX_SUBJECT_CHARS: usize = 80;
pub const MAX_BODY_CHARS: usize = 2000;

/// Oyuncunun kendi konusu.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SupportThread {
    #[ts(type = "number")]
    pub id: i64,
    pub subject: String,
    pub open: bool,
    pub created_at: String,
    pub updated_at: String,
    /// Okunmamış admin cevapları.
    pub unread: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SupportMessage {
    #[ts(type = "number")]
    pub id: i64,
    pub from_admin: bool,
    /// Yazan adminin adı; yalnızca adminler görür.
    pub author: Option<String>,
    pub body: String,
    pub created_at: String,
    pub mine: bool,
}

/// Admin ekranındaki konu satırı.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AdminSupportThread {
    #[ts(type = "number")]
    pub id: i64,
    /// Konuyu açan oyuncunun kullanıcı id'si.
    pub owner: String,
    pub names: Vec<String>,
    pub banned: bool,
    pub subject: String,
    pub open: bool,
    pub created_at: String,
    pub updated_at: String,
    /// Okunmamış oyuncu mesajları.
    pub unread: u32,
    pub last_from_admin: bool,
    /// Son mesajın başı.
    pub preview: String,
}

/// Rozetler: oyuncunun okunmamış cevapları, admin için cevap bekleyen konular.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SupportUnread {
    pub mine: u32,
    pub admin: u32,
}

#[derive(Deserialize)]
struct ThreadRow {
    id: i64,
    subject: String,
    status: String,
    created_at: String,
    updated_at: String,
    unread: Option<i64>,
}

#[derive(Deserialize)]
struct MessageRow {
    id: i64,
    from_admin: bool,
    author: Option<String>,
    body: String,
    created_at: String,
    mine: Option<bool>,
}

#[derive(Deserialize)]
struct AdminThreadRow {
    id: i64,
    owner: String,
    names: Option<Vec<String>>,
    banned: bool,
    subject: String,
    status: String,
    created_at: String,
    updated_at: String,
    unread: Option<i64>,
    last_from_admin: bool,
    preview: Option<String>,
}

#[derive(Deserialize)]
struct UnreadRow {
    mine: Option<i64>,
    admin: Option<i64>,
}

fn count(n: Option<i64>) -> u32 {
    u32::try_from(n.unwrap_or(0).max(0)).unwrap_or(u32::MAX)
}

/// Boşlukları sadeleştirir ve uzunluğu denetler; sunucu da aynısını yapar.
fn clean_subject(s: &str) -> Result<String> {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.is_empty() || s.chars().count() > MAX_SUBJECT_CHARS {
        return Err(CoreError::Friends("support.subjectInvalid"));
    }
    Ok(s)
}

fn clean_body(s: &str) -> Result<&str> {
    let s = s.trim();
    if s.is_empty() || s.chars().count() > MAX_BODY_CHARS {
        return Err(CoreError::Friends("friends.messageInvalid"));
    }
    Ok(s)
}

/// support.sql çalıştırılmamış bir sunucu: anlaşılır bir hataya çevirir.
fn not_ready(e: CoreError) -> CoreError {
    if super::library::missing_function(&e) {
        CoreError::Friends("support.serverNotReady")
    } else {
        e
    }
}

impl FriendsClient {
    pub async fn support_threads(&self) -> Result<Vec<SupportThread>> {
        let rows: Vec<ThreadRow> = self
            .rpc("support_my_threads", json!({}))
            .await
            .map_err(not_ready)?;
        Ok(rows
            .into_iter()
            .map(|r| SupportThread {
                id: r.id,
                subject: r.subject,
                open: r.status == "open",
                created_at: r.created_at,
                updated_at: r.updated_at,
                unread: count(r.unread),
            })
            .collect())
    }

    /// Yeni konu açar; konunun id'sini döndürür.
    pub async fn support_open(&self, subject: &str, body: &str) -> Result<i64> {
        let subject = clean_subject(subject)?;
        let body = clean_body(body)?;
        self.rpc(
            "support_open",
            json!({ "p_subject": subject, "p_body": body }),
        )
        .await
        .map_err(not_ready)
    }

    /// Konuya yazar; mesajın id'sini döndürür.
    pub async fn support_send(&self, thread: i64, body: &str) -> Result<i64> {
        let body = clean_body(body)?;
        self.rpc(
            "support_send",
            json!({ "p_thread": thread, "p_body": body }),
        )
        .await
        .map_err(not_ready)
    }

    /// `after` id'sinden sonraki mesajlar (`None` = hepsi).
    pub async fn support_messages(
        &self,
        thread: i64,
        after: Option<i64>,
    ) -> Result<Vec<SupportMessage>> {
        let rows: Vec<MessageRow> = self
            .rpc(
                "support_thread_messages",
                json!({ "p_thread": thread, "p_after": after.unwrap_or(0) }),
            )
            .await
            .map_err(not_ready)?;
        Ok(rows
            .into_iter()
            .map(|r| SupportMessage {
                id: r.id,
                from_admin: r.from_admin,
                author: r.author,
                body: r.body,
                created_at: r.created_at,
                mine: r.mine.unwrap_or(false),
            })
            .collect())
    }

    pub async fn support_mark_read(&self, thread: i64) -> Result<()> {
        self.rpc_void("support_mark_read", json!({ "p_thread": thread }))
            .await
            .map_err(not_ready)
    }

    /// Kapatır (`false`) ya da yeniden açar (`true`, yalnızca adminler).
    pub async fn support_set_open(&self, thread: i64, open: bool) -> Result<()> {
        self.rpc_void(
            "support_set_open",
            json!({ "p_thread": thread, "p_open": open }),
        )
        .await
        .map_err(not_ready)
    }

    /// Rozet sayıları. Sunucu henüz hazır değilse sıfır döner: rozet her
    /// birkaç dakikada sorulur, her seferinde hata göstermek gereksiz.
    pub async fn support_unread(&self) -> Result<SupportUnread> {
        match self
            .rpc::<Vec<UnreadRow>>("support_unread", json!({}))
            .await
        {
            Ok(rows) => Ok(rows
                .first()
                .map_or_else(SupportUnread::default, |r| SupportUnread {
                    mine: count(r.mine),
                    admin: count(r.admin),
                })),
            Err(e) if super::library::missing_function(&e) => Ok(SupportUnread::default()),
            Err(e) => Err(e),
        }
    }

    /// Adminler: açık (`true`) ya da kapalı konular.
    pub async fn admin_support_threads(&self, open: bool) -> Result<Vec<AdminSupportThread>> {
        let rows: Vec<AdminThreadRow> = self
            .rpc("admin_support_threads", json!({ "p_open": open }))
            .await
            .map_err(not_ready)?;
        Ok(rows
            .into_iter()
            .map(|r| AdminSupportThread {
                id: r.id,
                owner: r.owner,
                names: r.names.unwrap_or_default(),
                banned: r.banned,
                subject: r.subject,
                open: r.status == "open",
                created_at: r.created_at,
                updated_at: r.updated_at,
                unread: count(r.unread),
                last_from_admin: r.last_from_admin,
                preview: r.preview.unwrap_or_default(),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::tests::{FRIEND, signed_in};
    use super::*;

    #[tokio::test]
    async fn opens_and_writes_with_cleaned_text() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/support_open"))
            .and(body_json(
                json!({ "p_subject": "Ban itirazı", "p_body": "Merhaba" }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(7)))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/support_send"))
            .and(body_json(json!({ "p_thread": 7, "p_body": "devam" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(9)))
            .expect(1)
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        assert_eq!(
            c.support_open("  Ban \n itirazı ", "  Merhaba ")
                .await
                .unwrap(),
            7
        );
        assert_eq!(c.support_send(7, " devam\n").await.unwrap(), 9);
    }

    #[tokio::test]
    async fn bad_text_never_reaches_the_server() {
        let server = MockServer::builder().start().await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let code = |e: CoreError| e.code();
        assert_eq!(
            code(c.support_open("   ", "x").await.unwrap_err()),
            "support.subjectInvalid"
        );
        assert_eq!(
            code(c.support_open(&"k".repeat(81), "x").await.unwrap_err()),
            "support.subjectInvalid"
        );
        assert_eq!(
            code(c.support_open("Konu", " \n ").await.unwrap_err()),
            "friends.messageInvalid"
        );
        assert_eq!(
            code(c.support_send(1, &"ç".repeat(2001)).await.unwrap_err()),
            "friends.messageInvalid"
        );
        // 2000 karakter sınırı bayt değil karakter sayar.
        assert!(clean_body(&"ç".repeat(2000)).is_ok());
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn maps_rows_and_server_codes() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/support_my_threads"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "id": 3, "subject": "Soru", "status": "closed", "created_at": "2026-11-10T10:00:00Z",
                  "updated_at": "2026-11-10T11:00:00Z", "unread": 2 }
            ])))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/support_thread_messages"))
            .and(body_json(json!({ "p_thread": 3, "p_after": 0 })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "id": 1, "from_admin": false, "author": null, "body": "a",
                  "created_at": "2026-11-10T10:00:00Z", "mine": true },
                { "id": 2, "from_admin": true, "author": null, "body": "b",
                  "created_at": "2026-11-10T11:00:00Z", "mine": false }
            ])))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/admin_support_threads"))
            .and(body_json(json!({ "p_open": true })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "id": 3, "owner": FRIEND, "names": null, "banned": true, "subject": "Soru",
                  "status": "open", "created_at": "2026-11-10T10:00:00Z",
                  "updated_at": "2026-11-10T11:00:00Z", "unread": 1, "last_from_admin": false,
                  "preview": null }
            ])))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/support_send"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(json!({ "code": "P0001", "message": "support.closed" })),
            )
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;

        let threads = c.support_threads().await.unwrap();
        assert_eq!(
            (threads[0].id, threads[0].open, threads[0].unread),
            (3, false, 2)
        );
        let msgs = c.support_messages(3, None).await.unwrap();
        assert!(msgs[0].mine && !msgs[0].from_admin);
        assert!(!msgs[1].mine && msgs[1].from_admin);
        let admin = c.admin_support_threads(true).await.unwrap();
        assert_eq!(
            (
                admin[0].open,
                admin[0].banned,
                admin[0].names.len(),
                admin[0].preview.as_str()
            ),
            (true, true, 0, "")
        );
        assert_eq!(
            c.support_send(3, "x").await.unwrap_err().code(),
            "support.closed"
        );
    }

    #[tokio::test]
    async fn old_server_is_reported_once_and_badge_stays_quiet() {
        let server = MockServer::builder().start().await;
        let missing = ResponseTemplate::new(404).set_body_json(json!({
            "code": "PGRST202",
            "message": "Could not find the function public.support_unread without parameters in the schema cache"
        }));
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/support_unread"))
            .respond_with(missing.clone())
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/support_my_threads"))
            .respond_with(missing)
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        assert_eq!(c.support_unread().await.unwrap(), SupportUnread::default());
        assert_eq!(
            c.support_threads().await.unwrap_err().code(),
            "support.serverNotReady"
        );
    }

    #[tokio::test]
    async fn badge_counts_come_from_one_row() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/support_unread"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!([{ "mine": 2, "admin": 5 }])),
            )
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        assert_eq!(
            c.support_unread().await.unwrap(),
            SupportUnread { mine: 2, admin: 5 }
        );
    }
}
