// Big MehburMC catalogue (K77): skins built from a kit of reusable parts and
// capes from pattern families plus pixel emblems. All drawn in code (no
// third-party art); each entry picks its own colours and parts.

import type { SkinModel } from "../../../lib/ipc/bindings/SkinModel";
import { putPx } from "../editor/ops";
import type { CapePreset, SkinPreset } from "./index";
import {
  ARMS,
  LEGS,
  SIDES,
  around,
  bareArms,
  bareLegs,
  cap,
  hood,
  noise,
  number,
  person,
  plaid,
  stamp,
} from "./library";
import {
  type Color,
  type Outfit,
  type SkinCanvas,
  drawCape,
  grain,
  hex,
  mix,
  paintBox,
  paintFace,
  rng,
  shade,
} from "./paint";

// ------------------------------------------------------------ skin kit

/** One finishing touch on a drawn person. */
type Part = (c: SkinCanvas, r: () => number, o: Outfit) => void;

const px = (
  c: SkinCanvas,
  box: string,
  side: Parameters<SkinCanvas["face"]>[1],
  x: number,
  y: number,
  col: Color,
) => {
  const f = c.face(box, side);
  putPx(c.p, f.x + x, f.y + y, col);
};

/** Paints a pixel-art pattern onto a face; '.' is left alone. */
function art(
  c: SkinCanvas,
  box: string,
  side: Parameters<SkinCanvas["face"]>[1],
  rows: string[],
  colors: Record<string, string>,
  x0: number,
  y0: number,
) {
  rows.forEach((row, dy) =>
    [...row].forEach((ch, dx) => {
      const col = colors[ch];
      if (col) px(c, box, side, x0 + dx, y0 + dy, hex(col));
    }),
  );
}

let seedCounter = 1000;
function O(
  skin: string,
  hair: string,
  hairStyle: Outfit["hairStyle"],
  eyes: string,
  shirt: string,
  longSleeves: boolean,
  pants: string,
  shoes: string,
): Outfit {
  return { skin, hair, hairStyle, eyes, shirt, longSleeves, pants, shoes, seed: seedCounter++ };
}

function sk(id: string, model: SkinModel, o: Outfit, ...parts: Part[]): SkinPreset {
  return {
    id,
    model,
    draw: person(model, o, (c, r) => {
      for (const p of parts) p(c, r, o);
    }),
  };
}

// --- head
const capHat =
  (col: string, rows: number, brim?: string): Part =>
  (c, r) =>
    cap(c, hex(col), rows, r, brim ? hex(brim) : undefined);
const beanie =
  (col: string, fold: string, pom?: string): Part =>
  (c, r, o) => {
    capHat(col, 3, fold)(c, r, o);
    if (pom)
      for (const [x, y] of [
        [3, 3],
        [4, 3],
        [3, 4],
        [4, 4],
      ] as const)
        px(c, "hat", "top", x, y, hex(pom));
  };
const headband =
  (col: string, row = 2): Part =>
  (c) => {
    for (const s of SIDES)
      paintFace(c.p, c.face("hat", s), (_x, y) => (y === row ? hex(col) : null));
  };
const bandana =
  (col: string, dot?: string): Part =>
  (c, r) => {
    paintFace(c.p, c.face("hat", "top"), (x, y) =>
      dot && (x + y) % 3 === 0 ? hex(dot) : grain(hex(col), r, 6),
    );
    for (const s of SIDES) {
      paintFace(c.p, c.face("hat", s), (x, y) =>
        y < 2 ? (dot && (x + y) % 3 === 0 ? hex(dot) : grain(hex(col), r, 6)) : null,
      );
    }
    for (const [x, y] of [
      [3, 2],
      [4, 2],
      [3, 3],
      [4, 4],
    ] as const)
      px(c, "hat", "back", x, y, hex(col));
  };
const crown =
  (gold: string, gem: string): Part =>
  (c) => {
    for (const s of SIDES) {
      paintFace(c.p, c.face("hat", s), (x, y) =>
        y === 1 || (y === 0 && x % 2 === 0) ? hex(gold) : null,
      );
    }
    px(c, "hat", "front", 3, 1, hex(gem));
    px(c, "hat", "front", 4, 1, hex(gem));
  };
const tiara =
  (gold: string, gem: string): Part =>
  (c) => {
    for (let x = 1; x < 7; x++) px(c, "hat", "front", x, 1, hex(gold));
    px(c, "hat", "front", 3, 0, hex(gem));
    px(c, "hat", "front", 4, 0, hex(gem));
  };
const topHat =
  (col: string, band: string): Part =>
  (c, r) => {
    cap(c, hex(col), 2, r, shade(hex(col), -25));
    for (const s of SIDES)
      paintFace(c.p, c.face("hat", s), (_x, y) => (y === 1 ? hex(band) : null));
  };
const witchHat =
  (col: string, band: string): Part =>
  (c, r) => {
    cap(c, hex(col), 3, r, shade(hex(col), -20));
    for (const s of SIDES)
      paintFace(c.p, c.face("hat", s), (_x, y) => (y === 2 ? hex(band) : null));
    for (const [x, y] of [
      [3, 3],
      [4, 3],
      [3, 4],
      [4, 4],
      [4, 2],
      [5, 1],
    ] as const) {
      px(c, "hat", "top", x, y, shade(hex(col), 25));
    }
  };
const halo =
  (col: string): Part =>
  (c) =>
    paintFace(c.p, c.face("hat", "top"), (x, y) =>
      (x === 1 || x === 6) && y > 0 && y < 7
        ? hex(col)
        : (y === 1 || y === 6) && x > 0 && x < 7
          ? hex(col)
          : null,
    );
const horns =
  (col: string): Part =>
  (c) => {
    for (const [x, y] of [
      [0, 0],
      [1, 0],
      [0, 1],
      [7, 0],
      [6, 0],
      [7, 1],
    ] as const)
      px(c, "hat", "front", x, y, hex(col));
    for (const [x, y] of [
      [0, 7],
      [7, 7],
      [0, 6],
      [7, 6],
    ] as const)
      px(c, "hat", "top", x, y, shade(hex(col), -20));
  };
const catEars =
  (col: string, inner: string): Part =>
  (c) => {
    for (const x of [0, 1, 6, 7]) px(c, "hat", "front", x, 0, hex(col));
    px(c, "hat", "front", 1, 0, hex(inner));
    px(c, "hat", "front", 6, 0, hex(inner));
    for (const [x, y] of [
      [0, 6],
      [1, 6],
      [0, 7],
      [6, 6],
      [7, 6],
      [7, 7],
    ] as const)
      px(c, "hat", "top", x, y, hex(col));
  };
const headphones =
  (band: string, cup: string): Part =>
  (c) => {
    for (const s of ["left", "right"] as const) {
      paintFace(c.p, c.face("hat", s), (x, y) =>
        x >= 2 && x <= 5 && y >= 3 && y <= 6
          ? hex(cup)
          : x >= 3 && x <= 4 && y < 3
            ? hex(band)
            : null,
      );
    }
    paintFace(c.p, c.face("hat", "top"), (x) => (x === 0 || x === 7 ? hex(band) : null));
    paintFace(c.p, c.face("hat", "top"), (_x, y) => (y === 3 || y === 4 ? hex(band) : null));
  };
const helmet =
  (col: string, visor: string): Part =>
  (c, r) => {
    cap(c, hex(col), 3, r);
    for (let x = 0; x < 8; x++)
      px(c, "hat", "front", x, 3, x === 0 || x === 7 ? hex(col) : hex(visor));
  };
const hoodP =
  (col: string): Part =>
  (c, r) =>
    hood(c, hex(col), r);
const mohawk =
  (col: string): Part =>
  (c) => {
    paintFace(c.p, c.face("hat", "top"), (x) => (x === 3 || x === 4 ? hex(col) : null));
    for (const [x, y] of [
      [3, 0],
      [4, 0],
    ] as const)
      px(c, "hat", "front", x, y, hex(col));
    paintFace(c.p, c.face("hat", "back"), (x, y) =>
      (x === 3 || x === 4) && y < 4 ? hex(col) : null,
    );
  };
const bun =
  (col: string): Part =>
  (c) => {
    for (const [x, y] of [
      [3, 3],
      [4, 3],
      [3, 4],
      [4, 4],
      [3, 5],
      [4, 5],
    ] as const)
      px(c, "hat", "top", x, y, hex(col));
    for (const [x, y] of [
      [3, 0],
      [4, 0],
      [2, 1],
      [5, 1],
    ] as const)
      px(c, "hat", "back", x, y, hex(col));
  };
const turban =
  (col: string, gem: string): Part =>
  (c, r) => {
    cap(c, hex(col), 3, r);
    for (const s of SIDES)
      paintFace(c.p, c.face("hat", s), (x, y) =>
        y < 3 && (x + y) % 3 === 0 ? shade(hex(col), -22) : null,
      );
    px(c, "hat", "front", 3, 1, hex(gem));
    px(c, "hat", "front", 4, 1, hex(gem));
  };

// --- face
const glasses =
  (col: string): Part =>
  (c) => {
    for (const x of [1, 2, 5, 6]) px(c, "hat", "front", x, 3, hex(col));
    for (const x of [0, 3, 4, 7]) px(c, "hat", "front", x, 4, hex(col));
  };
