/**
 * Mod groups (K91): named sets of mods switched on or off together. A
 * group's state is read from the mods, so switching single mods by hand
 * keeps it truthful.
 */
import { baseName } from "./content";
import type { GroupMod } from "./ipc/bindings/GroupMod";
import type { InstalledItem } from "./ipc/bindings/InstalledItem";
import type { ModGroup } from "./ipc/bindings/ModGroup";

export type GroupState = "on" | "off" | "mixed" | "empty";

export const modLabel = (i: InstalledItem) =>
  i.title ?? baseName(i.fileName).replace(/\.jar$/i, "");

/** A member matches by Modrinth project, own mod id or file name. */
export function isMember(m: GroupMod, item: InstalledItem): boolean {
  return (
    (m.projectId !== null && m.projectId === item.projectId) ||
    (m.modId !== null && m.modId === item.modId) ||
    m.file === baseName(item.fileName)
  );
}

export const memberFromItem = (i: InstalledItem): GroupMod => ({
  file: baseName(i.fileName),
  projectId: i.projectId,
  modId: i.modId,
  title: modLabel(i),
});

/** Installed mods of the group (members no longer installed are left out). */
export const groupItems = (g: ModGroup, items: InstalledItem[]) =>
  items.filter((i) => g.mods.some((m) => isMember(m, i)));

export function groupState(g: ModGroup, items: InstalledItem[]): GroupState {
  const mine = groupItems(g, items);
  if (mine.length === 0) return "empty";
  const on = mine.filter((i) => i.enabled).length;
  return on === mine.length ? "on" : on === 0 ? "off" : "mixed";
}

/**
 * Files to switch for turning `g` on or off. Turning off leaves alone the
 * mods that another group which is on also holds.
 */
export function filesToSwitch(
  g: ModGroup,
  groups: ModGroup[],
  items: InstalledItem[],
  enable: boolean,
): string[] {
  const mine = groupItems(g, items).filter((i) => i.enabled !== enable);
  if (enable) return mine.map((i) => i.fileName);
  const keep = groups.filter((o) => o.id !== g.id && groupState(o, items) === "on");
  return mine
    .filter((i) => !keep.some((o) => o.mods.some((m) => isMember(m, i))))
    .map((i) => i.fileName);
}

/** Members with the current file names, ids and titles of installed mods. */
export function refreshMembers(g: ModGroup, items: InstalledItem[]): ModGroup {
  return {
    ...g,
    mods: g.mods.map((m) => {
      const i = items.find((x) => isMember(m, x));
      return i ? memberFromItem(i) : m;
    }),
  };
}

export const newGroupId = () =>
  `g-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
