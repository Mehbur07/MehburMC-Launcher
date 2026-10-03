import { describe, expect, it } from "vitest";

import { frontParts } from "./skinRender";

describe("frontParts", () => {
  it("fits every part inside the 16×32 front view", () => {
    for (const model of ["classic", "slim"] as const) {
      for (const legacy of [false, true]) {
        for (const p of frontParts(model, legacy)) {
          expect(p.dx).toBeGreaterThanOrEqual(0);
          expect(p.dx + p.w).toBeLessThanOrEqual(16);
          expect(p.dy + p.h).toBeLessThanOrEqual(32);
          // Source stays inside the texture (64×32 for legacy skins).
          expect(p.sx + p.w).toBeLessThanOrEqual(64);
          expect(p.sy + p.h).toBeLessThanOrEqual(legacy ? 32 : 64);
        }
      }
    }
  });

  it("uses 3px arms for slim skins and mirrors limbs on legacy skins", () => {
    const slim = frontParts("slim", false);
    expect(slim.find((p) => p.sx === 44 && p.sy === 20)?.w).toBe(3);
    const legacy = frontParts("classic", true);
    expect(legacy.filter((p) => p.flip)).toHaveLength(2);
    expect(legacy.some((p) => p.sy >= 32)).toBe(false);
  });
});