const shades =
  (frame: string, lens: string): Part =>
  (c) => {
    for (let x = 0; x < 8; x++)
      px(c, "hat", "front", x, 4, [1, 2, 5, 6].includes(x) ? hex(lens) : hex(frame));
    for (const x of [1, 2, 5, 6]) px(c, "hat", "front", x, 3, hex(frame));
  };
const beard =
  (col: string): Part =>
  (c, r) => {
    for (let y = 5; y < 8; y++) {
      for (let x = 0; x < 8; x++) {
        if (y === 6 && (x === 3 || x === 4)) continue;
        px(c, "head", "front", x, y, grain(hex(col), r, 10));
      }
    }
    for (const s of ["left", "right"] as const)
      paintFace(c.p, c.face("head", s), (_x, y) => (y >= 5 ? grain(hex(col), r, 10) : null));
  };
const mustache =
  (col: string): Part =>
  (c) => {
    for (let x = 1; x < 7; x++) px(c, "head", "front", x, 5, hex(col));
  };
const goatee =
  (col: string): Part =>
  (c) => {
    for (const [x, y] of [
      [3, 7],
      [4, 7],
      [2, 5],
      [5, 5],
    ] as const)
      px(c, "head", "front", x, y, hex(col));
  };
const maskLow =
  (col: string): Part =>
  (c) =>
    paintFace(c.p, c.face("hat", "front"), (_x, y) => (y >= 5 ? hex(col) : null));
const eyes =
  (col: string): Part =>
  (c) => {
    for (const x of [1, 2, 5, 6]) px(c, "head", "front", x, 4, hex(col));
  };
const lips =
  (col: string): Part =>
  (c) => {
    px(c, "head", "front", 3, 6, hex(col));
    px(c, "head", "front", 4, 6, hex(col));
  };
const faceArt =
  (rows: string[], colors: Record<string, string>, layer: "head" | "hat" = "head"): Part =>
  (c) =>
    art(c, layer, "front", rows, colors, 0, 0);

// --- body
const stripes =
  (col: string, every = 2, arms = true): Part =>
  (c) =>
    around(c, arms ? ["body", ...ARMS] : ["body"], (_x, y) =>
      y % every === 0 && y < 11 ? hex(col) : null,
    );
const tie =
  (col: string, collar = "#f4f4f4"): Part =>
  (c) => {
    for (let x = 2; x < 6; x++) px(c, "body", "front", x, 0, hex(collar));
    for (let y = 1; y < 8; y++) px(c, "body", "front", 3, y, hex(col));
    for (let y = 2; y < 8; y++) px(c, "body", "front", 4, y, hex(col));
  };
const bowtie =
  (col: string, shirt = "#f4f4f4"): Part =>
  (c) => {
    for (let y = 0; y < 12; y++)
      for (let x = 3; x < 5; x++) px(c, "body", "front", x, y, hex(shirt));
    for (const [x, y] of [
      [2, 1],
      [3, 1],
      [4, 1],
      [5, 1],
      [2, 0],
      [5, 2],
    ] as const)
      px(c, "body", "front", x, y, hex(col));
  };
const openCoat =
  (col: string, gap = [3, 4]): Part =>
  (c, r) => {
    paintFace(c.p, c.face("jacket", "front"), (x) =>
      gap.includes(x) ? null : grain(hex(col), r, 5),
    );
    for (const s of ["back", "left", "right"] as const)
      paintFace(c.p, c.face("jacket", s), () => grain(hex(col), r, 5));
    for (const id of ["rSleeve", "lSleeve"])
      paintBox(c.p, c.box(id), (_x, y) => (y < 10 ? grain(hex(col), r, 5) : null), [...SIDES]);
  };
const vest =
  (col: string): Part =>
  (c, r) => {
    paintFace(c.p, c.face("jacket", "front"), (x) =>
      x === 3 || x === 4 ? null : grain(hex(col), r, 6),
    );
    for (const s of ["back", "left", "right"] as const)
      paintFace(c.p, c.face("jacket", s), () => grain(hex(col), r, 6));
  };
const belt =
  (col: string, buckle?: string, row = 10): Part =>
  (c) => {
    around(c, ["body"], (_x, y) => (y === row ? hex(col) : null));
    if (buckle) {
      px(c, "body", "front", 3, row, hex(buckle));
      px(c, "body", "front", 4, row, hex(buckle));
    }
  };
const chest =
  (
    rows: string[],
    colors: Record<string, string>,
    y0 = 2,
    side: "front" | "back" = "front",
  ): Part =>
  (c) =>
    art(c, "body", side, rows, colors, Math.floor((8 - (rows[0]?.length ?? 0)) / 2), y0);
const suspenders =
  (col: string): Part =>
  (c) => {
    for (const s of ["front", "back"] as const)
      paintFace(c.p, c.face("body", s), (x, y) =>
        (x === 1 || x === 6) && y < 10 ? hex(col) : null,
      );
  };
const apron =
  (col: string, tie2?: string): Part =>
  (c, r) => {
    paintFace(c.p, c.face("jacket", "front"), (x, y) =>
      y >= 2 && x > 0 && x < 7 ? grain(hex(col), r, 5) : null,
    );
    for (const id of ["rPants", "lPants"])
      paintBox(c.p, c.box(id), (_x, y) => (y < 5 ? grain(hex(col), r, 5) : null), ["front"]);
    if (tie2) around(c, ["body"], (_x, y) => (y === 6 ? hex(tie2) : null));
  };
const scarf =
  (a: string, b: string): Part =>
  (c) =>
    around(c, ["jacket"], (x, y) => (y < 2 ? ((x + y) % 2 ? hex(a) : hex(b)) : null));
const armor =
  (col: string, ids = ["body", ...ARMS]): Part =>
  (c, r) => {
    for (const id of ids) {
      paintBox(c.p, c.box(id), (x, y, f) => {
        const edge = x === 0 || y === 0 || x === f.w - 1 || y === f.h - 1;
        return grain(shade(hex(col), edge ? -30 : y < 2 ? 15 : 0), r, 8);
      });
    }
  };
const sash =
  (col: string): Part =>
  (c) => {
    for (let i = 0; i < 12; i++) {
      const x = Math.min(7, Math.floor(i * 0.7));
      px(c, "body", "front", x, i, hex(col));
      px(c, "body", "front", Math.min(7, x + 1), i, hex(col));
    }
  };
const zipper =
  (col: string): Part =>
  (c) =>
    paintFace(c.p, c.face("body", "front"), (x) => (x === 4 ? hex(col) : null));
const buttons =
  (col: string, x = 3): Part =>
  (c) => {
    for (let y = 2; y < 10; y += 2) px(c, "body", "front", x, y, hex(col));
  };
const backpack =
  (col: string, strap?: string): Part =>
  (c, r) => {
    paintFace(c.p, c.face("jacket", "back"), (x, y) =>
      x > 0 && x < 7 && y > 0 && y < 10 ? grain(hex(col), r, 8) : null,
    );
    paintFace(c.p, c.face("body", "front"), (x, y) =>
      (x === 1 || x === 6) && y < 9 ? hex(strap ?? col) : null,
    );
  };
const cloak =
  (col: string, lining?: string): Part =>
  (c, r) => {
    paintFace(c.p, c.face("jacket", "back"), () => grain(hex(col), r, 6));
    for (const s of ["left", "right"] as const)
      paintFace(c.p, c.face("jacket", s), (x) => (x === 3 ? hex(lining ?? col) : null));
    for (const x of [0, 7]) for (let y = 0; y < 12; y++) px(c, "jacket", "front", x, y, hex(col));
  };
const chestBand =
  (col: string, row: number, h = 1): Part =>
  (c) =>
    around(c, ["body"], (_x, y) => (y >= row && y < row + h ? hex(col) : null));
const pattern =
  (fn: (x: number, y: number) => string | null, ids = ["body"]): Part =>
  (c) =>
    around(c, ids, (x, y) => {
      const v = fn(x, y);
      return v ? hex(v) : null;
    });
const shirtNumber =
  (n: string, col: string): Part =>
  (c) => {
    number(c, "body", "front", n, n.length === 1 ? 2 : 0, 3, hex(col));
    number(c, "body", "back", n, n.length === 1 ? 2 : 0, 3, hex(col));
  };

// --- arms and legs
const gloves =
  (col: string, from = 10): Part =>
  (c) => {
    around(c, ARMS, (_x, y) => (y >= from ? hex(col) : null));
    for (const id of ARMS) paintFace(c.p, c.face(id, "bottom"), hex(col));
  };
const cuffs =
  (col: string, row = 9): Part =>
  (c) =>
    around(c, ARMS, (_x, y) => (y === row ? hex(col) : null));
const bare =
  (from: number): Part =>
  (c, r, o) =>
    bareArms(c, hex(o.skin), from, r);
const shorts =
  (from = 6, to = 10): Part =>
  (c, r, o) =>
    bareLegs(c, hex(o.skin), from, to, r);
const skirt =
  (col: string, rows = 5, hem?: string): Part =>
  (c, r) => {
    for (const id of ["rPants", "lPants"]) {
      paintBox(
        c.p,
        c.box(id),
        (_x, y) => (y < rows ? (hem && y === rows - 1 ? hex(hem) : grain(hex(col), r, 6)) : null),
        [...SIDES],
      );
    }
  };
