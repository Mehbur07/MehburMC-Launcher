import { describe, expect, it, vi } from "vitest";

import { shortcut } from "./mac";

describe("shortcut", () => {
  it("uses Mac symbols on macOS", () => {
    expect(shortcut("Z", {}, true)).toBe("⌘Z");
    expect(shortcut("Z", { shift: true }, true)).toBe("⇧⌘Z");
  });

  it("uses Ctrl elsewhere", () => {
    expect(shortcut("Z", {}, false)).toBe("Ctrl+Z");
    expect(shortcut("Y", {}, false)).toBe("Ctrl+Y");
  });
});

describe("syncMenuLanguage", () => {
  it("asks Rust only on a Mac and only when the language changes", async () => {
    const { ipc } = await import("./ipc");
    const spy = vi.spyOn(ipc, "setMenuLanguage").mockResolvedValue();
    const { syncMenuLanguage } = await import("./mac");
    syncMenuLanguage("tr", false);
    expect(spy).not.toHaveBeenCalled();
    syncMenuLanguage("tr", true);
    syncMenuLanguage("tr", true);
    syncMenuLanguage("en", true);
    expect(spy.mock.calls).toEqual([["tr"], ["en"]]);
  });
});
