import { describe, expect, it } from "vitest";

import type { InstalledItem } from "./ipc/bindings/InstalledItem";
import type { ModGroup } from "./ipc/bindings/ModGroup";
import {
  filesToSwitch,
  groupItems,
  groupState,
  isMember,
  memberFromItem,
  refreshMembers,
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

const group = (id: string, files: string[]): ModGroup => ({
  id,
  name: id,
  mods: files.map((file) => ({ file, projectId: null, modId: null, title: null })),
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
    });
    expect(isMember(m, sodium)).toBe(true);
    // Offline (no Modrinth project): the jar's own id still matches.
    expect(isMember(m, item("sodium-0.7.jar", { modId: "sodium" }))).toBe(true);
    expect(isMember(m, item("lithium.jar", { modId: "lithium" }))).toBe(false);
    expect(isMember(group("g", ["x.jar"]).mods[0]!, item("x.jar.disabled"))).toBe(true);
    expect(refreshMembers({ ...group("g", []), mods: [m] }, [sodium]).mods[0]!.file).toBe(
      "sodium-0.6.jar",
    );
  });

  it("reads a group's state from its mods", () => {
    const items = [item("a.jar"), item("b.jar.disabled"), item("c.jar")];
    expect(groupState(group("g", ["a.jar", "c.jar"]), items)).toBe("on");
    expect(groupState(group("g", ["b.jar"]), items)).toBe("off");
    expect(groupState(group("g", ["a.jar", "b.jar"]), items)).toBe("mixed");
    expect(groupState(group("g", ["gone.jar"]), items)).toBe("empty");
    expect(groupItems(group("g", ["a.jar", "gone.jar"]), items)).toHaveLength(1);
  });

  it("switches only the group's mods and keeps those another group needs", () => {
    const items = [item("a.jar"), item("b.jar"), item("c.jar.disabled"), item("d.jar")];
    const pvp = group("pvp", ["a.jar", "b.jar", "c.jar"]);
    const perf = group("perf", ["b.jar", "d.jar"]);
    const all = [pvp, perf];
    expect(filesToSwitch(pvp, all, items, true)).toEqual(["c.jar.disabled"]);
    // b.jar is also in "perf", which is on: it stays on.
    expect(filesToSwitch(pvp, all, items, false)).toEqual(["a.jar"]);
    // Once "perf" is not fully on, b.jar goes off with "pvp".
    const perfMixed = [item("a.jar"), item("b.jar"), item("d.jar.disabled")];
    expect(filesToSwitch(pvp, all, perfMixed, false)).toEqual(["a.jar", "b.jar"]);
  });
});
