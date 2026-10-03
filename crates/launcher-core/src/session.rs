//! High-level launcher service used by the UI: instances, accounts, tasks
//! and the "play" flow (prepare → run → record play time).

use std::sync::Arc;

use crate::auth::store::AccountStore;
use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::events::EventSink;
use crate::instance::{Instance, InstanceStore, LoaderKind, Running, split_args};
use crate::launch::process::GameExit;
use crate::launch::{self, LaunchOptions, process};
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

pub struct Launcher {
    pub ctx: Ctx,
    pub instances: InstanceStore,
    pub accounts: AccountStore,
    pub tasks: Arc<TaskRegistry>,
    pub running: Running,
}

impl Launcher {
    pub fn new(paths: Paths, ui_sink: Arc<dyn EventSink>, concurrency: usize) -> Result<Arc<Self>> {
        let tasks = Arc::new(TaskRegistry::new(ui_sink.clone()));
        let sink = Arc::new(TrackingSink {
            tasks: tasks.clone(),
            inner: ui_sink,
        });
        Ok(Arc::new(Self {
            ctx: Ctx::new(paths.clone(), sink, concurrency)?,
            instances: InstanceStore::new(paths.clone()),
            accounts: AccountStore::new(paths),
            tasks,
            running: Running::default(),
        }))
    }

    fn version_id(inst: &Instance) -> Result<String> {
        match inst.loader.kind {
            LoaderKind::Vanilla => Ok(inst.mc_version.clone()),
            // Loader installation arrives in phase 4.
            other => Err(CoreError::InvalidInstance(format!(
                "{other:?} is not supported yet"
            ))),
        }
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
        let version_id = Self::version_id(&inst)?;
        let account = match mode {
            StartMode::Play => self.accounts.launch_account()?,
            // Repair never starts the game; any identity will do.
            StartMode::Repair => crate::auth::LaunchAccount::offline("Player")?,
        };
        let claim = self.running.try_claim(&inst)?;
        let kind = match mode {
            StartMode::Play => TaskKind::Launch,
            StartMode::Repair => TaskKind::Repair,
        };
        let (task_id, cancel) = self.tasks.create(kind, &inst.name, Some(&inst.id));

        let opts = LaunchOptions {
            version_id,
            game_dir: self.instances.dir(&inst.id)?,
            account,
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
                let prepared = launch::prepare(&this.ctx, &opts, &tid, &cancel).await?;
                if mode == StartMode::Repair {
                    return Ok(());
                }
                this.tasks.set_status(&tid, TaskStatus::Playing);
                hooks.game_started(&inst);
                let exit = process::run(&prepared, this.ctx.events.clone(), &tid, &cancel).await;
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

    /// Deletes an instance unless it is in use.
    pub fn delete_instance(&self, id: &str) -> Result<()> {
        let inst = self.instances.get(id)?;
        self.running.ensure_idle(&inst)?;
        self.instances.delete(id)
    }
}
