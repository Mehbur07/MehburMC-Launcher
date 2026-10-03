//! High-level launcher service used by the UI: instances, accounts, tasks
//! and the "play" flow (prepare → run → record play time).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::auth::LaunchAccount;
use crate::auth::microsoft::{self, DeviceCode};
use crate::auth::secrets::SecretStore;
use crate::auth::store::{Account, AccountStore};
use crate::content::shaders::{self, ShaderSetup};
use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::events::EventSink;
use crate::instance::{Instance, InstancePatch, InstanceStore, LoaderSpec, Running, split_args};
use crate::launch::process::GameExit;
use crate::launch::{self, LaunchOptions, process};
use crate::loader;
use crate::net::download::Verify;
use crate::paths::Paths;
use crate::settings::Settings;
use crate::tasks::{TaskKind, TaskRegistry, TaskStatus, TrackingSink};

/// Callbacks for UI side effects (window minimise/restore).
pub trait LaunchHooks: Send + Sync {
    fn game_started(&self, _instance: &Instance) {}
    fn game_exited(&self, _instance: &Instance, _exit: &GameExit) {}
}

pub struct NoHooks;
impl LaunchHooks for NoHooks {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartMode {
    Play,
    Repair,
}

/// What the UI shows while the user signs in in the browser.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LoginPrompt {
    pub login_id: String,
    pub user_code: String,
    pub verification_uri: String,
    #[ts(type = "number")]
    pub expires_in: u64,
}

struct PendingLogin {
    code: DeviceCode,
    client_id: String,
    cancel: CancellationToken,
}

pub struct Launcher {
    pub ctx: Ctx,
    pub instances: InstanceStore,
    pub accounts: AccountStore,
    pub tasks: Arc<TaskRegistry>,
    pub running: Running,
    logins: Mutex<HashMap<String, PendingLogin>>,
    login_counter: AtomicU64,
}

impl Launcher {
    pub fn new(
        paths: Paths,
        ui_sink: Arc<dyn EventSink>,
        concurrency: usize,
        secrets: Arc<dyn SecretStore>,
    ) -> Result<Arc<Self>> {
        let tasks = Arc::new(TaskRegistry::new(ui_sink.clone()));
        let sink = Arc::new(TrackingSink {
            tasks: tasks.clone(),
            inner: ui_sink,
        });
        Ok(Arc::new(Self {
            ctx: Ctx::new(paths.clone(), sink, concurrency)?,
            instances: InstanceStore::new(paths.clone()),
            accounts: AccountStore::new(paths, secrets),
            tasks,
            running: Running::default(),
            logins: Mutex::new(HashMap::new()),
            login_counter: AtomicU64::new(1),
        }))
    }

    /// Step 1 of Microsoft sign-in: get a device code to show the user.
    pub async fn begin_microsoft_login(&self, client_id: Option<String>) -> Result<LoginPrompt> {
        let client_id = client_id.ok_or(CoreError::AuthNotConfigured)?;
        let code = microsoft::request_device_code(&self.ctx, &client_id).await?;
        let login_id = format!(
            "login-{}",
            self.login_counter.fetch_add(1, Ordering::Relaxed)
        );
        let prompt = LoginPrompt {
            login_id: login_id.clone(),
            user_code: code.user_code.clone(),
            verification_uri: code.verification_uri.clone(),
            expires_in: code.expires_in,
        };
        self.logins.lock().expect("logins").insert(
            login_id,
            PendingLogin {
                code,
                client_id,
                cancel: CancellationToken::new(),
            },
        );
        Ok(prompt)
    }

    /// Step 2: waits until the user completed the browser step, then
    /// signs in to Minecraft and stores the account.
    pub async fn finish_microsoft_login(&self, login_id: &str) -> Result<Account> {
        let (code, client_id, cancel) = {
            let logins = self.logins.lock().expect("logins");
            let p = logins.get(login_id).ok_or(CoreError::AuthCodeExpired)?;
            (p.code.clone(), p.client_id.clone(), p.cancel.clone())
        };
        let result = async {
            let tokens = microsoft::poll_device_code(&self.ctx, &client_id, &code, &cancel).await?;
            let session = microsoft::minecraft_login(&self.ctx, &tokens.access_token).await?;
            self.accounts.add_microsoft(session, &tokens.refresh_token)
        }
        .await;
        self.logins.lock().expect("logins").remove(login_id);
        result
    }

    pub fn cancel_microsoft_login(&self, login_id: &str) {
        if let Some(p) = self.logins.lock().expect("logins").remove(login_id) {
            p.cancel.cancel();
        }
    }

    /// Verification page of a pending sign-in.
    pub fn login_page(&self, login_id: &str) -> Option<String> {
        self.logins
            .lock()
            .expect("logins")
            .get(login_id)
            .map(|p| p.code.verification_uri.clone())
    }

