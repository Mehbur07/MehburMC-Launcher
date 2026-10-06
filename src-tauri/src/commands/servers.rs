//! Servers page: launcher favourites, the game's `servers.dat` per instance
//! and status pings.

use launcher_core::servers::ping::{self, ServerStatus};
use launcher_core::servers::{FavoriteServer, GameServer, ServerAddress, dat};
use tauri::State;

use super::{CmdResult, blocking};
use crate::state::AppState;

#[tauri::command]
pub fn list_favorite_servers(state: State<'_, AppState>) -> CmdResult<Vec<FavoriteServer>> {
    Ok(state.launcher()?.servers.favorites())
}

#[tauri::command]
pub async fn add_favorite_server(
    state: State<'_, AppState>,
    name: String,
    address: String,
) -> CmdResult<Vec<FavoriteServer>> {
    let l = state.launcher()?.clone();
    blocking(move || l.servers.add(&name, &address)).await
}

#[tauri::command]
pub async fn remove_favorite_server(
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<Vec<FavoriteServer>> {
    let l = state.launcher()?.clone();
    blocking(move || l.servers.remove(&id)).await
}

#[tauri::command]
pub async fn reorder_favorite_servers(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> CmdResult<Vec<FavoriteServer>> {
    let l = state.launcher()?.clone();
    blocking(move || l.servers.reorder(&ids)).await
}

/// The instance's in-game multiplayer list.
#[tauri::command]
pub async fn list_game_servers(
    state: State<'_, AppState>,
    instance_id: String,
) -> CmdResult<Vec<GameServer>> {
    let l = state.launcher()?.clone();
    blocking(move || {
        let dir = l.instances.dir(&instance_id)?;
        dat::list(&dir.join(dat::FILE))
    })
    .await
}

/// Writes are refused while the instance runs: the game rewrites the file
/// from memory and would drop the change.
#[tauri::command]
pub async fn add_game_server(
    state: State<'_, AppState>,
    instance_id: String,
    name: String,
    address: String,
) -> CmdResult<Vec<GameServer>> {
    let l = state.launcher()?.clone();
    blocking(move || {
        let inst = l.instances.get(&instance_id)?;
        l.running.ensure_idle(&inst)?;
        let addr = ServerAddress::parse(&address)?;
        let name = name.trim();
        let name = if name.is_empty() {
            addr.to_string()
        } else {
            name.chars().filter(|c| !c.is_control()).take(64).collect()
        };
        dat::add(&l.instances.dir(&inst.id)?.join(dat::FILE), &name, &addr)
    })
    .await
}

#[tauri::command]
pub async fn remove_game_server(
    state: State<'_, AppState>,
    instance_id: String,
    index: u32,
    address: String,
) -> CmdResult<Vec<GameServer>> {
    let l = state.launcher()?.clone();
    blocking(move || {
        let inst = l.instances.get(&instance_id)?;
        l.running.ensure_idle(&inst)?;
        dat::remove(&l.instances.dir(&inst.id)?.join(dat::FILE), index, &address)
    })
    .await
}

/// Raw TCP status ping to an address the user entered or listed.
#[tauri::command]
pub async fn ping_server(address: String) -> CmdResult<ServerStatus> {
    let addr = ServerAddress::parse(&address)?;
    Ok(ping::ping(&addr).await?)
}
