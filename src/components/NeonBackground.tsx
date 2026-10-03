import { useEffect, useRef } from "react";

const GRID = 48;
const PARTICLES = 28;
const FRAME_MS = 1000 / 30; // 30 fps is plenty for a background.

/**
 * Slowly drifting grid + floating particles on one canvas. Pauses while the
 * window is hidden and renders a single static frame for reduced motion.
 */
export function NeonBackground({ accent }: { accent: string }) {
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = ref.current;
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx) return;

    // `accent` is only a re-run trigger; the color itself comes from CSS.
    void accent;
    const rgb = getComputedStyle(document.documentElement)
      .getPropertyValue("--mc-accent-rgb")
      .trim()
      .replace(/\s+/g, ",");
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    let w = 0;
    let h = 0;
    const resize = () => {
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      w = canvas.clientWidth;
      h = canvas.clientHeight;
      canvas.width = w * dpr;
      canvas.height = h * dpr;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    };
    resize();
    const ro = new ResizeObserver(resize);
    ro.observe(canvas);

    const particles = Array.from({ length: PARTICLES }, () => ({
      x: Math.random(),
      y: Math.random(),
      r: 0.6 + Math.random() * 1.4,
      v: 0.00025 + Math.random() * 0.0006,
      a: 0.15 + Math.random() * 0.35,
    }));

    let offset = 0;
    const draw = () => {
      ctx.clearRect(0, 0, w, h);
      ctx.lineWidth = 1;
      ctx.strokeStyle = `rgba(${rgb},0.05)`;
      ctx.beginPath();
      for (let x = -GRID + (offset % GRID); x < w; x += GRID) {
        ctx.moveTo(x + 0.5, 0);
        ctx.lineTo(x + 0.5, h);
      }
      for (let y = -GRID + (offset % GRID); y < h; y += GRID) {
        ctx.moveTo(0, y + 0.5);
        ctx.lineTo(w, y + 0.5);
      }
      ctx.stroke();

      for (const p of particles) {
        ctx.fillStyle = `rgba(${rgb},${p.a})`;
        ctx.beginPath();
        ctx.arc(p.x * w, p.y * h, p.r, 0, Math.PI * 2);
        ctx.fill();
      }
    };

    let raf = 0;
    let last = 0;
    const tick = (now: number) => {
      raf = requestAnimationFrame(tick);
      if (document.hidden || now - last < FRAME_MS) return;
      last = now;
      offset += 0.15;
      for (const p of particles) {
        p.y -= p.v;
        if (p.y < -0.02) {
          p.y = 1.02;
          p.x = Math.random();
        }
      }
      draw();
    };

    if (reduced) draw();
    else raf = requestAnimationFrame(tick);

    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
    };
  }, [accent]);

  return (
    <canvas
      ref={ref}
      aria-hidden="true"
      className="pointer-events-none absolute inset-0 h-full w-full"
    />
  );
}
