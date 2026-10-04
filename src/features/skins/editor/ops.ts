// Pure pixel operations for the skin editor (no DOM), so they stay testable.

import { type Box, type Face, locate, mirrorPixel } from "./layout";

export type Rgba = [number, number, number, number];

export interface Pixels {
  width: number;
  height: number;
  /** Row-major RGBA8, like ImageData.data. */
  data: Uint8ClampedArray;
}

export const TRANSPARENT: Rgba = [0, 0, 0, 0];

export function blank(width: number, height: number): Pixels {
  return { width, height, data: new Uint8ClampedArray(width * height * 4) };
}

export function clone(p: Pixels): Pixels {
  return { width: p.width, height: p.height, data: new Uint8ClampedArray(p.data) };
}

export function inside(p: Pixels, x: number, y: number): boolean {
  return x >= 0 && y >= 0 && x < p.width && y < p.height;
}

export function getPx(p: Pixels, x: number, y: number): Rgba {
  const i = (y * p.width + x) * 4;
  const d = p.data;
  return [d[i] ?? 0, d[i + 1] ?? 0, d[i + 2] ?? 0, d[i + 3] ?? 0];
}

export function putPx(p: Pixels, x: number, y: number, c: Rgba): void {
  if (!inside(p, x, y)) return;
  const i = (y * p.width + x) * 4;
  p.data[i] = c[0];
  p.data[i + 1] = c[1];
  p.data[i + 2] = c[2];
  p.data[i + 3] = c[3];
}

export function sameColor(a: Rgba, b: Rgba): boolean {
  // All fully transparent pixels count as the same colour.
  if (a[3] === 0 && b[3] === 0) return true;
  return a[0] === b[0] && a[1] === b[1] && a[2] === b[2] && a[3] === b[3];
}

export function hexToRgba(hex: string, alpha = 255): Rgba {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return [0, 0, 0, alpha];
  const n = parseInt(m[1] ?? "0", 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255, alpha];
}

export function rgbaToHex(c: Rgba): string {
  return `#${((1 << 24) | (c[0] << 16) | (c[1] << 8) | c[2]).toString(16).slice(1)}`;
}

/** What a stroke is allowed to touch. */
export interface PaintScope {
  boxes: Box[];
  /** Only pixels of this layer's boxes are painted; null = anywhere. */
  layer: "base" | "overlay" | null;
  mirror: boolean;
}

function allowed(scope: PaintScope, x: number, y: number): boolean {
  if (!scope.layer) return true;
  const f = locate(scope.boxes, x, y);
  return f !== null && f.box.layer === scope.layer;
}

/** Paints a size×size square brush at (x, y), plus its mirror image. */
export function stamp(p: Pixels, x: number, y: number, size: number, c: Rgba, scope: PaintScope) {
  const off = Math.floor((size - 1) / 2);
  for (let dy = 0; dy < size; dy++) {
    for (let dx = 0; dx < size; dx++) {
      const px = x - off + dx;
      const py = y - off + dy;
      if (!inside(p, px, py) || !allowed(scope, px, py)) continue;
      putPx(p, px, py, c);
      if (scope.mirror) {
        const m = mirrorPixel(scope.boxes, px, py);
        if (m) putPx(p, m[0], m[1], c);
      }
    }
  }
}

/** Integer points from a to b inclusive (Bresenham). */
export function linePoints(x0: number, y0: number, x1: number, y1: number): [number, number][] {
  const pts: [number, number][] = [];
  const dx = Math.abs(x1 - x0);
  const dy = -Math.abs(y1 - y0);
  const sx = x0 < x1 ? 1 : -1;
  const sy = y0 < y1 ? 1 : -1;
  let err = dx + dy;
  let x = x0;
  let y = y0;
  for (;;) {
    pts.push([x, y]);
    if (x === x1 && y === y1) break;
    const e2 = 2 * err;
    if (e2 >= dy) {
      err += dy;
      x += sx;
    }
    if (e2 <= dx) {
      err += dx;
      y += sy;
    }
  }
  return pts;
}

/**
 * Fills the 4-connected area of the start pixel's colour. On a mapped pixel
 * the fill stays inside that face, so it never bleeds into a neighbouring
 * body part. Returns the number of pixels changed.
 */
export function floodFill(p: Pixels, x: number, y: number, c: Rgba, scope: PaintScope): number {
  if (!inside(p, x, y) || !allowed(scope, x, y)) return 0;
  const target = getPx(p, x, y);
  if (sameColor(target, c)) return 0;
  const f: Face | null = scope.boxes.length ? locate(scope.boxes, x, y) : null;
  const inBounds = (px: number, py: number) =>
    f ? px >= f.x && px < f.x + f.w && py >= f.y && py < f.y + f.h : inside(p, px, py);
  const seen = new Uint8Array(p.width * p.height);
  const stack: [number, number][] = [[x, y]];
  let n = 0;
  while (stack.length) {
    const [cx, cy] = stack.pop()!;
    if (!inBounds(cx, cy)) continue;
    const k = cy * p.width + cx;
    if (seen[k] || !sameColor(getPx(p, cx, cy), target)) continue;
    seen[k] = 1;
    putPx(p, cx, cy, c);
    n++;
    if (scope.mirror) {
      const m = mirrorPixel(scope.boxes, cx, cy);
      if (m) putPx(p, m[0], m[1], c);
    }
    stack.push([cx + 1, cy], [cx - 1, cy], [cx, cy + 1], [cx, cy - 1]);
  }
  return n;
}

/** Clears every pixel of the given layer (or everything). */
export function clearLayer(p: Pixels, boxes: Box[], layer: "base" | "overlay" | null) {
  for (let y = 0; y < p.height; y++) {
    for (let x = 0; x < p.width; x++) {
      if (allowed({ boxes, layer, mirror: false }, x, y)) putPx(p, x, y, TRANSPARENT);
    }
  }
}

/** Bounded undo/redo of whole-texture snapshots (a texture is 16 KiB). */
export class History {
  private past: Uint8ClampedArray[] = [];
  private future: Uint8ClampedArray[] = [];

  constructor(private readonly limit = 100) {}

  /** Call before a change with the state being replaced. */
  push(p: Pixels) {
    this.past.push(new Uint8ClampedArray(p.data));
    if (this.past.length > this.limit) this.past.shift();
    this.future = [];
  }

  undo(p: Pixels): boolean {
    const prev = this.past.pop();
    if (!prev) return false;
    this.future.push(new Uint8ClampedArray(p.data));
    p.data.set(prev);
    return true;
  }

  redo(p: Pixels): boolean {
    const next = this.future.pop();
    if (!next) return false;
    this.past.push(new Uint8ClampedArray(p.data));
    p.data.set(next);
    return true;
  }

  get canUndo() {
    return this.past.length > 0;
  }

  get canRedo() {
    return this.future.length > 0;
  }

  reset() {
    this.past = [];
    this.future = [];
  }
}
