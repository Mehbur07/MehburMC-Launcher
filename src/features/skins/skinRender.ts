import type { SkinModel } from "../../lib/ipc/bindings/SkinModel";

/** One rectangle copied from the skin texture onto a 16×32 front view. */
export interface Part {
  sx: number;
  sy: number;
  w: number;
  h: number;
  dx: number;
  dy: number;
  /** Legacy 64×32 skins mirror the right limbs for the left ones. */
  flip?: boolean;
}

/** Front-facing parts in 64px texture units, base layer first, overlays last. */
export function frontParts(model: SkinModel, legacy: boolean): Part[] {
  const aw = model === "slim" ? 3 : 4;
  const base: Part[] = [
    { sx: 8, sy: 8, w: 8, h: 8, dx: 4, dy: 0 }, // head
    { sx: 20, sy: 20, w: 8, h: 12, dx: 4, dy: 8 }, // body
    { sx: 44, sy: 20, w: aw, h: 12, dx: 4 - aw, dy: 8 }, // right arm
    { sx: 4, sy: 20, w: 4, h: 12, dx: 4, dy: 20 }, // right leg
  ];
  if (legacy) {
    return [
      ...base,
      { sx: 44, sy: 20, w: aw, h: 12, dx: 12, dy: 8, flip: true },
      { sx: 4, sy: 20, w: 4, h: 12, dx: 8, dy: 20, flip: true },
      { sx: 40, sy: 8, w: 8, h: 8, dx: 4, dy: 0 }, // hat
    ];
  }
  return [
    ...base,
    { sx: 36, sy: 52, w: aw, h: 12, dx: 12, dy: 8 }, // left arm
    { sx: 20, sy: 52, w: 4, h: 12, dx: 8, dy: 20 }, // left leg
    { sx: 40, sy: 8, w: 8, h: 8, dx: 4, dy: 0 }, // hat
    { sx: 20, sy: 36, w: 8, h: 12, dx: 4, dy: 8 }, // jacket
    { sx: 44, sy: 36, w: aw, h: 12, dx: 4 - aw, dy: 8 }, // right sleeve
    { sx: 52, sy: 52, w: aw, h: 12, dx: 12, dy: 8 }, // left sleeve
    { sx: 4, sy: 36, w: 4, h: 12, dx: 4, dy: 20 }, // right trouser
    { sx: 4, sy: 52, w: 4, h: 12, dx: 8, dy: 20 }, // left trouser
  ];
}

/** Face plus hat layer, filling an 8×8 grid. */
export const HEAD_PARTS: Part[] = [
  { sx: 8, sy: 8, w: 8, h: 8, dx: 0, dy: 0 },
  { sx: 40, sy: 8, w: 8, h: 8, dx: 0, dy: 0 },
];

/** Draws `parts` with crisp pixels; `unit` = canvas pixels per skin pixel. */
export function drawParts(
  ctx: CanvasRenderingContext2D,
  img: HTMLImageElement,
  parts: Part[],
  unit: number,
) {
  // HD skins: texture pixels per 64px-unit.
  const s = img.naturalWidth / 64;
  ctx.imageSmoothingEnabled = false;
  for (const p of parts) {
    const [dx, dy, dw, dh] = [p.dx * unit, p.dy * unit, p.w * unit, p.h * unit];
    if (p.flip) {
      ctx.save();
      ctx.translate(dx + dw, dy);
      ctx.scale(-1, 1);
      ctx.drawImage(img, p.sx * s, p.sy * s, p.w * s, p.h * s, 0, 0, dw, dh);
      ctx.restore();
    } else {
      ctx.drawImage(img, p.sx * s, p.sy * s, p.w * s, p.h * s, dx, dy, dw, dh);
    }
  }
}

const images = new Map<string, Promise<HTMLImageElement>>();

/** Decodes a data URI once and reuses it for every thumbnail. */
export function loadImage(src: string): Promise<HTMLImageElement> {
  let p = images.get(src);
  if (!p) {
    p = new Promise((resolve, reject) => {
      const img = new Image();
      img.onload = () => resolve(img);
      img.onerror = () => reject(new Error("image decode failed"));
      img.src = src;
    });
    images.set(src, p);
    if (images.size > 200) images.delete(images.keys().next().value!);
  }
  return p;
}
