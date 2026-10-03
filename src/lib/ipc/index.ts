import { invoke } from "@tauri-apps/api/core";

import type { Bootstrap } from "./bindings/Bootstrap";
import type { ErrorPayload } from "./bindings/ErrorPayload";
import type { Settings } from "./bindings/Settings";

/** Typed wrappers around the Rust commands (see src-tauri/src/commands.rs). */
export const ipc = {
  getBootstrap: () => invoke<Bootstrap>("get_bootstrap"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
  openDataDir: () => invoke<void>("open_data_dir"),
};

export function isErrorPayload(e: unknown): e is ErrorPayload {
  return typeof e === "object" && e !== null && "code" in e && "detail" in e;
}

/** Normalizes anything thrown by `invoke` into an ErrorPayload. */
export function toErrorPayload(e: unknown): ErrorPayload {
  if (isErrorPayload(e)) return e;
  return { code: "unknown", params: {}, detail: e instanceof Error ? e.message : String(e) };
}
