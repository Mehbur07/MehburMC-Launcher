import { afterEach, describe, expect, it } from "vitest";

import {
  accentTokens,
  applyAccent,
  hexToHsv,
  hsvToHex,
  parseHex,
  readableAccent,
} from "./accentColor";

afterEach(() => applyAccent(undefined));

/** WCAG contrast of two #rrggbb colors, written out independently of the module. */
function ratio(a: string, b: string) {
  const lum = (hex: string) => {
    const part = (i: number) => {
      const x = Number.parseInt(hex.slice(i, i + 2), 16) / 255;
      return x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
    };
    return 0.2126 * part(1) + 0.7152 * part(3) + 0.0722 * part(5);
  };
  const [x, y] = [lum(a), lum(b)];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}

describe("accent color", () => {
  it("parses six-digit hex only", () => {
    expect(parseHex("#FF8800")).toBe("#ff8800");
    expect(parseHex(" ff8800 ")).toBe("#ff8800");
    for (const bad of ["", "#fff", "#ff880", "#ff88000", "#ff88zz", "url(x)"]) {
      expect(parseHex(bad)).toBeNull();
    }
  });

  it("converts between hex and hsv", () => {
    for (const hex of [
      "#ff0000",
      "#00ff00",
      "#0000ff",
      "#ff8800",
      "#123456",
      "#000000",
      "#ffffff",
    ]) {
      expect(hsvToHex(hexToHsv(hex))).toBe(hex);
    }
    expect(hexToHsv("#ff0000")).toEqual({ h: 0, s: 1, v: 1 });
    expect(hexToHsv("#000000").v).toBe(0);
  });

  it("keeps bright colors and lightens ones that would vanish", () => {
    expect(readableAccent("#ff8800")).toBe("#ff8800");
    expect(readableAccent("#00f0ff")).toBe("#00f0ff");
    for (const dark of ["#000000", "#0000ff", "#101010", "#330000", "#05070a"]) {
      const out = readableAccent(dark);
      expect(out).not.toBe(dark);
      expect(ratio(out, "#05070a")).toBeGreaterThanOrEqual(4.5);
    }
  });

  it("derives tokens with readable text on the accent", () => {
    for (const hex of [
      "#000000",
      "#0000ff",
      "#ff0000",
      "#ff8800",
      "#ffff00",
      "#ffffff",
      "#808080",
    ]) {
      const t = accentTokens(hex);
      expect(ratio(t["--mc-accent"] ?? "", t["--mc-on-accent"] ?? "")).toBeGreaterThanOrEqual(4);
      expect(t["--mc-accent-rgb"]).toMatch(/^\d+ \d+ \d+$/);
    }
    expect(accentTokens("#ff8800")["--mc-accent-rgb"]).toBe("255 136 0");
  });

  it("sets the variables on <html> and clears them for a preset", () => {
    const { style } = document.documentElement;
    applyAccent("#ff8800");
    expect(style.getPropertyValue("--mc-accent")).toBe("#ff8800");
    applyAccent(undefined);
    expect(style.getPropertyValue("--mc-accent")).toBe("");
    expect(style.getPropertyValue("--mc-accent-rgb")).toBe("");
  });
});
