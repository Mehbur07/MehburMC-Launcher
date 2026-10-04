import { describe, expect, it } from "vitest";

import changelogRaw from "../../../CHANGELOG.md?raw";
import { parseChangelog, parseInline } from "./changelog";

describe("changelog", () => {
  it("splits releases, headings and wrapped bullets", () => {
    const md = [
      "# Değişiklik Günlüğü",
      "intro is skipped",
      "## [0.2.0] — 2026-10-04",
      "### Eklendi",
      "- **Skin** stüdyosu: hazır",
      "  skinler ve editör.",
      "- İkinci madde",
      "## [0.1.0] - 2026-10-03",
      "Düz metin",
    ].join("\n");
    const r = parseChangelog(md);
    expect(r.map((x) => [x.version, x.label])).toEqual([
      ["0.2.0", "2026-10-04"],
      ["0.1.0", "2026-10-03"],
    ]);
    expect(r[0]!.blocks).toEqual([
      { kind: "heading", text: "Eklendi" },
      { kind: "bullet", text: "**Skin** stüdyosu: hazır skinler ve editör." },
      { kind: "bullet", text: "İkinci madde" },
    ]);
    expect(r[1]!.blocks).toEqual([{ kind: "text", text: "Düz metin" }]);
  });

  it("parses bold, code and links", () => {
    expect(parseInline("a **b** `c` [d](http://x) e")).toEqual([
      { kind: "text", text: "a " },
      { kind: "bold", text: "b" },
      { kind: "text", text: " " },
      { kind: "code", text: "c" },
      { kind: "text", text: " " },
      { kind: "text", text: "d" },
      { kind: "text", text: " e" },
    ]);
  });

  it("has a section for the version in tauri.conf.json", async () => {
    const { readFileSync } = await import("node:fs");
    const conf = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8")) as {
      version: string;
    };
    const versions = parseChangelog(changelogRaw).map((r) => r.version);
    expect(versions[0]).toBe(conf.version);
  });
});
