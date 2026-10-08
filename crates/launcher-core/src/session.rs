//! High-level launcher service used by the UI: instances, accounts, tasks
//! and the "play" flow (prepare → run → record play time).

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::auth::avatar::{self, AvatarStore};
use crate::auth::store::AccountStore;
use crate::content::install::{self, InstallRequest, InstallResult};
use crate::content::installed::{self, InstalledItem};
use crate::content::modpack::{self, ImportResult, PackKind};
use crate::content::{modrinth, shaders};
use crate::crash;
use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::events::{CoreEvent, EventSink};
use crate::events::{Progress, Stage};
use crate::friends::FriendsClient;
use crate::friends::share::{FriendInstallResult, SharedList};
use crate::instance::files::Folder;
use crate::instance::{Instance, InstancePatch, InstanceStore, LoaderSpec, Running, split_args};
use crate::launch::process::GameExit;
use crate::launch::{self, LaunchOptions, process};
use crate::loader;
use crate::net::download::Verify;
use crate::paths::Paths;
use crate::servers::{ServerAddress, ServerStore};
use crate::settings::{LaunchBehavior, Settings};
use crate::skin::{self, SkinStore};
use crate::tasks::{TaskKind, TaskRegistry, TaskStatus, TrackingSink};

/// Callbacks for UI side effects (window minimise/restore).
pub trait LaunchHooks: Send + Sync {
    fn game_started(&self, _instance: &Instance) {}
    /// The client reached the main menu.
    fn game_ready(&self, _instance: &Instance) {}
    fn game_exited(&self, _instance: &Instance, _exit: &GameExit) {}
}

pub struct NoHooks;
impl LaunchHooks for NoHooks {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartMode {
    Play,
    Repair,
}

/// Folders the running game reads (mods, resource and shader packs).
fn is_content(folder: Folder) -> bool {
    matches!(
        folder,
        Folder::Mods | Folder::ResourcePacks | Folder::ShaderPacks
    )
}

/// Faces of the launcher accounts for the UI (`account id → data: URI`).
#[derive(Debug, Clone, Default, serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AccountFaces {
    /// Chosen profile photos.
    pub photos: std::collections::HashMap<String, String>,
    /// Game default skin of accounts that have no skin assigned.
    pub default_skins: std::collections::HashMap<String, String>,
    /// Accounts whose name another user holds (rename them).
    pub name_conflicts: Vec<String>,
}

pub struct Launcher {
    pub ctx: Ctx,
    pub instances: InstanceStore,
    pub accounts: AccountStore,
    pub skins: SkinStore,
    pub avatars: AvatarStore,
    pub friends: FriendsClient,
    pub servers: ServerStore,
    pub tasks: Arc<TaskRegistry>,
    pub running: Running,
    /// Accounts whose name another MehburMC user reserved first (K67).
    name_conflicts: std::sync::Mutex<std::collections::HashSet<String>>,
}

