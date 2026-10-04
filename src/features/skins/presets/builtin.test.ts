import { readFileSync } from "node:fs";
import { inflateSync } from "node:zlib";
import { describe, expect, it } from "vitest";

import { CAPE_PRESETS, SKIN_PRESETS } from ".";

/** Decodes the unfiltered RGBA PNGs exported for the built-in library items. */
function decode(path: string): { width: number; height: number; data: Uint8Array } {
  const buf = readFileSync(path);
  const width = buf.readUInt32BE(16);
  const height = buf.readUInt32BE(20);
  const idat: Buffer[] = [];
  for (let o = 8; o < buf.length;) {
    const len = buf.readUInt32BE(o);
    if (buf.toString("ascii", o + 4, o + 8) === "IDAT") idat.push(buf.subarray(o + 8, o + 8 + len));
    o += 12 + len;
  }
  const raw = inflateSync(Buffer.concat(idat));
  const data = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y++) {
    expect(raw[y * (width * 4 + 1)]).toBe(0); // filter type None
    data.set(raw.subarray(y * (width * 4 + 1) + 1, (y + 1) * (width * 4 + 1)), y * width * 4);
  }
  return { width, height, data };
}

const dir = "crates/launcher-core/assets/builtin";

describe("built-in MehburMC textures", () => {
  // If this fails, re-export the PNGs from the "mehbur" presets.
  it("match the presets they were exported from", () => {
    const skin = decode(`${dir}/mehbur-skin.png`);
    const cape = decode(`${dir}/mehbur-cape.png`);
    const s = SKIN_PRESETS.find((p) => p.id === "mehbur")!.draw();
    const c = CAPE_PRESETS.find((p) => p.id === "mehbur")!.draw();
    expect([skin.width, skin.height]).toEqual([64, 64]);
    expect([cape.width, cape.height]).toEqual([64, 32]);
    expect(Array.from(skin.data)).toEqual(Array.from(s.data));
    expect(Array.from(cape.data)).toEqual(Array.from(c.data));
  });
});
