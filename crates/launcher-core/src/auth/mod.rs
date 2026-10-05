//! Accounts as seen by the launch pipeline. Only offline accounts exist;
//! Microsoft sign-in was removed on request (see ARCHITECTURE.md K35).

pub mod avatar;
pub mod offline;
pub mod store;

/// Identity handed to the game (`${auth_*}` placeholders).
#[derive(Debug, Clone)]
pub struct LaunchAccount {
    pub name: String,
    /// Hyphenated UUID.
    pub uuid: String,
    pub access_token: String,
    /// Always `legacy` (offline).
    pub user_type: String,
    pub xuid: String,
    pub client_id: String,
    pub offline: bool,
}

impl LaunchAccount {
    /// UUID without hyphens, the form used on the game command line.
    pub fn uuid_simple(&self) -> String {
        self.uuid.replace('-', "")
    }

    /// `${auth_session}` for pre-1.6 versions.
    pub fn session(&self) -> String {
        if self.offline {
            "-".into()
        } else {
            format!("token:{}:{}", self.access_token, self.uuid_simple())
        }
    }
}
