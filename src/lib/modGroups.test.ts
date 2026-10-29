import { describe, expect, it } from "vitest";

import type { InstalledItem } from "./ipc/bindings/InstalledItem";
import type { ModGroup } from "./ipc/bindings/ModGroup";
import {
  applyPlan,
  groupCounts,
  groupState,
  isMember,
  memberFromItem,
  refreshMembers,
  snapshot,
} from "./modGroups";

const item = (fileName: string, over: Partial<InstalledItem> = {}): InstalledItem => ({
  fileName,
  enabled: !fileName.endsWith(".disabled"),
  size: 1,
  projectId: null,
  slug: null,
  title: null,
  versionNumber: null,
  iconUrl: null,
  modId: null,
  update: null,
  ...over,
});

const group = (mods: [string, boolean][]): ModGroup => ({
  id: "g",
  name: "g",
  mods: mods.map(([file, enabled]) => ({
    file,
    projectId: null,
    modId: null,
    title: null,
    enabled,
  })),
});

describe("mod groups", () => {
  it("matches members by project, mod id or file and survives a rename", () => {
    const sodium = item("sodium-0.6.jar", { projectId: "AANobbMI", modId: "sodium" });
    const m = memberFromItem(item("sodium-0.5.jar", { projectId: "AANobbMI", modId: "sodium" }));
    expect(m).toEqual({
      file: "sodium-0.5.jar",
      projectId: "AANobbMI",
      modId: "sodium",
      title: "sodium-0.5",
      enabled: true,
    });
    expect(isMember(m, sodium)).toBe(true);
    // Offline (no Modrinth project): the jar's own id still matches.
    expect(isMember(m, item("sodium-0.7.jar", { modId: "sodium" }))).toBe(true);
    expect(isMember(m, item("lithium.jar", { modId: "lithium" }))).toBe(false);
    expect(isMember(group([["x.jar", true]]).mods[0]!, item("x.jar.disabled"))).toBe(true);
    const off = { ...m, enabled: false };
    const fresh = refreshMembers({ ...group([]), mods: [off] }, [sodium]).mods[0]!;
    expect(fresh.file).toBe("sodium-0.6.jar");
    expect(fresh.enabled).toBe(false);
  });

  it("records the current state of every mod", () => {
    const g = { ...group([]), mods: snapshot([item("a.jar"), item("b.jar.disabled")]) };
    expect(g.mods.map((m) => [m.file, m.enabled])).toEqual([
      ["a.jar", true],
      ["b.jar", false],
    ]);
    expect(groupCounts(g)).toEqual({ on: 1, off: 1 });
  });

  it("is active while the mods are as recorded and applies only recorded mods", () => {
    const g = group([
      ["a.jar", true],
      ["b.jar", false],
      ["gone.jar", true],
    ]);
    const now = [item("a.jar.disabled"), item("b.jar"), item("new.jar")];
    expect(groupState(g, now)).toBe("inactive");
    // new.jar came later: left alone.
    expect(applyPlan(g, now)).toEqual({ enable: ["a.jar.disabled"], disable: ["b.jar"] });
    const applied = [item("a.jar"), item("b.jar.disabled"), item("new.jar")];
    expect(groupState(g, applied)).toBe("active");
    expect(applyPlan(g, applied)).toEqual({ enable: [], disable: [] });
    expect(groupState(g, [item("other.jar")])).toBe("empty");
  });
});