impl Launcher {
    pub fn new(paths: Paths, ui_sink: Arc<dyn EventSink>, concurrency: usize) -> Result<Arc<Self>> {
        let tasks = Arc::new(TaskRegistry::new(ui_sink.clone()));
        let sink = Arc::new(TrackingSink {
            tasks: tasks.clone(),
            inner: ui_sink,
        });
        let ctx = Ctx::new(paths.clone(), sink, concurrency)?;
        Ok(Arc::new(Self {
            friends: FriendsClient::new(ctx.clone()),
            ctx,
            instances: InstanceStore::new(paths.clone()),
            accounts: AccountStore::new(paths.clone()),
            avatars: AvatarStore::new(paths.clone()),
            servers: ServerStore::new(paths.clone()),
            skins: SkinStore::new(paths),
            tasks,
            running: Running::default(),
            name_conflicts: Default::default(),
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
        self.start_with(instance_id, settings, mode, hooks, None)
    }

    /// [`Self::start`] that also joins `server` once the game is up.
    pub fn start_with(
        self: &Arc<Self>,
        instance_id: &str,
        settings: Settings,
        mode: StartMode,
        hooks: Arc<dyn LaunchHooks>,
        join_server: Option<ServerAddress>,
    ) -> Result<String> {
        let inst = self.instances.get(instance_id)?;
        let account_id = self.accounts.view().selected;
        let account = match mode {
            StartMode::Play => self.accounts.launch_account()?,
            // Repair never starts the game; any identity will do.
            StartMode::Repair => crate::auth::LaunchAccount::offline("Player")?,
        };
        let claim = match mode {
            StartMode::Play => self
                .running
                .try_claim_play(&inst, &account.uuid, &account.name)?,
            StartMode::Repair => self.running.try_claim(&inst)?,
        };
        let kind = match mode {
            StartMode::Play => TaskKind::Launch,
            StartMode::Repair => TaskKind::Repair,
        };
        let (task_id, cancel) = self.tasks.create(kind, &inst.name, Some(&inst.id));
        if mode == StartMode::Play {
            self.tasks.set_account(&task_id, &account.name);
        }

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
            join_server,
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
                if let Some(id) = &account_id {
                    this.sync_skin(&opts.game_dir, id, &opts.account.name);
                }
                this.tasks.set_status(&tid, TaskStatus::Playing);
                hooks.game_started(&inst);
                // "Close on launch": the launcher quits while the game runs,
                // so the game must not write into launcher-owned pipes.
                let output_file = (settings.launch_behavior == LaunchBehavior::Close)
                    .then(|| opts.game_dir.join("logs").join("launcher-output.log"));
                let ready_hooks = || {
                    let (h, i) = (hooks.clone(), inst.clone());
                    process::RunHooks {
                        on_ready: Some(Box::new(move || h.game_ready(&i))),
                        output_file: output_file.clone(),
                    }
                };
                let mut exit = process::run_with(
                    &prepared,
                    this.ctx.events.clone(),
                    &tid,
                    &cancel,
                    ready_hooks(),
                )
                .await;
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
                    exit = process::run_with(
                        &again,
                        this.ctx.events.clone(),
                        &tid,
                        &cancel,
                        ready_hooks(),
                    )
                    .await;
                }
                if let Ok(e) = &exit {
                    let secs = e.duration.as_secs();
                    if let Err(err) = this.instances.record_session(&inst.id, secs) {
                        tracing::warn!(error = %err.detail(), "could not record play time");
                    }
                    if !e.killed && e.code != Some(0) {
                        let info = crash::analyze(
                            &inst.id,
                            &inst.name,
                            crash::Inputs {
                                game_dir: &opts.game_dir,
                                exit_code: e.code,
                                crash_report: e.crash_report.as_deref(),
                                output: &e.output_tail,
                                started: e.started_at,
                            },
                        );
                        tracing::info!(diagnoses = ?info.diagnoses, "crash analysed");
                        this.ctx.events.emit(CoreEvent::GameCrashed {
                            task: tid.clone(),
                            info,
                        });
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

    /// Hands the account's offline skin to CustomSkinLoader, if installed.
    /// Never blocks the launch: failures are only logged.
    fn sync_skin(&self, game_dir: &std::path::Path, account_id: &str, player: &str) {
        if !skin::csl::is_installed(game_dir) {
            return;
        }
        let textures = self.skins.textures_for(account_id);
        match skin::csl::sync(game_dir, player, &textures) {
            Ok(files) => tracing::info!(
                count = files.len(),
                "offline skin synced for CustomSkinLoader"
            ),
            Err(e) => tracing::warn!(error = %e.detail(), "could not sync offline skin"),
        }
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

    /// Creates an account after reserving its name for all MehburMC users.
    pub async fn create_account(&self, name: &str) -> Result<crate::auth::store::Account> {
        let name = name.trim();
        crate::auth::offline::validate_name(name)?;
        let v = self.accounts.view();
        if v.accounts.iter().any(|a| a.name.eq_ignore_ascii_case(name)) {
            return Err(CoreError::AccountNameTaken(name.to_owned()));
        }
        self.friends.claim_name(name).await?;
        match self.accounts.add_offline(name) {
            Ok(acc) => Ok(acc),
            Err(e) => {
                self.friends.release_name(name).await;
                Err(e)
            }
        }
    }

    /// Renames an account; the reservation moves to the new name first.
    pub async fn rename_account(
        &self,
        id: &str,
        name: &str,
    ) -> Result<crate::auth::store::Account> {
        let name = name.trim();
        crate::auth::offline::validate_name(name)?;
        let v = self.accounts.view();
        let old = v
            .accounts
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.name.clone())
            .ok_or_else(|| CoreError::AccountNotFound(id.to_owned()))?;
        if v.accounts
            .iter()
            .any(|a| a.id != id && a.name.eq_ignore_ascii_case(name))
        {
            return Err(CoreError::AccountNameTaken(name.to_owned()));
        }
        if old != name {
            self.friends.rename_name(&old, name).await?;
        }
        match self.accounts.rename(id, name) {
            Ok(acc) => {
                self.name_conflicts
                    .lock()
                    .expect("conflicts lock")
                    .remove(id);
                Ok(acc)
            }
            Err(e) => {
                if old != name
                    && let Err(back) = self.friends.rename_name(name, &old).await
                {
                    tracing::warn!(error = %back.detail(), "could not restore the old name");
                }
                Err(e)
            }
        }
    }

    /// Removes an account and lets its name go.
    pub async fn remove_account(&self, id: &str) -> Result<crate::auth::store::AccountsView> {
        let name = self
            .accounts
            .view()
            .accounts
            .into_iter()
            .find(|a| a.id == id)
            .map(|a| a.name);
        let view = self.accounts.remove(id)?;
        if let Err(e) = self.skins.forget_account(id) {
            tracing::warn!(error = %e.detail(), "could not clear skin assignment");
        }
        let conflicted = self
            .name_conflicts
            .lock()
            .expect("conflicts lock")
            .remove(id);
        // Someone else's name was never ours to release.
        if let Some(name) = name
            && !conflicted
        {
            self.friends.release_name(&name).await;
        }
        Ok(view)
    }

    /// Startup: reserves the names of existing accounts and remembers the
    /// ones another user already has. Offline is not an error (next start).
    pub async fn sync_account_names(&self) {
        let accounts = self.accounts.view().accounts;
        let names: Vec<String> = accounts.iter().map(|a| a.name.clone()).collect();
        match self.friends.sync_names(&names).await {
            Ok(results) => {
                let conflicts = accounts
                    .iter()
                    .filter(|a| {
                        results
                            .get(&a.name)
                            .is_some_and(|r| *r != crate::friends::names::ClaimResult::Ok)
                    })
                    .map(|a| a.id.clone())
                    .collect();
                *self.name_conflicts.lock().expect("conflicts lock") = conflicts;
            }
            Err(e) => tracing::debug!(error = %e.detail(), "account name sync skipped"),
        }
    }

    /// Skin texture shown for an account: its own, else the game default
    /// for its UUID (from an installed client jar, if any).
    fn account_skin_png(
        &self,
        acc: &crate::auth::store::Account,
        defaults: &[skin::defaults::DefaultSkin],
    ) -> Option<Vec<u8>> {
        if let Some((png, _)) = self.skins.textures_for(&acc.id).skin {
            return Some(png);
        }
        let uri = &skin::defaults::for_uuid(defaults, &acc.uuid)?.data_uri;
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(uri.split_once(',')?.1)
            .ok()
    }

    /// Photos of all accounts, plus the default skin of accounts without a
    /// skin of their own (so the UI can draw its head).
    pub fn account_faces(&self) -> AccountFaces {
        let v = self.accounts.view();
        let defaults = skin::defaults::list(&self.ctx.paths);
        let default_skins = v
            .accounts
            .iter()
            .filter(|a| self.skins.textures_for(&a.id).skin.is_none())
            .filter_map(|a| {
                let d = skin::defaults::for_uuid(&defaults, &a.uuid)?;
                Some((a.id.clone(), d.data_uri.clone()))
            })
            .collect();
        let mut name_conflicts: Vec<String> = self
            .name_conflicts
            .lock()
            .expect("conflicts lock")
            .iter()
            .filter(|id| v.accounts.iter().any(|a| a.id == **id))
            .cloned()
            .collect();
        name_conflicts.sort();
        AccountFaces {
            photos: self
                .avatars
                .data_uris(v.accounts.iter().map(|a| a.id.as_str())),
            default_skins,
            name_conflicts,
        }
    }

    /// What friends see: the selected account's name (or "Player") and its
    /// photo, or the head of its skin, as a small PNG.
    pub fn friend_identity(&self) -> (String, Option<Vec<u8>>) {
        let v = self.accounts.view();
        let Some(acc) = v
            .selected
            .and_then(|id| v.accounts.into_iter().find(|a| a.id == id))
        else {
            return ("Player".into(), None);
        };
        let size = crate::friends::avatar::PUBLIC_SIZE;
        let image = match self.avatars.bytes(&acc.id) {
            Some(png) => skin::image::decode_limited(&png, avatar::MAX_FILE_BYTES, 4096)
                .ok()
                .map(|img| avatar::square_downscale(&img, size)),
            None => self
                .account_skin_png(&acc, &skin::defaults::list(&self.ctx.paths))
                .and_then(|png| skin::image::decode(&png).ok())
                .map(|img| skin::image::head(&img, size)),
        };
        (
            acc.name,
            image.and_then(|img| skin::image::encode(&img).ok()),
        )
    }

    /// Re-sends name and photo to the friends service, if friends are on.
    /// Errors are logged: this runs after unrelated changes (photo, skin,
    /// account selection) that must not fail because of the network.
    pub async fn refresh_friend_profile(&self) {
        if !self.friends.is_enabled().await {
            return;
        }
        let (name, png) = self.friend_identity();
        if let Err(e) = self.friends.sync_profile(&name, png.as_deref()).await {
            tracing::debug!(error = %e.detail(), "friend profile refresh failed");
        }
    }

    /// Shares an instance's mods with friends.
    pub async fn share_instance(&self, instance_id: &str) -> Result<SharedList> {
        let inst = self.instances.get(instance_id)?;
        let dir = self.instances.dir(&inst.id)?;
        self.friends.share_instance(&inst, &dir).await
    }

    /// Installs mods from a friend's shared list; the instance is reserved
    /// meanwhile so it cannot start.
    pub async fn install_friend_mods(
        &self,
        list_id: i64,
        instance_id: &str,
        files: &[String],
    ) -> Result<FriendInstallResult> {
        let inst = self.instances.get(instance_id)?;
        let _claim = self.running.try_claim(&inst)?;
        let dir = self.instances.dir(&inst.id)?;
        self.friends.install_from_list(list_id, &dir, files).await
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

    /// Shares a library skin/cape under the selected account's name (its
    /// name is reserved for this installation, K67). Returns the share id.
    pub async fn share_texture(
        &self,
        kind: skin::TextureKind,
        id: &str,
        name: &str,
        visibility: crate::friends::textures::Visibility,
    ) -> Result<i64> {
        let author = self.accounts.launch_account()?.name;
        let view = self.skins.view();
        let model = match kind {
            skin::TextureKind::Skin => view
                .skins
                .iter()
                .find(|s| s.entry.id == id)
                .map(|s| s.entry.model)
                .ok_or_else(|| CoreError::SkinNotFound(id.to_owned()))?,
            skin::TextureKind::Cape => {
                if !view.capes.iter().any(|c| c.entry.id == id) {
                    return Err(CoreError::SkinNotFound(id.to_owned()));
                }
                skin::SkinModel::Classic
            }
        };
        let path = self.skins.texture_path(id)?;
        let png = std::fs::read(&path).map_err(|e| CoreError::io(&path, e))?;
        self.friends
            .share_texture(kind, model, name, &author, &png, visibility)
            .await
    }

    /// Brings privately granted MehburMC textures in line with the server
    /// (K74): new grants are downloaded into the skin library, revoked ones
    /// removed (with their assignments). Offline or signed out nothing
    /// changes. Returns whether the library changed.
    pub async fn sync_private_textures(&self) -> Result<bool> {
        if !self.friends.can_play().await {
            return Ok(false);
        }
        let grants = self.friends.my_private_textures().await?;
        let local = self.skins.private_ids();
        let revoked: Vec<String> = local
            .iter()
            .filter(|id| !grants.iter().any(|g| &g.sha1 == *id))
            .cloned()
            .collect();
        let mut changed = !self.skins.remove_ids(&revoked)?.is_empty();
        for g in grants {
            // Deleting one by hand removes its file; the grant brings it back.
            if local.contains(&g.sha1) && self.skins.texture_path(&g.sha1)?.is_file() {
                continue;
            }
            match self.friends.private_texture_png(&g.sha1, g.kind).await {
                Ok(png) => {
                    self.skins.add_private(g.kind, &png, &g.name, g.model)?;
                    changed = true;
                }
                Err(e) => {
                    tracing::warn!(error = %e.detail(), sha1 = %g.sha1, "private texture not available")
                }
            }
        }
        Ok(changed)
    }

    /// Submits a mod jar to the MehburMC Library under the selected
    /// account's name. `expected_sha1` is the file the user saw the scan
    /// report for; a file changed since then is refused.
    pub async fn submit_library_mod(
        &self,
        path: &std::path::Path,
        expected_sha1: &str,
        name: &str,
        description: &str,
    ) -> Result<(i64, crate::content::scan::ScanReport)> {
        let author = self.accounts.launch_account()?.name;
        let bytes = crate::content::scan::read_jar(path)?;
        if crate::friends::avatar::sha1_hex(&bytes) != expected_sha1 {
            return Err(CoreError::Friends("library.changed"));
        }
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.friends
            .submit_library_mod(&bytes, &file_name, name, description, &author)
            .await
    }

    /// Installs a MehburMC Library mod into an instance's `mods/` folder;
    /// the instance is reserved meanwhile so it cannot start.
    pub async fn install_library_mod(
        &self,
        library_id: i64,
        instance_id: &str,
        accept_warnings: bool,
    ) -> Result<crate::friends::library::LibraryInstall> {
        let inst = self.instances.get(instance_id)?;
        let _claim = self.running.try_claim(&inst)?;
        let dir = self.instances.dir(&inst.id)?;
        self.friends
            .install_library_mod(
                library_id,
                &dir.join(Folder::Mods.dir_name()),
                inst.loader.kind,
                accept_warnings,
            )
            .await
    }

    /// Switches a file on/off (`.disabled`). Content is in use while the
    /// game runs, so that is refused then.
    pub fn toggle_instance_file(
        &self,
        id: &str,
        folder: Folder,
        name: &str,
    ) -> Result<crate::instance::files::FileEntry> {
        let inst = self.instances.get(id)?;
        if is_content(folder) {
            self.running.ensure_idle(&inst)?;
        }
        crate::instance::files::toggle(&self.instances, &inst.id, folder, name)
    }

    /// Turns the given files of a content folder on or off; files already
    /// in that state are left alone. Returns how many were switched.
    pub fn set_content_enabled(
        &self,
        id: &str,
        folder: Folder,
        names: &[String],
        enabled: bool,
    ) -> Result<u32> {
        if !is_content(folder) {
            return Err(CoreError::InvalidInstance(format!(
                "{folder:?} has no managed content"
            )));
        }
        let inst = self.instances.get(id)?;
        self.running.ensure_idle(&inst)?;
        let mut switched = 0;
        for name in names {
            let disabled = name.ends_with(crate::instance::files::DISABLED_SUFFIX);
            if disabled == enabled {
                crate::instance::files::toggle(&self.instances, &inst.id, folder, name)?;
                switched += 1;
            }
        }
        Ok(switched)
    }

    /// Deletes a file from an instance folder. Content (mods, resource and
    /// shader packs) is in use while the game runs, so it is refused then.
    pub fn delete_instance_file(&self, id: &str, folder: Folder, name: &str) -> Result<()> {
        let inst = self.instances.get(id)?;
        if is_content(folder) {
            self.running.ensure_idle(&inst)?;
        }
        crate::instance::files::delete(&self.instances, &inst.id, folder, name)
    }

    /// Deletes an instance unless it is in use.
    pub fn delete_instance(&self, id: &str) -> Result<()> {
        let inst = self.instances.get(id)?;
        self.running.ensure_idle(&inst)?;
        self.instances.delete(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::NullSink;
    use crate::instance::tests::new;

    fn launcher() -> (tempfile::TempDir, Arc<Launcher>) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::at(tmp.path().join("MehburMC"));
        paths.ensure_layout().unwrap();
        (tmp, Launcher::new(paths, Arc::new(NullSink), 2).unwrap())
    }

    #[tokio::test]
    async fn start_reports_user_fixable_errors_directly() {
        let (_tmp, l) = launcher();
        let inst = l.instances.create(new("A")).unwrap();
        let play = |id: &str| l.start(id, Settings::default(), StartMode::Play, Arc::new(NoHooks));

        assert_eq!(play(&inst.id).unwrap_err().code(), "account.none");
        l.accounts.add_offline("Steve").unwrap();
        assert_eq!(play("missing").unwrap_err().code(), "instance.notFound");

        // An instance that is already claimed (running or preparing) is busy.
        let _claim = l.running.try_claim(&inst).unwrap();
        assert_eq!(play(&inst.id).unwrap_err().code(), "instance.busy");
        assert_eq!(
            l.delete_instance(&inst.id).unwrap_err().code(),
            "instance.busy"
        );
        assert!(
            l.tasks.list().is_empty(),
            "no task is created for rejected starts"
        );
    }

    #[tokio::test]
    async fn sharing_needs_an_account_and_a_library_texture() {
        use crate::friends::textures::Visibility;
        let (_tmp, l) = launcher();
        let share = |id: &'static str| {
            let l = l.clone();
            async move {
                l.share_texture(skin::TextureKind::Skin, id, "n", Visibility::Public)
                    .await
                    .unwrap_err()
                    .code()
            }
        };
        assert_eq!(
            share("0000000000000000000000000000000000000000").await,
            "account.none"
        );
        l.accounts.add_offline("Steve").unwrap();
        assert_eq!(
            share("0000000000000000000000000000000000000000").await,
            "skin.notFound"
        );
    }

    #[test]
    fn bulk_toggle_switches_only_what_differs_and_respects_running() {
        let (_tmp, l) = launcher();
        let inst = l.instances.create(new("A")).unwrap();
        let mods = l.instances.dir(&inst.id).unwrap().join("mods");
        std::fs::write(mods.join("a.jar"), b"a").unwrap();
        std::fs::write(mods.join("b.jar.disabled"), b"b").unwrap();
        let names = vec!["a.jar".to_owned(), "b.jar.disabled".to_owned()];

        assert_eq!(
            l.set_content_enabled(&inst.id, Folder::Mods, &names, false)
                .unwrap(),
            1
        );
        assert!(mods.join("a.jar.disabled").exists() && mods.join("b.jar.disabled").exists());

        let names = vec!["a.jar.disabled".to_owned(), "b.jar.disabled".to_owned()];
        let claim = l.running.try_claim(&inst).unwrap();
        assert_eq!(
            l.set_content_enabled(&inst.id, Folder::Mods, &names, true)
                .unwrap_err()
                .code(),
            "instance.busy"
        );
        assert_eq!(
            l.toggle_instance_file(&inst.id, Folder::Mods, "a.jar.disabled")
                .unwrap_err()
                .code(),
            "instance.busy"
        );
        drop(claim);
        assert_eq!(
            l.set_content_enabled(&inst.id, Folder::Mods, &names, true)
                .unwrap(),
            2
        );
        assert!(mods.join("a.jar").exists() && mods.join("b.jar").exists());
        assert!(
            l.set_content_enabled(&inst.id, Folder::Saves, &names, true)
                .is_err()
        );
    }

    #[test]
    fn toggling_never_overwrites_the_twin_file() {
        let (_tmp, l) = launcher();
        let inst = l.instances.create(new("A")).unwrap();
        let mods = l.instances.dir(&inst.id).unwrap().join("mods");
        std::fs::write(mods.join("a.jar"), b"new").unwrap();
        std::fs::write(mods.join("a.jar.disabled"), b"old").unwrap();
        assert!(
            l.toggle_instance_file(&inst.id, Folder::Mods, "a.jar")
                .is_err()
        );
        assert_eq!(std::fs::read(mods.join("a.jar.disabled")).unwrap(), b"old");
        assert_eq!(std::fs::read(mods.join("a.jar")).unwrap(), b"new");
    }

    #[test]
    fn content_is_not_deleted_while_the_game_runs() {
        let (_tmp, l) = launcher();
        let inst = l.instances.create(new("A")).unwrap();
        let dir = l.instances.dir(&inst.id).unwrap();
        std::fs::write(dir.join("mods/a.jar"), b"").unwrap();
        std::fs::write(dir.join("screenshots/s.png"), b"").unwrap();

        let claim = l.running.try_claim(&inst).unwrap();
        assert_eq!(
            l.delete_instance_file(&inst.id, Folder::Mods, "a.jar")
                .unwrap_err()
                .code(),
            "instance.busy"
        );
        // Screenshots are not in use by the game.
        l.delete_instance_file(&inst.id, Folder::Screenshots, "s.png")
            .unwrap();
        drop(claim);

        l.delete_instance_file(&inst.id, Folder::Mods, "a.jar")
            .unwrap();
        assert!(!dir.join("mods/a.jar").exists());
    }

    #[test]
    fn skin_sync_only_touches_instances_with_customskinloader() {
        let (tmp, l) = launcher();
        let acc = l.accounts.add_offline("Steve").unwrap();
        let png = crate::skin::image::tests::png(64, 64, |_, _| false);
        let skin = l.skins.add_skin(&png, "s", None).unwrap();
        l.skins
            .assign(&acc.id, crate::skin::TextureKind::Skin, Some(&skin.id))
            .unwrap();

        let game = tmp.path().join("game");
        std::fs::create_dir_all(game.join("mods")).unwrap();
        l.sync_skin(&game, &acc.id, "Steve");
        assert!(!game.join("CustomSkinLoader").exists());

        std::fs::write(game.join("mods/CustomSkinLoader_Fabric-14.28.jar"), b"").unwrap();
        l.sync_skin(&game, &acc.id, "Steve");
        assert_eq!(
            std::fs::read(game.join("CustomSkinLoader/MehburMC/classic/Steve.png")).unwrap(),
            png
        );
    }
}