const boots =
  (col: string, from = 8, trim?: string): Part =>
  (c, r) =>
    around(c, LEGS, (_x, y) =>
      y >= from ? (trim && y === from ? hex(trim) : grain(hex(col), r, 6)) : null,
    );
const socks =
  (col: string, from = 8, to = 10): Part =>
  (c) =>
    around(c, LEGS, (_x, y) => (y >= from && y < to ? hex(col) : null));
const legStripe =
  (col: string): Part =>
  (c) => {
    paintFace(c.p, c.face("rLeg", "right"), (x, y) => (x === 1 && y < 10 ? hex(col) : null));
    paintFace(c.p, c.face("lLeg", "left"), (x, y) => (x === 2 && y < 10 ? hex(col) : null));
  };
const armStripe =
  (col: string): Part =>
  (c) => {
    paintFace(c.p, c.face("rArm", "right"), (x, y) => (x === 1 && y < 10 ? hex(col) : null));
    paintFace(c.p, c.face("lArm", "left"), (x, y) => (x === 2 && y < 10 ? hex(col) : null));
  };
const legsPattern =
  (fn: (x: number, y: number) => string | null): Part =>
  (c) =>
    around(c, LEGS, (x, y) => {
      const v = fn(x, y);
      return v ? hex(v) : null;
    });
const whole =
  (col: string, ids: string[]): Part =>
  (c, r) => {
    for (const id of ids) paintBox(c.p, c.box(id), () => grain(hex(col), r, 8));
  };

// Small chest emblems.
const HEART = [".#.#.", "#####", ".###.", "..#.."];
const STAR5 = ["..#..", "#####", ".###.", ".#.#."];
const BOLT5 = ["..##", ".##.", "####", ".##.", "##.."];
const SKULL5 = [".###.", "#.#.#", "#####", ".#.#."];
const MOON5 = [".##.", "#...", "#...", ".##."];
const NOTE5 = ["..##", "..#.", "..#.", "###.", "##.."];

