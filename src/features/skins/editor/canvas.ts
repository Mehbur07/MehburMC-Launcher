// DOM glue between Pixels and canvases / PNG data URIs.

import { type Pixels, blank, getPx, putPx } from "./ops";
import { mirrorPixel, skinBoxes } from "./layout";

export function toCanvas(p: Pixels, canvas = document.createElement("canvas")): HTMLCanvasElement {
  canvas.width = p.width;
  canvas.height = p.height;
  const ctx = canvas.getContext("2d");
  if (ctx) ctx.putImageData(new ImageData(new Uint8ClampedArray(p.data), p.width, p.height), 0, 0);
  return canvas;
}

export function toDataUri(p: Pixels): string {
  return toCanvas(p).toDataURL("image/png");
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("image"));
    img.src = src;
  });
}

/**
 * Decodes a texture into editor pixels: HD textures are scaled down to
 * 64 px wide, legacy 64×32 skins are upgraded to 64×64 (left limbs mirrored
 * from the right ones, like the game does).
 */
export async function fromDataUri(src: string, kind: "skin" | "cape"): Promise<Pixels> {
  const img = await loadImage(src);
  const scale = img.width / 64;
  const h = Math.round(img.height / scale);
  const c = document.createElement("canvas");
  c.width = 64;
  c.height = h;
  const ctx = c.getContext("2d");
  if (!ctx) return blank(64, kind === "skin" ? 64 : 32);
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(img, 0, 0, 64, h);
  const raw: Pixels = { width: 64, height: h, data: ctx.getImageData(0, 0, 64, h).data };
  if (kind === "cape") {
    const out = blank(64, 32);
    for (let y = 0; y < Math.min(32, h); y++)
      for (let x = 0; x < 64; x++) putPx(out, x, y, getPx(raw, x, y));
    return out;
  }
  return h === 32 ? upgradeLegacy(raw) : raw;
}

export function upgradeLegacy(legacy: Pixels): Pixels {
  const out = blank(64, 64);
  for (let y = 0; y < 32; y++) for (let x = 0; x < 64; x++) putPx(out, x, y, getPx(legacy, x, y));
  const boxes = skinBoxes("classic").filter((b) => b.id === "rArm" || b.id === "rLeg");
  const all = skinBoxes("classic");
  for (const b of boxes) {
    for (let y = b.v; y < b.v + b.d + b.h; y++) {
      for (let x = b.u; x < b.u + 2 * (b.d + b.w); x++) {
        const m = mirrorPixel(all, x, y);
        if (m) putPx(out, m[0], m[1], getPx(legacy, x, y));
      }
    }
  }
  return out;
}
