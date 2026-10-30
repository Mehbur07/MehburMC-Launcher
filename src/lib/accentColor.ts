/**
 * A user-picked accent color (K94). The presets live in tokens.css; a custom
 * color overrides the same variables on <html>, derived from one hex value.
 */
type Rgb = [number, number, number];

/** h 0–360, s and v 0–1. */
export type Hsv = { h: number; s: number; v: number };

const WHITE: Rgb = [255, 255, 255];
const BLACK: Rgb = [0, 0, 0];
/** `--mc-bg`: accent text has to stay readable on it. */
const BACKGROUND: Rgb = [5, 7, 10];
const MIN_CONTRAST = 4.5;

const VARIABLES = [
  "--mc-accent",
  "--mc-accent-2",
  "--mc-accent-glow",
  "--mc-accent-rgb",
  "--mc-on-accent",
];

/** `#rrggbb` in lower case, or null. The `#` is optional. */
export function parseHex(raw: string): string | null {
  const digits = /^#?([0-9a-f]{6})$/i.exec(raw.trim())?.[1];
  return digits ? `#${digits.toLowerCase()}` : null;
}

function toRgb(hex: string): Rgb {
  const n = Number.parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function toHex(rgb: Rgb): string {
  return `#${rgb.map((c) => c.toString(16).padStart(2, "0")).join("")}`;
}

function mix(a: Rgb, b: Rgb, t: number): Rgb {
  const at = (i: 0 | 1 | 2) => Math.round(a[i] + (b[i] - a[i]) * t);
  return [at(0), at(1), at(2)];
}

function luminance(rgb: Rgb): number {
  const linear = (c: number) => {
    const x = c / 255;
    return x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * linear(rgb[0]) + 0.7152 * linear(rgb[1]) + 0.0722 * linear(rgb[2]);
}

function contrast(a: Rgb, b: Rgb): number {
  const [x, y] = [luminance(a), luminance(b)];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}

/** The color itself, or a lighter one if it would vanish on the dark background. */
export function readableAccent(hex: string): string {
  const rgb = toRgb(hex);
  for (let t = 0; t < 1; t += 0.02) {
    const c = mix(rgb, WHITE, t);
    if (contrast(c, BACKGROUND) >= MIN_CONTRAST) return t === 0 ? hex : toHex(c);
  }
  return toHex(WHITE);
}

/** The accent variables for a custom color. */
export function accentTokens(hex: string): Record<string, string> {
  const base = toRgb(readableAccent(hex));
  const dark = mix(base, BLACK, 0.9);
  return {
    "--mc-accent": toHex(base),
    "--mc-accent-2": toHex(mix(base, BLACK, 0.25)),
    "--mc-accent-glow": toHex(mix(base, WHITE, 0.5)),
    "--mc-accent-rgb": base.join(" "),
    "--mc-on-accent": contrast(base, dark) >= contrast(base, WHITE) ? toHex(dark) : "#ffffff",
  };
}

/** Sets the custom color on <html>; without one the tokens.css preset shows. */
export function applyAccent(hex: string | undefined) {
  const { style } = document.documentElement;
  if (!hex) {
    VARIABLES.forEach((v) => style.removeProperty(v));
    return;
  }
  Object.entries(accentTokens(hex)).forEach(([k, v]) => style.setProperty(k, v));
}

export function hexToHsv(hex: string): Hsv {
  const rgb = toRgb(hex);
  const [r, g, b] = [rgb[0] / 255, rgb[1] / 255, rgb[2] / 255];
  const max = Math.max(r, g, b);
  const d = max - Math.min(r, g, b);
  let h = 0;
  if (d > 0) {
    if (max === r) h = ((g - b) / d + 6) % 6;
    else if (max === g) h = (b - r) / d + 2;
    else h = (r - g) / d + 4;
  }
  return { h: h * 60, s: max === 0 ? 0 : d / max, v: max };
}

export function hsvToHex({ h, s, v }: Hsv): string {
  const f = (n: number) => {
    const k = (n + h / 60) % 6;
    return (v - v * s * Math.max(0, Math.min(k, 4 - k, 1))) * 255;
  };
  return toHex([Math.round(f(5)), Math.round(f(3)), Math.round(f(1))]);
}