export const CATALOG_SKINS: SkinPreset[] = [
  // ---------------------------------------------------------- professions
  sk(
    "pilot",
    "classic",
    O("#e0b08a", "#4a2c18", "short", "#3a5a8a", "#f2f2f2", true, "#2a3448", "#1a1a1a"),
    capHat("#2a3448", 2, "#141820"),
    tie("#2a3448"),
    openCoat("#2a3448"),
    cuffs("#d9b23a"),
    chest(["#.##.#", ".####."], { "#": "#d9b23a" }, 2),
  ),
  sk(
    "nurse",
    "slim",
    O("#f0c8a8", "#5a3420", "long", "#4a7aa8", "#7fc8d8", false, "#7fc8d8", "#f4f4f4"),
    headband("#f4f4f4", 1),
    chest([".#.", "###", ".#."], { "#": "#d83a3a" }, 2),
    belt("#5fb0c0"),
  ),
  sk(
    "officer",
    "classic",
    O("#c99070", "#1e1612", "short", "#2a3a5a", "#2c4a7a", true, "#1c2436", "#101010"),
    capHat("#1c2436", 2, "#0c0c0c"),
    chest(["##", "##"], { "#": "#e2c040" }, 2),
    belt("#141414", "#c0c0c0"),
    buttons("#c0c0c0"),
  ),
  sk(
    "mechanic",
    "classic",
    O("#d6a07a", "#3a2618", "short", "#4a6a3a", "#3e5a7a", true, "#3e5a7a", "#2a2a2a"),
    capHat("#c8402a", 2, "#8a2a1a"),
    pattern((x, y) => ((x + y * 3) % 11 === 0 ? "#1c1c1c" : null), ["body", "rLeg"]),
    gloves("#2a2a2a"),
    belt("#2a2a2a"),
    chest(["##"], { "#": "#e8e8e8" }, 3),
  ),
  sk(
    "painter",
    "classic",
    O("#eac0a0", "#7a4a2a", "short", "#3a6a5a", "#f2ece0", true, "#3a4a6a", "#4a3020"),
    capHat("#2a2a32", 1, "#1a1a20"),
    mustache("#5a3a20"),
    pattern(
      (x, y) =>
        (x * 5 + y * 7) % 13 === 0
          ? ["#e83a3a", "#3a8ae8", "#f2c230", "#3ab86a"][(x + y) % 4]!
          : null,
      ["body", "rArm", "lArm", "rLeg"],
    ),
  ),
  sk(
    "teacher",
    "slim",
    O("#e8b896", "#8a5a2a", "long", "#4a6a2a", "#9a3a4a", true, "#3a3a46", "#2a1e18"),
    glasses("#3a2a1a"),
    bun("#8a5a2a"),
    openCoat("#5a5a66"),
    skirt("#3a3a46", 6),
  ),
  sk(
    "judge",
    "classic",
    O("#d8a888", "#e8e8e8", "short", "#3a3a3a", "#141418", true, "#141418", "#0c0c0c"),
    bowtie("#f4f4f4", "#f4f4f4"),
    openCoat("#18181e", [3, 4]),
    cloak("#18181e"),
    glasses("#c0a040"),
  ),
  sk(
    "detective",
    "classic",
    O("#e2b090", "#3a2a1a", "short", "#4a3a2a", "#e2d8c0", true, "#4a4a52", "#2a1c12"),
    capHat("#6a5a40", 2, "#4a3e2a"),
    openCoat("#a08a5a", [2, 3, 4, 5]),
    tie("#6a2a2a"),
    belt("#4a3a22", "#c0a040", 7),
  ),
  sk(
    "mailCarrier",
    "classic",
    O("#c89272", "#2a1a10", "short", "#3a5a3a", "#5a7ab0", false, "#2a3a5a", "#1a1a1a"),
    capHat("#2a3a5a", 2, "#1a2438"),
    sash("#8a5a2a"),
    backpack("#a07a40", "#8a5a2a"),
    shorts(6, 9),
    socks("#2a3a5a", 9, 11),
  ),
  sk(
    "barista",
    "slim",
    O("#e8c0a0", "#2a1a12", "long", "#5a3a20", "#f2f2f2", false, "#1e1e22", "#3a2a1a"),
    apron("#6a4228", "#6a4228"),
    headband("#6a4228", 2),
    chest(["###", "#.#", "###"], { "#": "#f2e6d0" }, 5),
  ),
  sk(
    "gardener",
    "slim",
    O("#e0aa84", "#c87a3a", "long", "#4a8a3a", "#c8e0a0", false, "#6a8a4a", "#5a4028"),
    capHat("#e8d080", 2, "#b89a40"),
    apron("#4a7a3a"),
    gloves("#e8c040"),
    boots("#3a6a2a", 8),
  ),
  sk(
    "sailor",
    "classic",
    O("#e0b090", "#c08a4a", "short", "#2a6aa8", "#f4f4f4", false, "#1e2a48", "#141414"),
    capHat("#f4f4f4", 2, "#1e2a48"),
    stripes("#2a4a8a", 2, false),
    scarf("#d83030", "#b02020"),
  ),
  sk(
    "captain",
    "classic",
    O("#d8a080", "#d8d8d8", "short", "#2a4a6a", "#1e2a48", true, "#1e2a48", "#141414"),
    capHat("#f4f4f4", 2, "#141414"),
    beard("#d8d8d8"),
    buttons("#e2c040", 2),
    buttons("#e2c040", 5),
    cuffs("#e2c040"),
  ),
  sk(
    "lifeguard",
    "classic",
    O("#c88a60", "#e8c060", "short", "#2a7ab0", "#d83a2a", false, "#d83a2a", "#c88a60"),
    bare(0),
    shorts(5, 12),
    chest([".#.", "###", ".#."], { "#": "#f4f4f4" }, 3),
    shades("#141414", "#2a4a6a"),
  ),
  sk(
    "cowboy",
    "classic",
    O("#d09870", "#6a4020", "short", "#3a5a3a", "#b0603a", true, "#4a6aa0", "#5a3a20"),
    capHat("#8a5a2a", 2, "#5a3a1a"),
    headband("#3a2a1a", 1),
    vest("#6a4228"),
    belt("#3a2a1a", "#d8b040"),
    boots("#5a3a20", 7),
    scarf("#c83030", "#a82020"),
  ),
  sk(
    "beekeeper",
    "classic",
    O("#e0b090", "#5a3a20", "short", "#3a3a3a", "#f2f0e6", true, "#f2f0e6", "#e8e2c8"),
    beanie("#f2f0e6", "#e0dccc"),
    maskLow("#3a3a3a"),
    gloves("#e8d070"),
    chest(["#.#", ".#.", "#.#"], { "#": "#f2c230" }, 4),
  ),
  sk(
    "baker",
    "classic",
    O("#eac2a0", "#e8d8b0", "short", "#4a6a3a", "#f4f4f4", true, "#c8b8a0", "#4a3a2a"),
    beanie("#f4f4f4", "#e0e0e0"),
    apron("#e8d0a0"),
    mustache("#c8a870"),
    pattern((x, y) => ((x + y) % 7 === 0 ? "#e8e0d0" : null), ARMS),
  ),
  sk(
    "waiter",
    "classic",
    O("#d8a888", "#1a1410", "short", "#3a3a3a", "#f4f4f4", true, "#141418", "#0c0c0c"),
    bowtie("#141418"),
    vest("#141418"),
    apron("#141418"),
  ),
  // ---------------------------------------------------------- show & music
  sk(
    "mime",
    "classic",
    O("#f4f0ec", "#141414", "short", "#141414", "#f4f4f4", true, "#141414", "#141414"),
    stripes("#141414", 2),
    capHat("#141414", 1),
    lips("#c82a2a"),
    gloves("#f4f4f4"),
    faceArt(["........", "........", "........", ".#....#.", "........"], { "#": "#141414" }),
    suspenders("#c82a2a"),
  ),
  sk(
    "clown",
    "classic",
    O("#f4ece4", "#e8603a", "long", "#3a6ab0", "#f2c230", true, "#3a6ab0", "#d83030"),
    pattern(
      (x, y) => ((x + y) % 4 === 0 ? "#d83030" : (x + y) % 4 === 2 ? "#3a9a4a" : null),
      ["body", "rArm", "lArm"],
    ),
    faceArt(["........", "........", "........", "........", "........", "...##...", "..####.."], {
      "#": "#d83030",
    }),
    bowtie("#3a9a4a", "#f2c230"),
  ),
  sk(
    "magician",
    "classic",
    O("#e0b496", "#141414", "short", "#3a3a6a", "#141418", true, "#141418", "#0c0c0c"),
    topHat("#141418", "#a8202a"),
    mustache("#141414"),
    cloak("#141418", "#a8202a"),
    bowtie("#a8202a"),
    gloves("#f4f4f4"),
  ),
  sk(
    "dj",
    "classic",
    O("#a8704a", "#141010", "short", "#2a2a2a", "#1e1e24", true, "#2a2a32", "#e8e8e8"),
    headphones("#1e1e1e", "#3ae0e0"),
    shades("#141414", "#3ae0e0"),
    chest(NOTE5, { "#": "#3ae0e0" }, 3),
    cuffs("#3ae0e0", 9),
  ),
  sk(
    "rockstar",
    "classic",
    O("#e2b494", "#1a1a1a", "long", "#3a3a3a", "#141414", false, "#2a2a30", "#0c0c0c"),
    openCoat("#2a2a2a", [2, 3, 4, 5]),
    chest(SKULL5, { "#": "#e8e8e8" }, 3),
    bandana("#c82a2a", "#f4f4f4"),
    belt("#3a3a3a", "#c0c0c0", 11),
    cuffs("#c0c0c0", 8),
  ),
  sk(
    "popstar",
    "slim",
    O("#f0c8b0", "#e85aa8", "long", "#7a3ab0", "#e85aa8", false, "#2a2a3a", "#f4f4f4"),
    headphones("#f4f4f4", "#e85aa8"),
    chest(STAR5, { "#": "#f2e04a" }, 3),
    skirt("#7a3ab0", 5, "#f2e04a"),
    bare(3),
  ),
  sk(
    "ballerina",
    "slim",
    O("#f4d4c4", "#5a3a20", "long", "#4a6aa0", "#f4b8c8", false, "#f4d4c4", "#f4b8c8"),
    bun("#5a3a20"),
    skirt("#f8d0dc", 3, "#ffe4ec"),
    bare(1),
    socks("#f4c8d8", 2, 10),
  ),
  // ---------------------------------------------------------- sports
  sk(
    "skateboarder",
    "classic",
    O("#d8a080", "#c8803a", "short", "#3a6a3a", "#e8603a", false, "#3a4a5a", "#f4f4f4"),
    beanie("#2a2a2a", "#3a3a3a"),
    chest(BOLT5, { "#": "#f2e04a" }, 3),
    legsPattern((_x, y) => (y === 9 ? "#e8e8e8" : null)),
  ),
  sk(
    "surfer",
    "classic",
    O("#c8885a", "#f0d070", "long", "#2a8ab0", "#2ab0b0", false, "#2a5ab0", "#c8885a"),
    bare(0),
    pattern((x, y) => (y < 4 ? null : (x * 2 + y) % 5 === 0 ? "#f4f4f4" : null)),
    shorts(6, 12),
    chest(["...", "###"], { "#": "#f2e04a" }, 10),
  ),
  sk(
    "cyclist",
    "classic",
    O("#e0b090", "#3a2a1a", "short", "#3a5a8a", "#f2d02a", false, "#141418", "#e8e8e8"),
    helmet("#e83a3a", "#141414"),
    chestBand("#141418", 5),
    shades("#141414", "#e83a3a"),
    shorts(7, 10),
    socks("#f4f4f4", 10, 11),
  ),
  sk(
    "golfer",
    "classic",
    O("#e2b494", "#8a5a2a", "short", "#3a6a3a", "#f4f4f4", false, "#c8b07a", "#f4f4f4"),
    capHat("#f4f4f4", 2, "#3a8a4a"),
    pattern((x, y) => ((x + y) % 4 === 0 && y < 10 ? "#3a8a4a" : null)),
    gloves("#f4f4f4"),
    belt("#3a2a1a"),
  ),
  sk(
    "hockey",
    "classic",
    O("#e0b494", "#6a4020", "short", "#3a5a8a", "#c8202a", true, "#141418", "#e8e8e8"),
    helmet("#141418", "#c0c0c0"),
    stripes("#f4f4f4", 4),
    shirtNumber("9", "#f4f4f4"),
    gloves("#141418", 9),
    socks("#c8202a", 6, 10),
  ),
  sk(
    "baseball",
    "classic",
    O("#c89070", "#2a1a12", "short", "#2a4a6a", "#f4f4f4", false, "#f4f4f4", "#141414"),
    capHat("#1e3a7a", 2, "#141e3a"),
    pattern((x) => (x % 3 === 1 ? "#a0a8c0" : null), ["body", "rLeg", "lLeg"]),
    shirtNumber("7", "#1e3a7a"),
    belt("#141414", undefined, 11),
  ),
  sk(
    "karate",
    "classic",
    O("#e4b898", "#141414", "short", "#3a3a3a", "#f4f4f4", false, "#f4f4f4", "#e4b898"),
    headband("#c82a2a", 2),
    belt("#141414", "#141414", 7),
    bare(4),
    legsPattern((_x, y) => (y >= 11 ? "#e4b898" : null)),
    chest(["#....", ".#...", "..#.."], { "#": "#d8d8d8" }, 0),
  ),
  sk(
    "swimmer",
    "slim",
    O("#e8c0a0", "#3a2a1a", "none", "#3a6aa8", "#2a6ad8", false, "#2a6ad8", "#e8c0a0"),
    capHat("#f2c230", 3),
    shades("#141414", "#3ad8e8"),
    bare(0),
    shorts(3, 12),
  ),
  sk(
    "runner",
    "slim",
    O("#a8704a", "#141010", "long", "#3a2a1a", "#3ad87a", false, "#141418", "#f2f2f2"),
    headband("#3ad87a", 2),
    shirtNumber("01", "#141418"),
    shorts(5, 9),
    socks("#3ad87a", 9, 11),
    bare(3),
  ),
  // ---------------------------------------------------------- royalty & fantasy
  sk(
    "king",
    "classic",
    O("#e0b090", "#8a5a2a", "short", "#3a5a8a", "#8a1a2a", true, "#4a1a2a", "#2a1a12"),
    crown("#f2c230", "#d82a3a"),
    beard("#8a5a2a"),
    cloak("#a8202a", "#f4f4f4"),
    belt("#f2c230", "#d82a3a"),
    cuffs("#f4f4f4"),
  ),
  sk(
    "queen",
    "slim",
    O("#f0ccb0", "#c8a05a", "long", "#3a6aa8", "#5a2a8a", true, "#5a2a8a", "#2a1a12"),
    crown("#f2c230", "#3ab0e8"),
    cloak("#7a3ab0", "#f4f4f4"),
    skirt("#5a2a8a", 10, "#f2c230"),
    chest(["#.#", ".#."], { "#": "#f2c230" }, 1),
  ),
  sk(
    "princess",
    "slim",
    O("#f4d0b8", "#f0d070", "long", "#3a8ad8", "#f4a0c0", false, "#f4a0c0", "#f4f4f4"),
    tiara("#f2c230", "#e85aa8"),
    skirt("#f4b8d0", 10, "#ffe0ec"),
    bare(3),
    chestBand("#ffe0ec", 5),
    chest(HEART, { "#": "#e85aa8" }, 6),
  ),
  sk(
    "prince",
    "classic",
    O("#e8bc98", "#3a2414", "short", "#3a6a3a", "#2a4a8a", true, "#f2f2f2", "#3a2414"),
    tiara("#f2c230", "#3ab06a"),
    sash("#f2c230"),
    cloak("#2a3a6a", "#f2c230"),
    boots("#3a2414", 7),
  ),
  sk(
    "angel",
    "slim",
    O("#f4dcc8", "#f4e090", "long", "#7ab0e8", "#f4f4f4", false, "#f4f4f4", "#f4f0e0"),
    halo("#f2d84a"),
    skirt("#f4f4f4", 10, "#f2e8b0"),
    bare(3),
    chest(["#.....#", "##...##", "###.###", "##...##"], { "#": "#e8f0ff" }, 2, "back"),
  ),
  sk(
    "demon",
    "classic",
    O("#b8323a", "#141010", "short", "#f2d02a", "#141010", true, "#2a1414", "#141010"),
    horns("#2a2a2a"),
    eyes("#f2d02a"),
    cloak("#2a0a0e", "#d83a2a"),
    pattern((x, y) => ((x + y * 2) % 9 === 0 ? "#5a1a1a" : null)),
  ),
  sk(
    "witch",
    "slim",
    O("#d8e0c0", "#3a1a4a", "long", "#3ad86a", "#2a1a3a", true, "#2a1a3a", "#141414"),
    witchHat("#1e1424", "#7a3ab0"),
    cloak("#1e1424", "#7a3ab0"),
    skirt("#2a1a3a", 9, "#7a3ab0"),
    stripes("#7a3ab0", 2, false),
  ),
  sk(
    "viking",
    "classic",
    O("#e2b494", "#c8602a", "short", "#3a7ab0", "#6a5a4a", true, "#5a4a3a", "#4a3020"),
    capHat("#8a8a90", 2, "#5a5a60"),
    horns("#e8e0c8"),
    beard("#c8602a"),
    belt("#3a2a1a", "#c0c0c0"),
    boots("#6a4a2a", 7, "#9a8a6a"),
    cuffs("#9a8a6a", 6),
  ),
  sk(
    "samurai",
    "classic",
    O("#e4b894", "#141010", "short", "#2a2a2a", "#8a1a1a", true, "#1e1e2a", "#141414"),
    capHat("#1e1e24", 2, "#141418"),
    tiara("#d8b040", "#d8b040"),
    armor("#8a1a1a", ["body"]),
    sash("#141414"),
    pattern((x) => (x % 2 ? "#2a2a3a" : null), ["rLeg", "lLeg"]),
  ),
  sk(
    "gladiator",
    "classic",
    O("#c8885a", "#2a1a10", "short", "#3a2a1a", "#c8885a", false, "#8a1a1a", "#6a4a2a"),
    capHat("#b08a3a", 3),
    mohawk("#c82a2a"),
    bare(0),
    armor("#b08a3a", ["rArm"]),
    skirt("#8a1a1a", 6, "#b08a3a"),
    boots("#6a4a2a", 8),
  ),
  sk(
    "mummy",
    "classic",
    O("#d8ccb0", "#d8ccb0", "none", "#1a1a1a", "#d8ccb0", true, "#d8ccb0", "#c8bca0"),
    pattern(
      (x, y) => ((x + y) % 3 === 0 ? "#b8ac90" : y % 2 ? "#e8dcc0" : null),
      ["head", "body", "rArm", "lArm", "rLeg", "lLeg"],
    ),
    faceArt(["........", "........", "........", "........", ".##..##.", "........"], {
      "#": "#141414",
    }),
  ),
  sk(
    "zombie",
    "classic",
    O("#6a9a5a", "#2a3a22", "short", "#c82a2a", "#3a6a8a", false, "#3a3a6a", "#2a2a2a"),
    pattern((x, y) => ((x * 3 + y) % 7 === 0 ? "#2a4a5a" : null)),
    legsPattern((x, y) => (y > 7 && (x + y) % 3 === 0 ? "#6a9a5a" : null)),
    bare(4),
  ),
  sk(
    "ghost",
    "classic",
    O("#eef2f8", "#eef2f8", "none", "#1a1a2a", "#eef2f8", true, "#eef2f8", "#dde4f0"),
    whole("#eef2f8", ["body", "rArm", "lArm", "rLeg", "lLeg"]),
    faceArt(
      [
        "........",
        "........",
        "........",
        ".##..##.",
        ".##..##.",
        "........",
        "...##...",
        "...##...",
      ],
      { "#": "#1a1a2a" },
    ),
  ),
  sk(
    "werewolf",
    "classic",
    O("#6a5a4a", "#4a3a2a", "short", "#f2c230", "#8a6a4a", false, "#3a4a6a", "#4a3a2a"),
    beard("#4a3a2a"),
    catEars("#4a3a2a", "#8a6a5a"),
    bare(3),
    pattern((x, y) => ((x + y) % 5 === 0 ? "#3a3022" : null), ["rArm", "lArm"]),
    legsPattern((_x, y) => (y > 9 ? "#4a3a2a" : null)),
  ),
  sk(
    "druid",
    "classic",
    O("#d8a888", "#e8e8e0", "long", "#3a8a4a", "#3a5a2a", true, "#3a5a2a", "#4a3020"),
    hoodP("#2e4a22"),
    beard("#e8e8e0"),
    belt("#6a4a2a", "#3ab06a"),
    pattern((x, y) => ((x + y * 2) % 6 === 0 ? "#5a8a3a" : null), ["body"]),
  ),
  sk(
    "genie",
    "classic",
    O("#5aa0d8", "#141418", "none", "#f2e04a", "#7a3ab0", false, "#7a3ab0", "#f2c230"),
    turban("#7a3ab0", "#f2c230"),
    bare(0),
    vest("#7a3ab0"),
    cuffs("#f2c230", 8),
    legsPattern((x, y) => (y > 8 ? (x % 2 ? "#5aa0d8" : "#4a90c8") : null)),
  ),
  sk(
    "steampunk",
    "classic",
    O("#e0b494", "#6a4020", "short", "#5a3a20", "#e8dcc0", true, "#4a3a2a", "#2a1a10"),
    topHat("#4a3020", "#b8862a"),
    glasses("#b8862a"),
    vest("#7a4a22"),
    buttons("#b8862a", 2),
    belt("#3a2414", "#b8862a"),
    boots("#2a1a10", 7),
  ),
  sk(
    "alien",
    "classic",
    O("#7ad86a", "#7ad86a", "none", "#141414", "#c0c8d0", true, "#c0c8d0", "#8a9aa8"),
    faceArt(["........", "........", "........", ".##..##.", ".##..##.", "........", "........"], {
      "#": "#141414",
    }),
    chest([".#.", "###", ".#."], { "#": "#7ad86a" }, 3),
    belt("#8a9aa8"),
  ),
  sk(
    "pumpkinHead",
    "classic",
    O("#e8862a", "#e8862a", "none", "#141414", "#5a3a6a", true, "#3a2a1a", "#2a1a10"),
    pattern((x) => (x % 2 ? "#d8761a" : null), ["head"]),
    faceArt(
      [
        "...##...",
        "........",
        "........",
        ".#....#.",
        ".##..##.",
        "........",
        ".######.",
        "..#..#..",
      ],
      { "#": "#2a1a0a" },
    ),
    capHat("#3a6a2a", 0),
    chest(["#.#", ".#."], { "#": "#e8862a" }, 3),
  ),
  sk(
    "snowman",
    "classic",
    O("#f4f6fa", "#141414", "none", "#141414", "#f4f6fa", true, "#f4f6fa", "#e8ecf4"),
    topHat("#141414", "#c82a2a"),
    faceArt(
      [
        "........",
        "........",
        "........",
        "........",
        ".#....#.",
        "...##...",
        "........",
        "........",
      ],
      { "#": "#141414" },
    ),
    faceArt(["........", "........", "........", "........", "........", "...##...", "........"], {
      "#": "#e8862a",
    }),
    scarf("#c82a2a", "#e84a4a"),
    buttons("#141414"),
  ),
  sk(
    "scarecrow",
    "classic",
    O("#e8d090", "#e8c040", "none", "#141414", "#8a6a3a", true, "#5a6a8a", "#6a4a2a"),
    capHat("#6a5030", 2, "#4a3820"),
    plaidPart("#8a6a3a", "#c8a050"),
    faceArt(
      [
        "........",
        "........",
        "........",
        "........",
        ".#....#.",
        "........",
        ".######.",
        "........",
      ],
      { "#": "#2a1a0a" },
    ),
    gloves("#e8c040", 11),
    legsPattern((x, y) => (y === 11 && x % 2 ? "#e8c040" : null)),
  ),
  // ---------------------------------------------------------- styles
  sk(
    "gamer",
    "classic",
    O("#e0b494", "#2a1a12", "short", "#3a3a3a", "#1e1e28", true, "#2a2a36", "#e8e8e8"),
    headphones("#141414", "#7a3ae8"),
    chest(["#####", "#.#.#", "#####"], { "#": "#7a3ae8" }, 3),
    cuffs("#7a3ae8"),
  ),
  sk(
    "punk",
    "classic",
    O("#e8bc9c", "#e83a8a", "none", "#3a3a3a", "#141414", false, "#2a2a30", "#141414"),
    mohawk("#e83a8a"),
    openCoat("#1e1e1e", [2, 3, 4, 5]),
    chest(SKULL5, { "#": "#e83a8a" }, 3),
    legsPattern((x, y) => ((x + y) % 4 === 0 ? "#c82a2a" : null)),
    boots("#141414", 8),
  ),
  sk(
    "goth",
    "slim",
    O("#ece4e4", "#141414", "long", "#7a1a3a", "#141414", true, "#141414", "#0c0c0c"),
    lips("#5a0a2a"),
    skirt("#1e1e1e", 7, "#5a0a2a"),
    chest(MOON5, { "#": "#c8c8d0" }, 3),
    cuffs("#5a0a2a"),
  ),
  sk(
    "nerd",
    "classic",
    O("#f0c8a8", "#8a5a2a", "short", "#3a5a8a", "#e8e0c0", false, "#5a5a3a", "#3a2a1a"),
    glasses("#141414"),
    tie("#3a5a8a"),
    vest("#a85a3a"),
    socks("#f4f4f4", 9, 11),
  ),
  sk(
    "hipster",
    "classic",
    O("#e2b494", "#5a3a20", "short", "#3a5a3a", "#c87a3a", true, "#3a3e4a", "#6a4228"),
    beanie("#3a5a5a", "#2e4a4a"),
    beard("#5a3a20"),
    glasses("#141414"),
    plaidPart("#c87a3a", "#5a3a20"),
    legsPattern((_x, y) => (y === 9 ? "#5a6070" : null)),
  ),
  sk(
    "businessman",
    "classic",
    O("#d8a888", "#2a2a2a", "short", "#3a4a5a", "#f4f4f4", true, "#2a2e3a", "#141414"),
    tie("#2a4a8a"),
    openCoat("#2a2e3a", [3, 4]),
    backpack("#3a2a1a", "#2a2e3a"),
    belt("#141414", "#c0c0c0"),
  ),
  sk(
    "businesswoman",
    "slim",
    O("#f0c4a4", "#3a2014", "long", "#5a3a2a", "#f4f4f4", true, "#3a3a46", "#141414"),
    openCoat("#5a1a2a", [3, 4]),
    skirt("#3a3a46", 6),
    bun("#3a2014"),
  ),
  sk(
    "tracksuit",
    "classic",
    O("#e2b494", "#2a1a12", "short", "#3a3a3a", "#2a4ab0", true, "#2a4ab0", "#f4f4f4"),
    armStripe("#f4f4f4"),
    legStripe("#f4f4f4"),
    zipper("#f4f4f4"),
    capHat("#141414", 2, "#2a2a2a"),
  ),
  sk(
    "pajamas",
    "classic",
    O("#f0c8a8", "#c87a3a", "short", "#3a6aa8", "#8ab8e8", true, "#8ab8e8", "#e8a8b8"),
    beanie("#8ab8e8", "#6a98c8", "#f4f4f4"),
    pattern(
      (x, y) => ((x + y) % 4 === 0 ? "#f4f4f4" : null),
      ["body", "rArm", "lArm", "rLeg", "lLeg"],
    ),
    buttons("#f4f4f4"),
  ),
  sk(
    "tourist",
    "classic",
    O("#eac0a0", "#c8a05a", "short", "#3a6aa8", "#3ab0b0", false, "#e8d8a8", "#c8885a"),
    capHat("#f4f4f4", 2, "#3ab0b0"),
    shades("#141414", "#5a3a20"),
    pattern((x, y) =>
      (x * 2 + y) % 5 === 0 ? "#f2e04a" : (x + y * 3) % 7 === 0 ? "#e85a8a" : null,
    ),
    chest(["##", "##"], { "#": "#2a2a2a" }, 4),
    shorts(6, 10),
  ),
  sk(
    "hiker",
    "slim",
    O("#e8b896", "#a8582a", "long", "#3a7a3a", "#d8602a", true, "#5a5a3a", "#4a3020"),
    capHat("#3a5a3a", 2, "#2a4a2a"),
    backpack("#3a6aa8", "#2a4a8a"),
    boots("#4a3020", 8, "#8a6a4a"),
    socks("#c8c8b0", 7, 8),
  ),
  // ---------------------------------------------------------- holidays & the world
  sk(
    "santa",
    "classic",
    O("#f0c4a4", "#f4f4f4", "short", "#3a6aa8", "#c82a2a", true, "#c82a2a", "#141414"),
    beanie("#c82a2a", "#f4f4f4", "#f4f4f4"),
    beard("#f4f4f4"),
    belt("#141414", "#f2c230", 7),
    cuffs("#f4f4f4"),
    boots("#141414", 8),
  ),
  sk(
    "diver",
    "classic",
    O("#e0b494", "#2a1a12", "short", "#3a6aa8", "#1e2a3a", true, "#1e2a3a", "#f2c230"),
    shades("#f2c230", "#8ad8f2"),
    maskLow("#2a2a2a"),
    backpack("#f2c230", "#2a2a2a"),
    armStripe("#f2c230"),
    legStripe("#f2c230"),
  ),
  sk(
    "blacksmith",
    "classic",
    O("#c88a60", "#2a1a10", "short", "#3a2a1a", "#6a5a4a", false, "#3a3a3a", "#2a1a10"),
    apron("#3a2414", "#3a2414"),
    bare(4),
    gloves("#4a3020", 9),
    beard("#2a1a10"),
    boots("#2a1a10", 9),
  ),
  sk(
    "alchemist",
    "classic",
    O("#e0b494", "#a8a8a8", "long", "#7a3ab0", "#5a3a6a", true, "#3a2a3a", "#2a1a20"),
    hoodP("#3a2a4a"),
    goatee("#a8a8a8"),
    sash("#3ab06a"),
    chest([".#.", "###", "###"], { "#": "#3ae07a" }, 7),
    cuffs("#c0a040"),
  ),
  sk(
    "arcticExplorer",
    "classic",
    O("#f0c8a8", "#5a3a20", "short", "#3a6aa8", "#e85a2a", true, "#2a3a5a", "#5a3a20"),
    hoodP("#e85a2a"),
    headband("#e8e0d0", 0),
    shades("#141414", "#8ad8f2"),
    gloves("#2a2a2a"),
    boots("#5a3a20", 8, "#e8e0d0"),
    zipper("#2a2a2a"),
  ),
  sk(
    "desertNomad",
    "classic",
    O("#b87a50", "#2a1a10", "none", "#3a2a1a", "#e8d0a0", true, "#c8b080", "#8a6a3a"),
    turban("#e8d0a0", "#3ab0b0"),
    maskLow("#d8c090"),
    sash("#b8402a"),
    cloak("#d8c090"),
  ),
  sk(
    "engineer",
    "classic",
    O("#e2b494", "#3a2a1a", "short", "#3a3a3a", "#c82a2a", true, "#3a3a46", "#2a2a2a"),
    glasses("#3a3a3a"),
    helmet("#f2c230", "#2a2a2a"),
    chest(["#.#.", ".#.#", "#.#."], { "#": "#f2c230" }, 3),
    gloves("#3a3a3a"),
    belt("#2a2a2a", "#c0c0c0"),
  ),
];

