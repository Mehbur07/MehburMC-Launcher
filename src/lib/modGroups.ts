/**
 * Mod groups (K91, K92): a named record of which mods are on and which are
 * off, taken when the group is made. Applying it puts those mods back into
 * that state; mods added later are left alone.
 */
import { baseName } from "./content";
import type { GroupMod } from "./ipc/bindings/GroupMod";
import type { InstalledItem } from "./ipc/bindings/InstalledItem";
import type { ModGroup } from "./ipc/bindings/ModGroup";

/** `active` = every recorded mod that is installed is as recorded. */
export type GroupState = "active" | "inactive" | "empty";

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

/** A mod as recorded now (its current on/off state). */
export const memberFromItem = (i: InstalledItem): GroupMod => ({
  file: baseName(i.fileName),
  projectId: i.projectId,
  modId: i.modId,
  title: modLabel(i),
  enabled: i.enabled,
});

/** Every installed mod with its current state. */
export const snapshot = (items: InstalledItem[]): GroupMod[] => items.map(memberFromItem);

/** Installed mods paired with their recorded state. */
function recorded(g: ModGroup, items: InstalledItem[]) {
  return items.flatMap((i) => {
    const m = g.mods.find((x) => isMember(x, i));
    return m ? [{ item: i, want: m.enabled }] : [];
  });
}

export function groupState(g: ModGroup, items: InstalledItem[]): GroupState {
  const r = recorded(g, items);
  if (r.length === 0) return "empty";
  return r.every((x) => x.item.enabled === x.want) ? "active" : "inactive";
}

/** How many recorded mods the group switches on and off. */
export function groupCounts(g: ModGroup) {
  const on = g.mods.filter((m) => m.enabled).length;
  return { on, off: g.mods.length - on };
}

/** Files to switch on and off to apply the group. */
export function applyPlan(g: ModGroup, items: InstalledItem[]) {
  const r = recorded(g, items).filter((x) => x.item.enabled !== x.want);
  return {
    enable: r.filter((x) => x.want).map((x) => x.item.fileName),
    disable: r.filter((x) => !x.want).map((x) => x.item.fileName),
  };
}

/** Members with the current file names, ids and titles; states stay as recorded. */
export function refreshMembers(g: ModGroup, items: InstalledItem[]): ModGroup {
  return {
    ...g,
    mods: g.mods.map((m) => {
      const i = items.find((x) => isMember(m, x));
      return i ? { ...memberFromItem(i), enabled: m.enabled } : m;
    }),
  };
}

export const newGroupId = () =>
  `g-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
