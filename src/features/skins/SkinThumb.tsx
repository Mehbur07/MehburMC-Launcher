import { UserRound } from "lucide-react";
import { useEffect, useRef } from "react";

import type { SkinModel } from "../../lib/ipc/bindings/SkinModel";
import { drawParts, frontParts, HEAD_PARTS, loadImage } from "./skinRender";

/** 2D render of a skin: the whole front view, or just the head. */
export function SkinThumb({
  src,
  model = "classic",
  variant = "body",
  unit = 4,
  className = "",
}: {
  src: string | null;
  model?: SkinModel;
  variant?: "body" | "head";
  /** Canvas pixels per skin pixel. */
  unit?: number;
  className?: string;
}) {
  const ref = useRef<HTMLCanvasElement>(null);
  const [w, h] = variant === "head" ? [8, 8] : [16, 32];

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas || !src) return;
    let alive = true;
    void loadImage(src)
      .then((img) => {
        const ctx = canvas.getContext("2d");
        if (!alive || !ctx) return;
        ctx.clearRect(0, 0, canvas.width, canvas.height);
        const legacy = img.naturalHeight * 2 === img.naturalWidth;
        drawParts(ctx, img, variant === "head" ? HEAD_PARTS : frontParts(model, legacy), unit);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [src, model, variant, unit]);

  if (!src) {
    return (
      <div className={`grid place-items-center ${className}`}>
        <UserRound size={variant === "head" ? 18 : 32} className="text-accent" />
      </div>
    );
  }
  return (
    <canvas
      ref={ref}
      width={w * unit}
      height={h * unit}
      className={`[image-rendering:pixelated] ${className}`}
      aria-hidden
    />
  );
}

/** Back face of a cape (the part seen behind the player), pixel-scaled. */
export function CapeThumb({
  src,
  unit = 7,
  className = "",
}: {
  src: string;
  unit?: number;
  className?: string;
}) {
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    let alive = true;
    void loadImage(src)
      .then((img) => {
        const ctx = canvas.getContext("2d");
        if (!alive || !ctx) return;
        // 64×32 layout (and HD multiples); old 22×17 capes use the same offsets.
        const s =
          img.naturalWidth % 22 === 0 && img.naturalWidth * 17 === img.naturalHeight * 22
            ? img.naturalWidth / 22
            : img.naturalWidth / 64;
        ctx.imageSmoothingEnabled = false;
        ctx.clearRect(0, 0, canvas.width, canvas.height);
        ctx.drawImage(img, s, s, 10 * s, 16 * s, 0, 0, 10 * unit, 16 * unit);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [src, unit]);

  return (
    <canvas
      ref={ref}
      width={10 * unit}
      height={16 * unit}
      className={`[image-rendering:pixelated] ${className}`}
      aria-hidden
    />
  );
}