function plaidPart(a: string, line: string): Part {
  return (c, r) => around(c, ["body", ...ARMS], plaid(hex(a), hex(line), r));
}

// ------------------------------------------------------------ cape kit

type Front = (x: number, y: number, w: number, h: number) => Color;

function cp(id: string, lining: string, front: Front): CapePreset {
  return { id, draw: () => drawCape({ lining: hex(lining), front }) };
}

const grad =
  (...stops: string[]): Front =>
  (_x, y, _w, h) => {
    const t = (y / (h - 1)) * (stops.length - 1);
    const i = Math.min(stops.length - 2, Math.floor(t));
    return mix(hex(stops[i]!), hex(stops[i + 1]!), t - i);
  };

/** An emblem on a background, optionally with a border. */
const emblem =
  (bg: Front, rows: string[], colors: Record<string, string>, border?: string, yOff = 0): Front =>
  (x, y, w, h) => {
    if (border && (x === 0 || y === 0 || x === w - 1 || y === h - 1)) return hex(border);
    return stamp(rows, colors, x, y, w, h, yOff) ?? bg(x, y, w, h);
  };

const hstripes =
  (colors: string[], size = 2): Front =>
  (_x, y) =>
    hex(colors[Math.floor(y / size) % colors.length]!);
const vstripes =
  (colors: string[], size = 2): Front =>
  (x) =>
    hex(colors[Math.floor(x / size) % colors.length]!);
