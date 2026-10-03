import { describe, expect, it } from "vitest";

import type { VersionSummary } from "../../lib/ipc/bindings/VersionSummary";
import { compactNumber, modrinthLoader, pickVersion } from "./content";

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
});