    /// Validates and starts a task in the background; returns its id.
    /// Errors that the user must fix (no account, busy) are returned directly.
    pub fn start(
        self: &Arc<Self>,
        instance_id: &str,
        settings: Settings,
        mode: StartMode,
        hooks: Arc<dyn LaunchHooks>,
    ) -> Result<String> {
        let inst = self.instances.get(instance_id)?;
        let account = match mode {
            StartMode::Play => {
                let acc = self.accounts.selected()?;
                if acc.needs_login {
                    return Err(CoreError::AuthRelogin { name: acc.name });
                }
                Some(acc)
            }
            // Repair never starts the game; any identity will do.
            StartMode::Repair => None,
        };
        let client_id = settings.msa_client_id();
        let claim = self.running.try_claim(&inst)?;
        let kind = match mode {
            StartMode::Play => TaskKind::Launch,
            StartMode::Repair => TaskKind::Repair,
        };
        let (task_id, cancel) = self.tasks.create(kind, &inst.name, Some(&inst.id));

        let mut opts = LaunchOptions {
            // Filled in once the loader is installed.
            version_id: String::new(),
            game_dir: self.instances.dir(&inst.id)?,
            // Resolved in the task (Microsoft accounts may need a refresh).
            account: LaunchAccount::offline("Player")?,
            max_memory_mb: inst.memory_mb.unwrap_or(settings.default_memory_mb),
            min_memory_mb: None,
            java_path: inst.java_path.clone().map(Into::into),
            auto_download_java: true,
            extra_jvm_args: split_args(&inst.jvm_args),
            extra_game_args: if inst.fullscreen {
                vec!["--fullscreen".into()]
            } else {
                vec![]
            },
            resolution: inst.resolution.map(|r| (r.width, r.height)),
            verify: match mode {
                StartMode::Play => Verify::Quick,
                StartMode::Repair => Verify::Full,
            },
        };

        let this = self.clone();
        let tid = task_id.clone();
        tokio::spawn(async move {
            let _claim = claim;
            let result: Result<()> = async {
                if let Some(acc) = &account {
                    opts.account = this
                        .accounts
                        .launch_account(&this.ctx, acc, client_id.as_deref())
                        .await?;
                }
                let installed = loader::ensure_installed(
                    &this.ctx,
                    &inst.mc_version,
                    &inst.loader,
                    mode == StartMode::Repair,
                    &tid,
                    &cancel,
                )
                .await?;
                // Pin the automatically chosen loader version so the
                // instance does not silently change on the next update.
                if inst.loader.version.is_none() && installed.loader_version.is_some() {
                    let patch = InstancePatch {
                        loader: Some(LoaderSpec {
                            kind: inst.loader.kind,
                            version: installed.loader_version.clone(),
                        }),
                        ..Default::default()
                    };
                    if let Err(e) = this.instances.update(&inst.id, patch) {
                        tracing::warn!(error = %e.detail(), "could not pin loader version");
                    }
                }
                opts.version_id = installed.version_id;
                let prepared = launch::prepare(&this.ctx, &opts, &tid, &cancel).await?;
                if mode == StartMode::Repair {
                    return Ok(());
                }
                this.tasks.set_status(&tid, TaskStatus::Playing);
                hooks.game_started(&inst);
                let mut exit =
                    process::run(&prepared, this.ctx.events.clone(), &tid, &cancel).await;
                if let Ok(e) = &exit
                    && loader::early_window::looks_like_early_window_crash(inst.loader.kind, e)
                    && loader::early_window::disable(&opts.game_dir).unwrap_or(false)
                {
                    tracing::warn!(
                        code = ?e.code,
                        "native crash in the early loading window; retrying with earlyWindowControl=false"
                    );
                    // Argfiles were removed with the first run: prepare again (fast).
                    let again = launch::prepare(&this.ctx, &opts, &tid, &cancel).await?;
                    this.tasks.set_status(&tid, TaskStatus::Playing);
                    exit = process::run(&again, this.ctx.events.clone(), &tid, &cancel).await;
                }
                if let Ok(e) = &exit {
                    let secs = e.duration.as_secs();
                    if let Err(err) = this.instances.record_session(&inst.id, secs) {
                        tracing::warn!(error = %err.detail(), "could not record play time");
                    }
                    hooks.game_exited(&inst, e);
                }
                // A user-requested stop of a running game is a normal end.
                exit.map(|_| ())
            }
            .await;
            if let Err(e) = &result {
                tracing::warn!(task = %tid, error = %e.detail(), "task ended with error");
            }
            this.tasks.finish(&tid, &result);
        });
        Ok(task_id)
    }

    /// Stops whatever runs for the instance (download or game).
    pub fn stop(&self, instance_id: &str) -> bool {
        self.tasks
            .active_for_instance(instance_id)
            .is_some_and(|t| self.tasks.cancel(&t, false))
    }

    /// Downloads Iris + Sodium (or Oculus + Embeddium) into the instance's
    /// `mods/` folder. The instance is reserved meanwhile so it cannot start.
    pub async fn install_shader_support(&self, instance_id: &str) -> Result<ShaderSetup> {
        let inst = self.instances.get(instance_id)?;
        let _claim = self.running.try_claim(&inst)?;
        let mods = self.instances.dir(&inst.id)?.join("mods");
        std::fs::create_dir_all(&mods).map_err(|e| crate::CoreError::io(&mods, e))?;
        shaders::install(&self.ctx, &inst, &mods, &CancellationToken::new()).await
    }

    /// Deletes an instance unless it is in use.
    pub fn delete_instance(&self, id: &str) -> Result<()> {
        let inst = self.instances.get(id)?;
        self.running.ensure_idle(&inst)?;
        self.instances.delete(id)
    }
}