const diagonal =
  (colors: string[], size = 2, dir = 1): Front =>
  (x, y) =>
    hex(colors[Math.floor((x * dir + y + 40) / size) % colors.length]!);
const chevron =
  (a: string, b: string, size = 3): Front =>
  (x, y, w) =>
    Math.floor((y + Math.abs(x - (w - 1) / 2)) / size) % 2 ? hex(a) : hex(b);
const checker =
  (a: string, b: string, size = 2): Front =>
  (x, y) =>
    (Math.floor(x / size) + Math.floor(y / size)) % 2 ? hex(a) : hex(b);
const dots =
  (bg: string, colors: string[], step = 3): Front =>
  (x, y) =>
    x % step === 1 && (y + (Math.floor(x / step) % 2) * Math.floor(step / 2)) % step === 1
      ? hex(colors[(x + y) % colors.length]!)
      : hex(bg);
const argyle =
  (a: string, b: string, line: string): Front =>
  (x, y) => {
    const u = (x + 0.5) / 5;
    const v = (y + 0.5) / 8;
    const d = Math.abs((u % 1) - 0.5) + Math.abs((v % 1) - 0.5);
    if ((x + y) % 5 === 0 && (x - y + 50) % 5 === 0) return hex(line);
    return d < 0.5 ? hex(a) : hex(b);
  };
const zigzag =
  (a: string, b: string, size = 3): Front =>
  (x, y) =>
    Math.floor((y + (x % 4 < 2 ? x % 4 : 4 - (x % 4))) / size) % 2 ? hex(a) : hex(b);
