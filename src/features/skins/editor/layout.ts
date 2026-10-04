// UV layout of the 64×64 skin and 64×32 cape textures, shared by the editor
// (guides, mirror, region fill) and the code-drawn presets.

import type { SkinModel } from "../../../lib/ipc/bindings/SkinModel";

export type Layer = "base" | "overlay";
export type FaceName = "top" | "bottom" | "right" | "front" | "left" | "back";

/** A cuboid unwrapped Minecraft-style from (u, v); w × h × d in pixels. */
export interface Box {
  id: string;
  /** Translation key under `skins.editor.part`. */
  part: string;
  layer: Layer;
  u: number;
  v: number;
  w: number;
  h: number;
  d: number;
  /** The box mirror-painting maps to (itself for head/body). */
  pair: string;
}

export interface Face {
  box: Box;
  name: FaceName;
  x: number;
  y: number;
  w: number;
  h: number;
}

export function faces(b: Box): Face[] {
  const { u, v, w, h, d } = b;
  return [
    { box: b, name: "top", x: u + d, y: v, w, h: d },
    { box: b, name: "bottom", x: u + d + w, y: v, w, h: d },
    { box: b, name: "right", x: u, y: v + d, w: d, h },
    { box: b, name: "front", x: u + d, y: v + d, w, h },
    { box: b, name: "left", x: u + d + w, y: v + d, w: d, h },
    { box: b, name: "back", x: u + d + w + d, y: v + d, w, h },
  ];
}

export function face(b: Box, name: FaceName): Face {
  return faces(b).find((f) => f.name === name)!;
}

/** All boxes of a 64×64 skin; arms are 3 px wide on slim skins. */
export function skinBoxes(model: SkinModel): Box[] {
  const a = model === "slim" ? 3 : 4;
  const box = (
    id: string,
    part: string,
    layer: Layer,
    u: number,
    v: number,
    w: number,
    h: number,
    d: number,
    pair = id,
  ): Box => ({ id, part, layer, u, v, w, h, d, pair });
  return [
    box("head", "head", "base", 0, 0, 8, 8, 8),
    box("hat", "head", "overlay", 32, 0, 8, 8, 8),
    box("body", "body", "base", 16, 16, 8, 12, 4),
    box("jacket", "body", "overlay", 16, 32, 8, 12, 4),
    box("rArm", "rightArm", "base", 40, 16, a, 12, 4, "lArm"),
    box("rSleeve", "rightArm", "overlay", 40, 32, a, 12, 4, "lSleeve"),
    box("lArm", "leftArm", "base", 32, 48, a, 12, 4, "rArm"),
    box("lSleeve", "leftArm", "overlay", 48, 48, a, 12, 4, "rSleeve"),
    box("rLeg", "rightLeg", "base", 0, 16, 4, 12, 4, "lLeg"),
    box("rPants", "rightLeg", "overlay", 0, 32, 4, 12, 4, "lPants"),
    box("lLeg", "leftLeg", "base", 16, 48, 4, 12, 4, "rLeg"),
    box("lPants", "leftLeg", "overlay", 0, 48, 4, 12, 4, "rPants"),
  ];
}

/** The cape (10×16×1) and the elytra wing (10×20×2) of a 64×32 cape texture. */
export const CAPE_BOXES: Box[] = [
  { id: "cape", part: "cape", layer: "base", u: 0, v: 0, w: 10, h: 16, d: 1, pair: "cape" },
  { id: "elytra", part: "elytra", layer: "base", u: 22, v: 0, w: 10, h: 20, d: 2, pair: "elytra" },
];

export const SKIN_SIZE = { w: 64, h: 64 } as const;
export const CAPE_SIZE = { w: 64, h: 32 } as const;

export function boxesFor(kind: "skin" | "cape", model: SkinModel): Box[] {
  return kind === "skin" ? skinBoxes(model) : CAPE_BOXES;
}

/** The face containing (x, y), if the pixel is part of the texture map. */
export function locate(boxes: Box[], x: number, y: number): Face | null {
  for (const b of boxes) {
    for (const f of faces(b)) {
      if (x >= f.x && x < f.x + f.w && y >= f.y && y < f.y + f.h) return f;
    }
  }
  return null;
}

const SWAP: Partial<Record<FaceName, FaceName>> = { right: "left", left: "right" };

/**
 * The pixel that mirror painting sets alongside (x, y): the same spot on the
 * other limb (or the other half of the head/body), flipped left↔right.
 */
export function mirrorPixel(boxes: Box[], x: number, y: number): [number, number] | null {
  const f = locate(boxes, x, y);
  if (!f) return null;
  const target = boxes.find((b) => b.id === f.box.pair);
  if (!target) return null;
  const tf = face(target, SWAP[f.name] ?? f.name);
  const fx = x - f.x;
  const fy = y - f.y;
  // Faces of paired limbs can differ in width only on mismatched models.
  if (fx >= tf.w || fy >= tf.h) return null;
  return [tf.x + (tf.w - 1 - fx), tf.y + fy];
}
