import { describe, expect, it } from "vitest";

import type { VersionSummary } from "../../lib/ipc/bindings/VersionSummary";
import {
  compactNumber,
  folderFor,
  installedByProject,
  modrinthLoader,
  pickVersion,
} from "./content";

const v = (id: string, versionType: string): VersionSummary => ({
  id,
  name: id,
  versionNumber: id,
  versionType,
  gameVersions: [],
  loaders: [],
  datePublished: "",
});

describe("content helpers", () => {
  it("formats download counts", () => {
    expect(compactNumber(999)).toBe("999");
    expect(compactNumber(1234)).toBe("1.2K");
    expect(compactNumber(2_000_000)).toBe("2M");
    expect(compactNumber(235_049_829)).toBe("235M");
  });

  it("maps loaders to Modrinth tags", () => {
    expect(modrinthLoader("neoForge")).toBe("neoforge");
    expect(modrinthLoader("legacyFabric")).toBe("legacy-fabric");
    expect(modrinthLoader("vanilla")).toBeNull();
  });

  it("prefers releases", () => {
    expect(pickVersion([v("a", "beta"), v("b", "release")])?.id).toBe("b");
    expect(pickVersion([v("a", "alpha")])?.id).toBe("a");
    expect(pickVersion([])).toBeUndefined();
  });

  it("maps content types to instance folders", () => {
    expect(folderFor("mod")).toBe("mods");
    expect(folderFor("resourcepack")).toBe("resourcePacks");
    expect(folderFor("shader")).toBe("shaderPacks");
    expect(folderFor("modpack")).toBeNull();
  });

  it("groups installed files by project", () => {
    const f = (fileName: string, projectId: string | null) => ({
      fileName,
      enabled: true,
      size: 0,
      projectId,
      slug: null,
      title: null,
      versionNumber: null,
      iconUrl: null,
      update: null,
    });
    const g = installedByProject([f("a.jar", "p"), f("b.jar", "p"), f("c.jar", null)]);
    expect(Object.keys(g)).toEqual(["p"]);
    expect(g.p?.map((x) => x.fileName)).toEqual(["a.jar", "b.jar"]);
  });
});
