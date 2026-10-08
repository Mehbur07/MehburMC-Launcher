import { createHash } from "node:crypto";
import { describe, expect, it } from "vitest";

import en from "../../../i18n/en.json";
import tr from "../../../i18n/tr.json";
import { faces, skinBoxes } from "../editor/layout";
import { getPx } from "../editor/ops";
import { CAPE_PRESETS, SKIN_PRESETS } from ".";
import { CATALOG_CAPES, CATALOG_SKINS } from "./catalog";
import { LIBRARY_CAPES, LIBRARY_SKINS } from "./library";

const sha1 = (data: Uint8ClampedArray) => createHash("sha1").update(data).digest("hex");
const ALL_SKINS = [...SKIN_PRESETS, ...LIBRARY_SKINS, ...CATALOG_SKINS];
const ALL_CAPES = [...CAPE_PRESETS, ...LIBRARY_CAPES, ...CATALOG_CAPES];

describe("MehburMC catalogue (K77)", () => {
  it("offers at least 100 skins and 100 capes with unique ids", () => {
    expect(ALL_SKINS.length).toBeGreaterThanOrEqual(100);
    expect(ALL_CAPES.length).toBeGreaterThanOrEqual(100);
    const ids = [...ALL_SKINS, ...ALL_CAPES].map((p) => p.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("names every item in both languages", () => {
    for (const p of [...CATALOG_SKINS, ...CATALOG_CAPES]) {
      expect(tr.skins.presets.items, p.id).toHaveProperty(p.id);
      expect(en.skins.presets.items, p.id).toHaveProperty(p.id);
    }
  });

  it("draws valid textures with an opaque base layer", () => {
    for (const p of CATALOG_SKINS) {
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
    for (const p of CATALOG_CAPES) {
      const px = p.draw();
      expect([px.width, px.height]).toEqual([64, 32]);
      for (let y = 1; y < 17; y++) {
        for (let x = 1; x < 11; x++) expect(getPx(px, x, y)[3], p.id).toBe(255);
      }
    }
  });

  it("is deterministic and every design in the whole collection is different", () => {
    const hashes = [...ALL_SKINS, ...ALL_CAPES].map((p) => sha1(p.draw().data));
    expect(new Set(hashes).size).toBe(hashes.length);
    for (const p of [...CATALOG_SKINS, ...CATALOG_CAPES]) {
      expect(sha1(p.draw().data), p.id).toBe(sha1(p.draw().data));
    }
  });

  // Pins the designs: if one changes on purpose, update the snapshot.
  it("keeps the published designs", () => {
    const pins = Object.fromEntries(
      [...CATALOG_SKINS, ...CATALOG_CAPES].map((p) => [p.id, sha1(p.draw().data)]),
    );
    expect(pins).toMatchSnapshot();
  });
});
