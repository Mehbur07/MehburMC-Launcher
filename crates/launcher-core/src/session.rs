//! High-level launcher service used by the UI: instances, accounts, tasks
//! and the "play" flow (prepare → run → record play time).

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::auth::store::AccountStore;
use crate::content::install::{self, InstallRequest, InstallResult};
use crate::content::installed::{self, InstalledItem};
use crate::content::modpack::{self, ImportResult, PackKind};
use crate::content::{modrinth, shaders};
use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::events::EventSink;
use crate::events::{Progress, Stage};
use crate::instance::files::Folder;
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

        let mut opts = LaunchOptions {
            // Filled in once the loader is installed.
            version_id: String::new(),
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
    pub async fn install_shader_support(&self, instance_id: &str) -> Result<InstallResult> {
        let inst = self.instances.get(instance_id)?;
        let _claim = self.running.try_claim(&inst)?;
        let dir = self.instances.dir(&inst.id)?;
        shaders::install(&self.ctx, &inst, &dir, &CancellationToken::new()).await
    }

    /// Installs Modrinth projects (with dependencies) into an instance.
    pub async fn install_content(
        &self,
        instance_id: &str,
        requests: &[InstallRequest],
    ) -> Result<InstallResult> {
        let inst = self.instances.get(instance_id)?;
        let _claim = self.running.try_claim(&inst)?;
        let dir = self.instances.dir(&inst.id)?;
        install::install(
            &self.ctx,
            &inst,
            &dir,
            requests,
            &install::quiet_progress(),
            &CancellationToken::new(),
        )
        .await
    }

    /// Lists a content folder with Modrinth metadata (and updates).
    pub async fn scan_content(
        &self,
        instance_id: &str,
        folder: Folder,
        check_updates: bool,
    ) -> Result<Vec<InstalledItem>> {
        let inst = self.instances.get(instance_id)?;
        let dir = self.instances.dir(&inst.id)?;
        installed::scan(&self.ctx, &inst, &dir, folder, check_updates).await
    }

    pub async fn update_content(
        &self,
        instance_id: &str,
        folder: Folder,
        files: &[String],
    ) -> Result<Vec<String>> {
        let inst = self.instances.get(instance_id)?;
        let _claim = self.running.try_claim(&inst)?;
        let dir = self.instances.dir(&inst.id)?;
        installed::update(
            &self.ctx,
            &inst,
            &dir,
            folder,
            files,
            &CancellationToken::new(),
        )
        .await
    }

    /// Imports a `.mrpack` or CurseForge `.zip` as a new instance. Runs as a
    /// task (progress in Downloads, cancellable) but returns the result.
    pub async fn import_modpack(
        &self,
        pack: &std::path::Path,
        curseforge_key: Option<&str>,
    ) -> Result<ImportResult> {
        let kind = modpack::detect(pack)?;
        let title = pack
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "modpack".into());
        let (task, cancel) = self.tasks.create(TaskKind::Install, &title, None);
        let progress = Progress::new(self.ctx.events.clone(), &task, Stage::Content);
        let result = match kind {
            PackKind::Modrinth => {
                modpack::import_mrpack(&self.ctx, &self.instances, pack, &progress, &cancel).await
            }
            PackKind::CurseForge => {
                modpack::import_curseforge(
                    &self.ctx,
                    &self.instances,
                    pack,
                    curseforge_key,
                    &progress,
                    &cancel,
                )
                .await
            }
        };
        self.tasks.finish_ref(&task, result.as_ref().map(|_| ()));
        result
    }

    /// Downloads a Modrinth modpack version (`.mrpack`) and imports it.
    pub async fn install_modrinth_modpack(&self, version_id: &str) -> Result<ImportResult> {
        let version = modrinth::version(&self.ctx, version_id).await?;
        let file = version
            .files
            .iter()
            .find(|f| f.primary && f.filename.ends_with(".mrpack"))
            .or_else(|| {
                version
                    .files
                    .iter()
                    .find(|f| f.filename.ends_with(".mrpack"))
            })
            .ok_or_else(|| CoreError::ModpackInvalid("no .mrpack file".into()))?;
        let name = crate::instance::files::checked_name(&file.filename)?.to_owned();
        let dest = self.ctx.paths.modpacks().join(name);
        self.ctx
            .downloader()
            .run(
                vec![install::download_item(&dest, file)],
                Verify::Full,
                &install::quiet_progress(),
                &CancellationToken::new(),
            )
            .await?;
        self.import_modpack(&dest, None).await
    }

    /// Deletes an instance unless it is in use.
    pub fn delete_instance(&self, id: &str) -> Result<()> {
        let inst = self.instances.get(id)?;
        self.running.ensure_idle(&inst)?;
        self.instances.delete(id)
    }
}
