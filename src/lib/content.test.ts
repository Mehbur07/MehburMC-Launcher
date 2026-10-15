import { describe, expect, it } from "vitest";

import { baseName, stableKeys, toggledName } from "./content";

describe("content names", () => {
  it("adds and drops the .disabled suffix", () => {
    expect(baseName("a.jar.disabled")).toBe("a.jar");
    expect(baseName("a.jar")).toBe("a.jar");
    expect(toggledName("a.jar")).toBe("a.jar.disabled");
    expect(toggledName("a.jar.disabled")).toBe("a.jar");
  });

  it("keeps a row key across a toggle unless both copies exist", () => {
    const key = stableKeys(["a.jar", "b.jar.disabled"]);
    expect(key("a.jar")).toBe(key("a.jar.disabled"));
    expect(key("b.jar.disabled")).toBe("b.jar");

    const both = stableKeys(["c.jar", "c.jar.disabled"]);
    expect(both("c.jar")).not.toBe(both("c.jar.disabled"));
  });
});
