import { ipc } from "../../lib/ipc";
import type { Folder } from "../../lib/ipc/bindings/Folder";
import type { ImportResult } from "../../lib/ipc/bindings/ImportResult";
import type { InstalledItem } from "../../lib/ipc/bindings/InstalledItem";
import type { LoaderKind } from "../../lib/ipc/bindings/LoaderKind";
import type { ProjectType } from "../../lib/ipc/bindings/ProjectType";
import type { VersionSummary } from "../../lib/ipc/bindings/VersionSummary";

/** Modrinth loader tag for mod searches on an instance (null = no mods). */
export function modrinthLoader(kind: LoaderKind): string | null {
  return (
    {
      fabric: "fabric",
      quilt: "quilt",
      legacyFabric: "legacy-fabric",
      forge: "forge",
      neoForge: "neoforge",
      vanilla: null,
      optifine: null,
    } satisfies Record<LoaderKind, string | null>
  )[kind];
}

/** 1234 → "1.2K", 235049829 → "235M". */
export function compactNumber(n: number): string {
  if (n < 1000) return String(n);
  const units = [
    [1e9, "B"],
    [1e6, "M"],
    [1e3, "K"],
  ] as const;
  for (const [v, u] of units) {
    if (n >= v) {
      const x = n / v;
      return `${x >= 100 ? Math.round(x) : x.toFixed(1).replace(/\.0$/, "")}${u}`;
    }
  }
  return String(n);
}

/** Newest release, else the newest version. */
export function pickVersion(versions: VersionSummary[]): VersionSummary | undefined {
  return versions.find((v) => v.versionType === "release") ?? versions[0];
}

/** Installs a Modrinth modpack (chosen version or the newest release). */
export async function installModpack(projectId: string, versionId?: string): Promise<ImportResult> {
  let id = versionId;
  if (!id) {
    const v = pickVersion(await ipc.projectVersions(projectId, "modpack"));
    if (!v) throw { code: "content.modpackInvalid", params: { reason: "no versions" }, detail: "" };
    id = v.id;
  }
  return ipc.installModrinthModpack(id);
}

/** Instance folder that holds a content type (modpacks become instances). */
export function folderFor(type: ProjectType): Folder | null {
  return (
    {
      mod: "mods",
      resourcepack: "resourcePacks",
      shader: "shaderPacks",
      modpack: null,
    } satisfies Record<ProjectType, Folder | null>
  )[type];
}

/** Installed files per Modrinth project id. */
export function installedByProject(items: InstalledItem[]): Record<string, InstalledItem[]> {
  const out: Record<string, InstalledItem[]> = {};
  for (const i of items) if (i.projectId) (out[i.projectId] ??= []).push(i);
  return out;
}
