import { createHash } from "node:crypto";
import { describe, expect, it } from "vitest";

import en from "../../../i18n/en.json";
import tr from "../../../i18n/tr.json";
import { faces, skinBoxes } from "../editor/layout";
import { getPx } from "../editor/ops";
import { CAPE_PRESETS, SKIN_PRESETS } from ".";
import { LIBRARY_CAPES, LIBRARY_SKINS } from "./library";

const sha1 = (data: Uint8ClampedArray) => createHash("sha1").update(data).digest("hex");

describe("MehburMC collection (phase 22)", () => {
  it("has 20 skins and 15 capes with unique ids", () => {
    expect(LIBRARY_SKINS).toHaveLength(20);
    expect(LIBRARY_CAPES).toHaveLength(15);
    const ids = [...LIBRARY_SKINS, ...LIBRARY_CAPES].map((p) => p.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("names every item in both languages", () => {
    for (const p of [...LIBRARY_SKINS, ...LIBRARY_CAPES]) {
      expect(tr.skins.presets.items).toHaveProperty(p.id);
      expect(en.skins.presets.items).toHaveProperty(p.id);
    }
  });

  it("draws valid textures with an opaque base layer", () => {
    for (const p of LIBRARY_SKINS) {
      const px = p.draw();
      expect([px.width, px.height]).toEqual([64, 64]);
      for (const b of skinBoxes(p.model).filter((b) => b.layer === "base")) {
        for (const f of faces(b)) {
          for (let y = 0; y < f.h; y++) {
            for (let x = 0; x < f.w; x++) {
              expect(getPx(px, f.x + x, f.y + y)[3], `${p.id} ${b.id} ${f.name}`).toBe(255);
            }
          }
        }
      }
    }
    for (const p of LIBRARY_CAPES) {
      const px = p.draw();
      expect([px.width, px.height]).toEqual([64, 32]);
      // The visible outside of the cape is fully painted.
      for (let y = 1; y < 17; y++) {
        for (let x = 1; x < 11; x++) expect(getPx(px, x, y)[3], p.id).toBe(255);
      }
    }
  });

  it("is deterministic and every design is different", () => {
    const all = [...SKIN_PRESETS, ...CAPE_PRESETS, ...LIBRARY_SKINS, ...LIBRARY_CAPES];
    const hashes = all.map((p) => sha1(p.draw().data));
    expect(new Set(hashes).size).toBe(hashes.length);
    for (const p of [...LIBRARY_SKINS, ...LIBRARY_CAPES]) {
      expect(sha1(p.draw().data)).toBe(sha1(p.draw().data));
    }
  });

  // Pins the designs: if one changes on purpose, update the snapshot.
  it("keeps the published designs", () => {
    const pins = Object.fromEntries(
      [...LIBRARY_SKINS, ...LIBRARY_CAPES].map((p) => [p.id, sha1(p.draw().data)]),
    );
    expect(pins).toMatchSnapshot();
  });
});
