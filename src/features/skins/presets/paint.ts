// Small painting toolkit for the code-drawn preset textures.

import type { SkinModel } from "../../../lib/ipc/bindings/SkinModel";
import {
  type Box,
  CAPE_BOXES,
  type Face,
  type FaceName,
  face,
  faces,
  skinBoxes,
} from "../editor/layout";
import { type Pixels, type Rgba, blank, hexToRgba, putPx } from "../editor/ops";

export type Color = Rgba;
export const hex = (h: string): Color => hexToRgba(h);

/** Lightens (amt > 0) or darkens a colour, keeping alpha. */
export function shade(c: Color, amt: number): Color {
  const f = (v: number) => Math.max(0, Math.min(255, Math.round(v + amt)));
  return [f(c[0]), f(c[1]), f(c[2]), c[3]];
}

export function mix(a: Color, b: Color, t: number): Color {
  const f = (x: number, y: number) => Math.round(x + (y - x) * t);
  return [f(a[0], b[0]), f(a[1], b[1]), f(a[2], b[2]), f(a[3], b[3])];
}

/** Deterministic PRNG (mulberry32) so presets look the same every time. */
export function rng(seed: number): () => number {
  let s = seed >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Per-pixel brightness jitter for a hand-drawn look. */
export function grain(c: Color, r: () => number, amount = 10): Color {
  return shade(c, (r() - 0.5) * amount);
}

/** (fx, fy) are face-local; return null to leave the pixel as it is. */
export type FacePainter = (fx: number, fy: number, f: Face) => Color | null;

export function paintFace(p: Pixels, f: Face, fn: FacePainter | Color) {
  for (let fy = 0; fy < f.h; fy++) {
    for (let fx = 0; fx < f.w; fx++) {
      const c = typeof fn === "function" ? fn(fx, fy, f) : fn;
      if (c) putPx(p, f.x + fx, f.y + fy, c);
    }
  }
}

export function paintBox(p: Pixels, b: Box, fn: FacePainter | Color, only?: FaceName[]) {
  for (const f of faces(b)) {
    if (!only || only.includes(f.name)) paintFace(p, f, fn);
  }
}

export interface SkinCanvas {
  p: Pixels;
  model: SkinModel;
  box: (id: string) => Box;
  face: (id: string, name: FaceName) => Face;
}

export function skinCanvas(model: SkinModel): SkinCanvas {
  const boxes = skinBoxes(model);
  const box = (id: string) => boxes.find((b) => b.id === id)!;
  return { p: blank(64, 64), model, box, face: (id, name) => face(box(id), name) };
}

/** Outfit description for {@link drawPerson}. */
export interface Outfit {
  skin: string;
  hair: string;
  hairStyle: "short" | "long" | "none";
  eyes: string;
  shirt: string;
  /** Sleeves reaching the hands; false leaves the forearms bare. */
  longSleeves: boolean;
  pants: string;
  shoes: string;
  seed: number;
}

/** A plain person; presets paint their details on top. */
export function drawPerson(c: SkinCanvas, o: Outfit) {
  const r = rng(o.seed);
  const skin = hex(o.skin);
  const hair = hex(o.hair);
  const shirt = hex(o.shirt);
  const pants = hex(o.pants);
  const shoes = hex(o.shoes);
  const { p } = c;

  // Head: skin, then hair.
  paintBox(p, c.box("head"), () => grain(skin, r, 6));
  if (o.hairStyle !== "none") {
    const long = o.hairStyle === "long";
    paintFace(p, c.face("head", "top"), () => grain(hair, r));
    paintFace(p, c.face("head", "back"), (_x, y) => (long || y < 6 ? grain(hair, r) : null));
    for (const side of ["right", "left"] as const) {
      paintFace(p, c.face("head", side), (x, y) =>
        y < 2 || (long && y < 7) || (y < 4 && x < 5) ? grain(hair, r) : null,
      );
    }
    paintFace(p, c.face("head", "front"), (x, y) =>
      y < 2 || (y < 3 && (x < 2 || x > 5)) || (long && (x === 0 || x === 7) && y < 7)
        ? grain(hair, r)
        : null,
    );
  }
  // Face details on the front.
  const front = c.face("head", "front");
  const eye = hex(o.eyes);
  const white = hex("#f4f4f4");
  putPx(p, front.x + 1, front.y + 4, white);
  putPx(p, front.x + 2, front.y + 4, eye);
  putPx(p, front.x + 5, front.y + 4, eye);
  putPx(p, front.x + 6, front.y + 4, white);
  putPx(p, front.x + 3, front.y + 6, shade(skin, -45));
  putPx(p, front.x + 4, front.y + 6, shade(skin, -45));
  putPx(p, front.x + 3, front.y + 5, shade(skin, -18));
  putPx(p, front.x + 4, front.y + 5, shade(skin, -18));

  // Torso.
  paintBox(p, c.box("body"), (_x, y, f) =>
    f.name === "bottom" ? shade(shirt, -25) : grain(shade(shirt, y > 10 ? -12 : 0), r, 8),
  );
  // Arms: sleeves then hands.
  for (const id of ["rArm", "lArm"]) {
    paintBox(p, c.box(id), (_x, y, f) => {
      if (f.name === "bottom") return shade(skin, -10);
      if (f.name === "top") return grain(shirt, r, 8);
      const sleeve = o.longSleeves ? y < 10 : y < 4;
      return sleeve ? grain(shirt, r, 8) : grain(skin, r, 6);
    });
  }
  // Legs: trousers then shoes.
  for (const id of ["rLeg", "lLeg"]) {
    paintBox(p, c.box(id), (_x, y, f) => {
      if (f.name === "bottom") return shade(shoes, -15);
      if (f.name === "top") return grain(pants, r, 8);
      return y >= 10 ? grain(shoes, r, 6) : grain(shade(pants, y > 7 ? -8 : 0), r, 8);
    });
  }
}

export interface CapeDesign {
  /** Outside of the cape (also the elytra wings), from its top-left. */
  front: (x: number, y: number, w: number, h: number) => Color;
  /** Inner lining and edges. */
  lining: Color;
}

/** Paints a 64×32 cape texture, reusing the design on the elytra wings. */
export function drawCape(d: CapeDesign): Pixels {
  const p = blank(64, 32);
  const [cape, elytra] = CAPE_BOXES as [Box, Box];
  paintBox(p, cape, d.lining);
  // The design area of a cape texture is the UV "front" face at (1, 1).
  paintFace(p, face(cape, "front"), (x, y, f) => d.front(x, y, f.w, f.h));
  paintBox(p, elytra, shade(d.lining, -10));
  for (const n of ["front", "back"] as const) {
    paintFace(p, face(elytra, n), (x, y, f) => d.front(x, y, f.w, f.h));
  }
  return p;
}