const waves =
  (colors: string[]): Front =>
  (x, y) =>
    hex(colors[Math.floor(y + Math.sin(x * 0.9) * 1.5 + 2) % colors.length]!);
const texture = (
  seed: number,
  fn: (v: number, x: number, y: number, r: () => number) => string,
  sx = 0.6,
  sy = 0.6,
): Front => {
  return (x, y) => {
    // Fresh generators per call keep every draw identical.
    const n = noise(seed);
    const r = rng(seed * 131 + x * 17 + y);
    return hex(fn(n(x * sx, y * sy), x, y, r));
  };
};

// Emblems (at most 10 wide, 12 tall).
const E: Record<string, string[]> = {
  heart: [".##.##.", "#######", "#######", ".#####.", "..###..", "...#..."],
  sword: ["..s..", ".sss.", ".sss.", ".sss.", ".sss.", ".sss.", "ggggg", "..h..", "..h..", "..p.."],
  shield: ["ooooooo", "offxffo", "offxffo", "oxxxxxo", "offxffo", ".ofxfo.", "..oxo..", "...o..."],
  crown: ["g..g..g", "gg.g.gg", "ggggggg", "grgbgrg", "ggggggg"],
  bolt: ["...##", "..##.", ".##..", "#####", "..##.", ".##..", "##..."],
  anchor: [
    "..###..",
    "..#.#..",
    "..###..",
    "#######",
    "...#...",
    "...#...",
    "#..#..#",
    "##.#.##",
    ".#####.",
  ],
  note: ["..#####", "..#...#", "..#...#", "..#...#", "###.###", "###.###"],
  paw: ["..#.#..", ".......", "#.....#", "..###..", ".#####.", ".#####.", ".##.##."],
  clover: [".##.##.", "#######", ".#####.", "#######", ".##.##.", "...#...", "...#..."],
  key: ["..###..", ".#...#.", ".#...#.", "..###..", "...#...", "...##..", "...#...", "...##.."],
  eye: ["..###..", ".#ooo#.", "#oopoo#", ".#ooo#.", "..###.."],
  gem: [".aaaaa.", "abababa", ".bbbbb.", "..bbb..", "...b..."],
  leaf: ["....##", "...###", "..###.", ".###..", ".##...", "#....."],
  flame: ["...#...", "..##...", "..###..", ".#####.", ".##y##.", "##yyy##", "#yyyyy#", ".#yyy#."],
  crosshair: ["...#...", ".#####.", ".#...#.", "##.#.##", ".#...#.", ".#####.", "...#..."],
  pickaxe: [".hhhhh.", "h..w..h", "...w...", "...w...", "...w...", "...w..."],
  potion: ["..c..", "..g..", ".ggg.", "gpppg", "gpppg", "gpppg", ".ggg."],
  mushroom: [".rrrrr.", "rrwrrwr", "rrrrrrr", "..sss..", "..sss..", "..sss.."],
  tree: ["...g...", "..ggg..", ".ggggg.", "..ggg..", ".ggggg.", "ggggggg", "...t...", "...t..."],
  mountain: ["...w...", "..www..", "..###..", ".#####.", "#######"],
  arrow: ["...#...", "..###..", ".#####.", "...#...", "...#...", "...#...", "..#.#.."],
  smiley: [".yyyyy.", "yyyyyyy", "ykyyyky", "yyyyyyy", "ykyyyky", "yykkkyy", ".yyyyy."],
  cube: ["..ttt..", "ltttttr", "lllrrrr", "lllrrrr", "lllrrrr", ".llrrr."],
  planet: ["..ppp..", ".ppppp.", "rrrrrrr", ".ppppp.", "..ppp.."],
  rocket: ["...r...", "..www..", "..wbw..", "..www..", ".rwwwr.", "r.www.r", "...o..."],
  castle: ["#.#.#", "#####", "#####", "##.##", "##.##"],
  fish: ["..###...", ".#####.#", "#k######", ".#####.#", "..###..."],
  butterfly: ["aa...aa", "aaa.aaa", "aaabaaa", ".aabaa.", "aaabaaa", "aa.b.aa"],
  chest: ["########", "#wwwwww#", "########", "#www#ww#", "#wwwwww#", "########"],
  bird: ["##...##", ".##.##.", "..###..", "...#..."],
  skull: [".wwwww.", "wwwwwww", "wkkwkkw", "wwwkwww", ".wwwww.", ".w.w.w."],
  hourglass: ["#######", ".#sss#.", "..#s#..", "...#...", "..#s#..", ".#sss#.", "#######"],
  sparkle: ["...#...", "...#...", "..###..", "#######", "..###..", "...#...", "...#..."],
};

const W = "#f4f4f4";

