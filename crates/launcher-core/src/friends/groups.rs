//! Group chats and the caller's own nicknames for friends (K85). The server
//! (`supabase/phase23.sql`) owns every rule: who may add, rename, remove,
//! read and post; these calls only forward and validate input.

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;
use ts_rs::TS;

use super::{FriendsClient, MAX_MESSAGE_CHARS, check_uuid};
use crate::error::{CoreError, Result};

/// Longest group name (matches the database check).
pub const MAX_GROUP_NAME: usize = 48;
/// Largest group, the creator included (matches the database).
pub const MAX_GROUP_MEMBERS: usize = 16;
/// Longest nickname (matches the database check).
pub const MAX_NICKNAME: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GroupMember {
    pub id: String,
    pub display_name: String,
    /// The caller's own name for this member (friends only).
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub online: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChatGroup {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub owner: String,
    /// The caller owns the group (renames, adds, removes).
    pub is_owner: bool,
    pub members: Vec<GroupMember>,
    #[ts(type = "number")]
    pub unread: i64,
    pub last_message_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GroupMessage {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub group_id: i64,
    pub sender: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Deserialize)]
struct GroupRow {
    id: i64,
    name: String,
    owner: Option<String>,
    members: Option<Vec<GroupMember>>,
    unread: i64,
    last_message_at: Option<String>,
}

#[derive(Deserialize)]
struct GroupMessageRow {
    id: i64,
    group_id: i64,
    sender: String,
    body: String,
    created_at: String,
}

impl From<GroupMessageRow> for GroupMessage {
    fn from(m: GroupMessageRow) -> Self {
        Self {
            id: m.id,
            group_id: m.group_id,
            sender: m.sender,
            body: m.body,
            created_at: m.created_at,
        }
    }
}

const MESSAGE_COLUMNS: &str = "id,group_id,sender,body,created_at";

fn clean_name(name: &str, max: usize, code: &'static str) -> Result<String> {
    let clean = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.is_empty() || clean.chars().count() > max {
        return Err(CoreError::Friends(code));
    }
    Ok(clean)
}

fn check_members(members: &[String]) -> Result<()> {
    if members.len() >= MAX_GROUP_MEMBERS {
        return Err(CoreError::Friends("friends.groupFull"));
    }
    members.iter().try_for_each(|m| check_uuid(m))
}

impl FriendsClient {
    /// The caller's name for `friend`, visible only to the caller; `None`
    /// or blank removes it.
    pub async fn set_nickname(&self, friend: &str, nickname: Option<&str>) -> Result<()> {
        check_uuid(friend)?;
        let nickname = match nickname.map(str::trim).filter(|n| !n.is_empty()) {
            Some(n) => Some(clean_name(n, MAX_NICKNAME, "friends.nicknameInvalid")?),
            None => None,
        };
        self.rpc_void(
            "set_nickname",
            json!({ "p_friend": friend, "p_nickname": nickname }),
        )
        .await
    }

    /// Groups of the caller, most recently active first.
    pub async fn groups(&self) -> Result<Vec<ChatGroup>> {
        let me = self.user_id().await?;
        let rows: Vec<GroupRow> = self.rpc("my_groups", json!({})).await?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let owner = r.owner.unwrap_or_default();
                ChatGroup {
                    id: r.id,
                    name: r.name,
                    is_owner: owner == me,
                    owner,
                    members: r.members.unwrap_or_default(),
                    unread: r.unread,
                    last_message_at: r.last_message_at,
                }
            })
            .collect())
    }

    /// Creates a group with friends of the caller; returns its id.
    pub async fn create_group(&self, name: &str, members: &[String]) -> Result<i64> {
        let name = clean_name(name, MAX_GROUP_NAME, "friends.groupNameInvalid")?;
        check_members(members)?;
        self.rpc(
            "create_group",
            json!({ "p_name": name, "p_members": members }),
        )
        .await
    }

    pub async fn rename_group(&self, group: i64, name: &str) -> Result<()> {
        let name = clean_name(name, MAX_GROUP_NAME, "friends.groupNameInvalid")?;
        self.rpc_void("rename_group", json!({ "p_group": group, "p_name": name }))
            .await
    }

    /// Owner only: adds friends of the owner.
    pub async fn add_group_members(&self, group: i64, members: &[String]) -> Result<()> {
        check_members(members)?;
        self.rpc_void(
            "add_group_members",
            json!({ "p_group": group, "p_members": members }),
        )
        .await
    }

    /// Owner only.
    pub async fn remove_group_member(&self, group: i64, member: &str) -> Result<()> {
        check_uuid(member)?;
        self.rpc_void(
            "remove_group_member",
            json!({ "p_group": group, "p_member": member }),
        )
        .await
    }

    pub async fn leave_group(&self, group: i64) -> Result<()> {
        self.rpc_void("leave_group", json!({ "p_group": group }))
            .await
    }

    /// Messages after `after` (oldest first), or the latest 100.
    pub async fn group_messages(
        &self,
        group: i64,
        after: Option<i64>,
    ) -> Result<Vec<GroupMessage>> {
        let mut url = reqwest::Url::parse("http://x/rest/v1/group_messages").expect("static url");
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("select", MESSAGE_COLUMNS);
            q.append_pair("group_id", &format!("eq.{group}"));
            match after {
                Some(id) => {
                    q.append_pair("id", &format!("gt.{id}"));
                    q.append_pair("order", "id.asc");
                    q.append_pair("limit", "200");
                }
                None => {
                    q.append_pair("order", "id.desc");
                    q.append_pair("limit", "100");
                }
            }
        }
        let path = format!("{}?{}", url.path(), url.query().unwrap_or(""));
        let mut rows: Vec<GroupMessageRow> = self.json(Method::GET, &path, None, &[]).await?;
        if after.is_none() {
            rows.reverse();
        }
        Ok(rows.into_iter().map(GroupMessage::from).collect())
    }

    pub async fn send_group_message(&self, group: i64, body: &str) -> Result<GroupMessage> {
        let body = body.trim();
        if body.is_empty() || body.chars().count() > MAX_MESSAGE_CHARS {
            return Err(CoreError::Friends("friends.messageInvalid"));
        }
        let me = self.user_id().await?;
        let mut rows: Vec<GroupMessageRow> = self
            .json(
                Method::POST,
                &format!("/rest/v1/group_messages?select={MESSAGE_COLUMNS}"),
                Some(&json!({ "group_id": group, "sender": me, "body": body })),
                &[("Prefer", "return=representation")],
            )
            .await
            // RLS refusal: no longer a member (removed, or the group is gone).
            .map_err(|e| match e {
                CoreError::Friends("friends.notFriends") => {
                    CoreError::Friends("friends.groupNotFound")
                }
                e => e,
            })?;
        rows.pop()
            .map(GroupMessage::from)
            .ok_or_else(|| CoreError::FriendsServer {
                status: 201,
                reason: "empty insert response".into(),
            })
    }

    pub async fn mark_group_read(&self, group: i64) -> Result<()> {
        self.rpc_void("mark_group_read", json!({ "p_group": group }))
            .await
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{body_json, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use serde_json::Value;

    use super::super::tests::{ME, signed_in};
    use super::*;

    const B: &str = "22222222-2222-2222-2222-222222222222";

    #[tokio::test]
    async fn lists_groups_with_owner_flag_and_members() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/my_groups"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "id": 7, "name": "Squad", "owner": ME, "unread": 2, "last_message_at": null,
                  "members": [
                    { "id": ME, "displayName": "Me", "nickname": null, "online": true },
                    { "id": B, "displayName": "Bob", "nickname": "Bobby", "online": false }
                  ] },
                { "id": 8, "name": "Other", "owner": B, "unread": 0, "last_message_at": "2026-10-21T10:00:00Z",
                  "members": null }
            ])))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let groups = c.groups().await.unwrap();
        assert!(groups[0].is_owner && !groups[1].is_owner);
        assert_eq!(groups[0].members[1].nickname.as_deref(), Some("Bobby"));
        assert_eq!(groups[0].unread, 2);
        assert!(groups[1].members.is_empty());
    }

    #[tokio::test]
    async fn validates_before_the_server() {
        let server = MockServer::builder().start().await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let code = |r: Result<i64>| r.unwrap_err().code();
        assert_eq!(
            code(c.create_group("   ", &[B.into()]).await),
            "friends.groupNameInvalid"
        );
        assert_eq!(
            code(c.create_group(&"x".repeat(49), &[B.into()]).await),
            "friends.groupNameInvalid"
        );
        let many: Vec<String> = (0..16).map(|_| B.to_owned()).collect();
        assert_eq!(
            code(c.create_group("Big", &many).await),
            "friends.groupFull"
        );
        assert_eq!(
            code(c.create_group("Bad", &["x';drop".into()]).await),
            "friends.notFriends"
        );
        assert_eq!(
            c.set_nickname(B, Some(&"n".repeat(33)))
                .await
                .unwrap_err()
                .code(),
            "friends.nicknameInvalid"
        );
        assert_eq!(
            c.send_group_message(1, "  ").await.unwrap_err().code(),
            "friends.messageInvalid"
        );
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn forwards_cleaned_names_and_nicknames() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/create_group"))
            .and(body_json(
                json!({ "p_name": "Yeni Ekip", "p_members": [B] }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(42)))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/set_nickname"))
            .and(body_json(
                json!({ "p_friend": B, "p_nickname": "Koca Bob" }),
            ))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/set_nickname"))
            .and(body_json(json!({ "p_friend": B, "p_nickname": null })))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        assert_eq!(
            c.create_group("  Yeni   Ekip ", &[B.into()]).await.unwrap(),
            42
        );
        c.set_nickname(B, Some("  Koca   Bob ")).await.unwrap();
        c.set_nickname(B, Some("   ")).await.unwrap();
    }

    #[tokio::test]
    async fn reads_and_posts_group_messages() {
        let server = MockServer::builder().start().await;
        Mock::given(method("GET"))
            .and(path("/rest/v1/group_messages"))
            .and(query_param("group_id", "eq.7"))
            .and(query_param("order", "id.desc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "id": 2, "group_id": 7, "sender": B, "body": "iki", "created_at": "t2" },
                { "id": 1, "group_id": 7, "sender": ME, "body": "bir", "created_at": "t1" }
            ])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/rest/v1/group_messages"))
            .and(query_param("id", "gt.2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/group_messages"))
            .and(body_json(
                json!({ "group_id": 7, "sender": ME, "body": "selam" }),
            ))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!([
                { "id": 3, "group_id": 7, "sender": ME, "body": "selam", "created_at": "t3" }
            ])))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/group_messages"))
            .and(body_json(
                json!({ "group_id": 9, "sender": ME, "body": "x" }),
            ))
            .respond_with(ResponseTemplate::new(403).set_body_json(json!({
                "code": "42501", "message": "new row violates row-level security policy"
            })))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let msgs = c.group_messages(7, None).await.unwrap();
        assert_eq!(msgs.iter().map(|m| m.id).collect::<Vec<_>>(), vec![1, 2]);
        assert!(c.group_messages(7, Some(2)).await.unwrap().is_empty());
        assert_eq!(c.send_group_message(7, "  selam ").await.unwrap().id, 3);
        assert_eq!(
            c.send_group_message(9, "x").await.unwrap_err().code(),
            "friends.groupNotFound"
        );
    }

    #[test]
    fn group_rows_parse_without_optional_fields() {
        let v: Value = json!({ "id": 1, "name": "n", "owner": null, "members": null,
                               "unread": 0, "last_message_at": null });
        let r: GroupRow = serde_json::from_value(v).unwrap();
        assert!(r.owner.is_none() && r.members.is_none());
    }
}
