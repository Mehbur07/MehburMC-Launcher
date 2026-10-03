import { describe, expect, it } from "vitest";

import type { ManifestEntry } from "../../lib/ipc/bindings/ManifestEntry";
import { filterVersions } from "./VersionPicker";

const v = (id: string, type: string): ManifestEntry => ({
  id,
  type,
  url: "",
  releaseTime: "",
  sha1: null,
});
const ALL = [
  v("26.4-snapshot-2", "snapshot"),
  v("26.3", "release"),
  v("1.12.2", "release"),
  v("b1.7.3", "old_beta"),
];

describe("filterVersions", () => {
  it("filters by type and query, keeping manifest order", () => {
    expect(filterVersions(ALL, new Set(["release"]), "").map((x) => x.id)).toEqual([
      "26.3",
      "1.12.2",
    ]);
    expect(filterVersions(ALL, new Set(["release", "snapshot"]), "26").map((x) => x.id)).toEqual([
      "26.4-snapshot-2",
      "26.3",
    ]);
    expect(filterVersions(ALL, new Set(["old_beta"]), "")).toHaveLength(1);
    expect(filterVersions(ALL, new Set(["release"]), "nope")).toHaveLength(0);
  });
});
