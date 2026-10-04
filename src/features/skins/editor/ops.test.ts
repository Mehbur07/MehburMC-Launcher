import { describe, expect, it } from "vitest";

import { CAPE_PRESETS, SKIN_PRESETS } from "../presets";
import { face, locate, mirrorPixel, skinBoxes } from "./layout";
import {
  History,
  blank,
  clearLayer,
  floodFill,
  getPx,
  hexToRgba,
  linePoints,
  putPx,
  rgbaToHex,
  stamp,
} from "./ops";

const RED = hexToRgba("#ff0000");
const classic = skinBoxes("classic");

describe("skin layout", () => {
  it("locates faces and tells base from overlay", () => {
    expect(locate(classic, 8, 8)?.box.id).toBe("head");
    expect(locate(classic, 8, 8)?.name).toBe("front");
    expect(locate(classic, 40, 8)?.box.layer).toBe("overlay");
    expect(locate(classic, 0, 0)).toBeNull();
  });

  it("mirrors limbs onto the opposite limb, flipped", () => {
    const rFront = face(
      classic.find((b) => b.id === "rArm")!,
      "front",
    );
    const lFront = face(
      classic.find((b) => b.id === "lArm")!,
      "front",
    );
    expect(mirrorPixel(classic, rFront.x, rFront.y + 3)).toEqual([lFront.x + 3, lFront.y + 3]);
    // Head mirrors onto itself; the outer side faces swap.
    const head = classic.find((b) => b.id === "head")!;
    expect(mirrorPixel(classic, 8, 8)).toEqual([15, 8]);
    const right = face(head, "right");
    const left = face(head, "left");
    expect(mirrorPixel(classic, right.x, right.y)).toEqual([left.x + left.w - 1, left.y]);
  });

  it("uses 3 px arms on slim skins", () => {
    const slim = skinBoxes("slim");
    expect(slim.find((b) => b.id === "rArm")?.w).toBe(3);
    // The last columns of the classic arm's back face are unused on slim skins.
    expect(locate(classic, 54, 20)?.box.id).toBe("rArm");
    expect(locate(slim, 54, 20)).toBeNull();
  });
});

describe("pixel ops", () => {
  it("converts colours", () => {
    expect(hexToRgba("#10a0ff")).toEqual([16, 160, 255, 255]);
    expect(rgbaToHex([16, 160, 255, 255])).toBe("#10a0ff");
  });

  it("stamps with mirror and layer limits", () => {
    const p = blank(64, 64);
    stamp(p, 8, 8, 1, RED, { boxes: classic, layer: "base", mirror: true });
    expect(getPx(p, 8, 8)).toEqual(RED);
    expect(getPx(p, 15, 8)).toEqual(RED);
    // Painting the base layer leaves the hat untouched.
    stamp(p, 40, 8, 1, RED, { boxes: classic, layer: "base", mirror: false });
    expect(getPx(p, 40, 8)[3]).toBe(0);
  });

  it("fills only inside the clicked face", () => {
    const p = blank(64, 64);
    const n = floodFill(p, 8, 8, RED, { boxes: classic, layer: null, mirror: false });
    expect(n).toBe(64); // 8×8 head front
    expect(getPx(p, 7, 8)[3]).toBe(0); // head right side untouched
    expect(floodFill(p, 8, 8, RED, { boxes: classic, layer: null, mirror: false })).toBe(0);
  });

  it("draws Bresenham lines with both ends", () => {
    expect(linePoints(0, 0, 3, 1)).toEqual([
      [0, 0],
      [1, 0],
      [2, 1],
      [3, 1],
    ]);
    expect(linePoints(2, 2, 2, 2)).toEqual([[2, 2]]);
  });

  it("clears one layer", () => {
    const p = blank(64, 64);
    putPx(p, 8, 8, RED);
    putPx(p, 40, 8, RED);
    clearLayer(p, classic, "overlay");
    expect(getPx(p, 8, 8)).toEqual(RED);
    expect(getPx(p, 40, 8)[3]).toBe(0);
  });

  it("undoes and redoes", () => {
    const p = blank(4, 4);
    const h = new History(2);
    expect(h.canUndo).toBe(false);
    for (let i = 1; i <= 3; i++) {
      h.push(p);
      putPx(p, 0, 0, [i, 0, 0, 255]);
    }
    expect(h.undo(p)).toBe(true);
    expect(getPx(p, 0, 0)[0]).toBe(2);
    expect(h.undo(p)).toBe(true);
    expect(getPx(p, 0, 0)[0]).toBe(1);
    // Limit of 2 snapshots: the blank state is gone.
    expect(h.undo(p)).toBe(false);
    expect(h.redo(p)).toBe(true);
    expect(getPx(p, 0, 0)[0]).toBe(2);
  });
});

describe("presets", () => {
  it("draw valid, opaque skins and capes", () => {
    const ids = new Set<string>();
    for (const s of SKIN_PRESETS) {
      const p = s.draw();
      expect([p.width, p.height]).toEqual([64, 64]);
      // Every base-layer pixel is opaque, as the game expects.
      for (const b of skinBoxes(s.model).filter((x) => x.layer === "base")) {
        const f = face(b, "front");
        for (let y = f.y; y < f.y + f.h; y++)
          for (let x = f.x; x < f.x + f.w; x++) expect(getPx(p, x, y)[3]).toBe(255);
      }
      ids.add(s.id);
    }
    for (const c of CAPE_PRESETS) {
      const p = c.draw();
      expect([p.width, p.height]).toEqual([64, 32]);
      expect(getPx(p, 1, 1)[3]).toBe(255);
      ids.add(c.id);
    }
    expect(ids.size).toBe(SKIN_PRESETS.length + CAPE_PRESETS.length);
  });

  it("are deterministic", () => {
    const a = SKIN_PRESETS[0]!.draw();
    const b = SKIN_PRESETS[0]!.draw();
    expect(Array.from(a.data)).toEqual(Array.from(b.data));
  });
});