export const CATALOG_CAPES: CapePreset[] = [
  // ---------------------------------------------------------- emblems
  cp("heartCape", "#5a0a1a", emblem(grad("#ff6a8a", "#a8102a"), E.heart!, { "#": W }, W)),
  cp(
    "swordCape",
    "#1a2a3a",
    emblem(grad("#3a5a7a", "#141e2a"), E.sword!, {
      s: "#d8e0e8",
      g: "#d8b040",
      h: "#6a4a2a",
      p: "#d8b040",
    }),
  ),
  cp(
    "shieldCape",
    "#2a1a0a",
    emblem(grad("#6a4a2a", "#3a2614"), E.shield!, { o: "#c0c0c8", f: "#2a4ab0", x: "#f2c230" }),
  ),
  cp(
    "crownCape",
    "#3a0a4a",
    emblem(
      grad("#7a2ab0", "#3a0a4a"),
      E.crown!,
      { g: "#f2c230", r: "#e83a3a", b: "#3a8ae8" },
      "#f2c230",
      2,
    ),
  ),
  cp("boltCape", "#141420", emblem(grad("#2a2a3a", "#0c0c14"), E.bolt!, { "#": "#f2e04a" })),
  cp("anchorCape", "#0c1a3a", emblem(grad("#1e3a6a", "#0c1a3a"), E.anchor!, { "#": W }, "#c8a040")),
  cp("noteCape", "#2a0a3a", emblem(grad("#e85aa8", "#5a1a8a"), E.note!, { "#": "#141414" })),
  cp("pawCape", "#3a2414", emblem(grad("#e8c8a0", "#c89a6a"), E.paw!, { "#": "#5a3a20" })),
  cp(
    "cloverCape",
    "#0e3a14",
    emblem(grad("#3ab06a", "#14602a"), E.clover!, { "#": "#e8ffe0" }, "#f2c230"),
  ),
  cp("keyCape", "#2a1e0a", emblem(grad("#4a3a20", "#1e160a"), E.key!, { "#": "#f2c230" })),
  cp(
    "eyeCape",
    "#0a0a14",
    emblem(
      grad("#2a1a4a", "#0a0a14"),
      E.eye!,
      { "#": "#7a3ab0", o: "#e8e0ff", p: "#141414" },
      "#7a3ab0",
    ),
  ),
  cp(
    "gemCape",
    "#0a2a2a",
    emblem(grad("#1a4a4a", "#0a1e1e"), E.gem!, { a: "#a8f8ff", b: "#3ad8e8" }),
  ),
  cp("leafCape", "#1e2a0a", emblem(grad("#f2e8c0", "#d8c890"), E.leaf!, { "#": "#3a8a2a" })),
  cp(
    "flameCape",
    "#1a0a04",
    emblem(grad("#2a1408", "#0c0602"), E.flame!, { "#": "#ff6a1a", y: "#ffd23a" }),
  ),
  cp(
    "targetCape",
    "#1a1a1a",
    emblem(grad("#e8e8e8", "#b8b8b8"), E.crosshair!, { "#": "#d82a2a" }, "#141414"),
  ),
  cp(
    "pickaxeCape",
    "#2a2a2a",
    emblem(grad("#7a7a7a", "#4a4a4a"), E.pickaxe!, { h: "#5ae8f0", w: "#8a5a2a" }),
  ),
  cp(
    "potionCape",
    "#1a0a2a",
    emblem(grad("#3a1a5a", "#140a20"), E.potion!, { c: "#8a5a2a", g: "#d8e8f0", p: "#e83ab0" }),
  ),
  cp(
    "mushroomCape",
    "#2a1a0a",
    emblem(grad("#8ac86a", "#4a8a3a"), E.mushroom!, { r: "#d82a2a", w: W, s: "#f2e8d0" }),
  ),
  cp(
    "treeCape",
    "#0e2a14",
    emblem(grad("#a8e0ff", "#e8f8ff"), E.tree!, { g: "#2a8a3a", t: "#6a4a2a" }, "#2a8a3a"),
  ),
  cp(
    "mountainCape",
    "#1a2a4a",
    emblem(grad("#f8b06a", "#7a5ab0"), E.mountain!, { w: W, "#": "#3a3a5a" }, undefined, -3),
  ),
  cp("arrowCape", "#0a2a1a", emblem(grad("#1ab07a", "#0a5a3a"), E.arrow!, { "#": W })),
  cp(
    "smileyCape",
    "#1a3a8a",
    emblem(grad("#3a8ae8", "#1a3a8a"), E.smiley!, { y: "#f2e04a", k: "#141414" }),
  ),
  cp(
    "cubeCape",
    "#141414",
    emblem(grad("#2a2a34", "#141418"), E.cube!, { t: "#7ae86a", l: "#8a5a3a", r: "#6a4228" }),
  ),
  cp(
    "planetCape",
    "#06061a",
    emblem((x, y) => ((x * 7 + y * 3) % 11 === 0 ? hex(W) : hex("#0c0c2a")), E.planet!, {
      p: "#e8a04a",
      r: "#f2e0b0",
    }),
  ),
  cp(
    "rocketCape",
    "#0a0a2a",
    emblem(grad("#1a1a4a", "#3a1a5a"), E.rocket!, {
      r: "#e83a3a",
      w: W,
      b: "#3ab0e8",
      o: "#ffa02a",
    }),
  ),
  cp(
    "castleCape",
    "#2a2a3a",
    emblem(grad("#8ac8f0", "#d8f0ff"), E.castle!, { "#": "#7a7a8a" }, "#5a5a6a", -3),
  ),
  cp(
    "fishCape",
    "#0a2a4a",
    emblem(waves(["#2a7ab0", "#1e6aa0", "#2a8ac0"]), E.fish!, { "#": "#ff8a2a", k: "#141414" }),
  ),
  cp(
    "butterflyCape",
    "#2a0a2a",
    emblem(grad("#ffe0f0", "#f0b8e0"), E.butterfly!, { a: "#a83ae8", b: "#2a1a2a" }),
  ),
  cp(
    "treasureCape",
    "#2a1a0a",
    emblem(grad("#3a2a5a", "#1a1030"), E.chest!, { "#": "#5a3a1a", w: "#a87a3a" }, "#f2c230"),
  ),
  cp("birdCape", "#1a3a5a", emblem(grad("#8ad0ff", "#f8e0b0"), E.bird!, { "#": "#2a2a3a" })),
  cp(
    "skullCape",
    "#0c0c0c",
    emblem(grad("#1e1e1e", "#0c0c0c"), E.skull!, { w: "#e8e8e0", k: "#0c0c0c" }, "#5a5a5a"),
  ),
  cp(
    "hourglassCape",
    "#2a1e0a",
    emblem(grad("#3a2a5a", "#1a1030"), E.hourglass!, { "#": "#c8a04a", s: "#f2e0a0" }),
  ),
  cp("sparkleCape", "#0a0a2a", emblem(grad("#3a2a8a", "#0a0a2a"), E.sparkle!, { "#": "#fff6b0" })),
  // ---------------------------------------------------------- stripes
  cp("candyStripes", "#c84a7a", hstripes(["#ff8ab0", W], 2)),
  cp("bee", "#141414", hstripes(["#f2c230", "#141414"], 3)),
  cp("navyStripes", "#0c1a3a", hstripes(["#1e3a7a", W], 2)),
  cp("retro", "#3a2414", hstripes(["#6a3a1a", "#d86a1a", "#f2b02a", "#f2e0a0"], 4)),
  cp("pastel", "#c8b8e8", hstripes(["#ffb8c8", "#ffe0a8", "#c8f0c0", "#b8e0ff", "#d8c8ff"], 3)),
  cp("circus", "#a8101a", vstripes(["#e82a2a", W], 2)),
  cp("mint", "#3a9a7a", vstripes(["#8ae8c8", W], 1)),
  cp("nightStripes", "#0c0614", vstripes(["#5a2a8a", "#141018"], 2)),
  cp("forestStripes", "#14301a", vstripes(["#2a6a3a", "#3a8a4a", "#1e4a2a"], 2)),
  cp("hazard", "#141414", diagonal(["#f2c230", "#141414"], 2)),
  cp("candyCane", "#a8101a", diagonal(["#e82a2a", W], 2, -1)),
  cp("neonDiagonal", "#0a0a0a", diagonal(["#0a0a0a", "#3ae8f0", "#0a0a0a", "#e83ae0"], 2)),
  cp("oceanDiagonal", "#0a2a4a", diagonal(["#1e6ab0", "#3a9ad8", "#8ad0f0"], 2, -1)),
  cp("lavaDiagonal", "#2a0a04", diagonal(["#2a0a04", "#e84a1a", "#ffb02a"], 2)),
  cp("blueChevron", "#0c1a3a", chevron("#2a5ab0", "#8ab8f0")),
  cp("redChevron", "#3a0a0a", chevron("#c82a2a", "#f2d0d0")),
  cp("goldChevron", "#2a1a04", chevron("#f2c230", "#3a2a0a")),
  cp("greenChevron", "#0a2a14", chevron("#3ab06a", "#d8f8e0", 2)),
  // ---------------------------------------------------------- colour
  cp("aurora", "#06141e", grad("#06141e", "#1ab08a", "#7a3ae8", "#06141e")),
  cp("mintFade", "#3a8a7a", grad("#e8fff4", "#3ac8a0")),
  cp("peach", "#c86a4a", grad("#ffe0c8", "#ff8a6a")),
  cp("twilight", "#1a0a2a", grad("#ff9a6a", "#a83a8a", "#1a0a3a")),
  cp("toxic", "#0a1a04", grad("#d8ff3a", "#3ab01a", "#0a2a04")),
  cp("cherry", "#3a0414", grad("#ff3a5a", "#7a0a1e")),
  cp("steel", "#2a2e34", grad("#d8dce4", "#6a707a", "#2a2e34")),
  cp("cottonCandy", "#c87ab0", grad("#ffb8e0", "#b8e0ff")),
  cp("redChecker", "#141414", checker("#c82a2a", "#141414", 3)),
  cp("royalChecker", "#2a0a3a", checker("#5a1a8a", "#f2c230", 1)),
  cp("polkaDots", "#a8101a", dots("#d82a2a", [W], 3)),
  cp("blueDots", "#0c2a5a", dots("#1e4a9a", ["#8ad0ff"], 3)),
  cp("confetti", "#d8d8d8", dots(W, ["#e83a3a", "#3a8ae8", "#f2c230", "#3ab06a", "#a83ae8"], 2)),
  // ---------------------------------------------------------- materials
  cp(
    "marble",
    "#c8c8c8",
    texture(
      301,
      (v) => (Math.abs(v - 0.5) < 0.04 ? "#7a7a84" : v > 0.5 ? "#f2f2f2" : "#e2e2e6"),
      0.5,
      0.9,
    ),
  ),
  cp(
    "wood",
    "#4a2e14",
    texture(302, (v, x) => ((x + Math.round(v * 3)) % 3 === 0 ? "#6a4220" : "#9a6a3a"), 0.4, 0.15),
  ),
  cp("brick", "#4a2414", (x, y) =>
    y % 3 === 2 || (x + (Math.floor(y / 3) % 2) * 2) % 4 === 0
      ? hex("#d8ccc0")
      : hex((x * 3 + y) % 5 ? "#a8402a" : "#8a3220"),
  ),
  cp(
    "netherrack",
    "#2a0606",
    texture(
      304,
      (v, _x, _y, r) => (v > 0.6 ? "#8a1a1a" : r() > 0.85 ? "#c83a2a" : "#5a1010"),
      0.9,
      0.9,
    ),
  ),
  cp(
    "obsidian",
    "#06040c",
    texture(305, (v) => (v > 0.7 ? "#5a2a8a" : v > 0.55 ? "#2a1a3a" : "#120a1a"), 0.8, 0.8),
  ),
  cp(
    "sand",
    "#c8a860",
    texture(
      306,
      (v, _x, _y, r) => (r() > 0.9 ? "#e8d8a0" : v > 0.5 ? "#e0c888" : "#d4b876"),
      0.7,
      0.7,
    ),
  ),
  cp(
    "clouds",
    "#5a9ad8",
    texture(307, (v) => (v > 0.58 ? W : v > 0.5 ? "#d8ecff" : "#7ab8f0"), 0.45, 0.6),
  ),
  cp(
    "rust",
    "#3a1a0a",
    texture(
      308,
      (v, _x, _y, r) => (v > 0.6 ? "#b8602a" : r() > 0.8 ? "#5a6a6a" : "#7a3a1a"),
      0.8,
      0.8,
    ),
  ),
  cp(
    "slime",
    "#1a4a14",
    texture(309, (v) => (v > 0.65 ? "#c8ff9a" : v > 0.4 ? "#7ad85a" : "#4aa83a"), 0.6, 0.6),
  ),
  cp(
    "crystal",
    "#0a1a3a",
    texture(
      310,
      (v, x, y) => ((x + y) % 4 === 0 && v > 0.5 ? W : v > 0.5 ? "#7ab8ff" : "#3a6ad8"),
      0.5,
      0.5,
    ),
  ),
  // ---------------------------------------------------------- patterns
  cp("argyle", "#0a2a1a", argyle("#2a7a4a", "#1a4a7a", "#f2e8c0")),
  cp("redArgyle", "#3a0a0a", argyle("#c82a2a", "#3a1a1a", "#f2c230")),
  cp("orangeZigzag", "#3a1a04", zigzag("#ff8a1a", "#3a1a04")),
  cp("purpleZigzag", "#1a0a2a", zigzag("#a83ae8", "#f2e8ff", 2)),
  cp("turquoiseWaves", "#0a3a3a", waves(["#1ab0b0", "#8ae8e0", "#0a7a7a"])),
  cp("sunsetWaves", "#3a0a1a", waves(["#ff8a3a", "#ff3a6a", "#ffd23a", "#a83a8a"])),
];
