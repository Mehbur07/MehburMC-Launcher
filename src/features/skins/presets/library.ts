// Phase 22 collection: 20 skins and 15 capes, drawn in code like the
// presets (no third-party art). Listed under Skin & Cape presets.

import type { SkinModel } from "../../../lib/ipc/bindings/SkinModel";
import type { Pixels } from "../editor/ops";
import { putPx } from "../editor/ops";
import type { CapePreset, SkinPreset } from "./index";
import {
  type Color,
  type FacePainter,
  type Outfit,
  type SkinCanvas,
  drawCape,
  drawPerson,
  grain,
  hex,
  mix,
  paintBox,
  paintFace,
  rng,
  shade,
  skinCanvas,
} from "./paint";

const SIDES = ["front", "back", "left", "right"] as const;
const ARMS = ["rArm", "lArm"];
const LEGS = ["rLeg", "lLeg"];

function person(model: SkinModel, outfit: Outfit, paint: (c: SkinCanvas, r: () => number) => void) {
  return (): Pixels => {
    const c = skinCanvas(model);
    drawPerson(c, outfit);
    paint(c, rng(outfit.seed * 7 + 3));
    return c.p;
  };
}

/** Paints the four side faces of every listed box. */
function around(c: SkinCanvas, ids: string[], fn: FacePainter | Color) {
  for (const id of ids) paintBox(c.p, c.box(id), fn, [...SIDES]);
}

/** A helmet or cap on the hat layer: top plus the first `rows` rows. */
function cap(c: SkinCanvas, color: Color, rows: number, r: () => number, brim?: Color) {
  paintFace(c.p, c.face("hat", "top"), () => grain(color, r, 8));
  for (const s of SIDES) {
    paintFace(c.p, c.face("hat", s), (_x, y) =>
      y < rows ? grain(color, r, 8) : brim && y === rows ? brim : null,
    );
  }
}

/** A hood on the hat layer that leaves the face open. */
function hood(c: SkinCanvas, color: Color, r: () => number) {
  paintFace(c.p, c.face("hat", "top"), () => grain(color, r, 8));
  paintFace(c.p, c.face("hat", "back"), () => grain(color, r, 8));
  for (const s of ["left", "right"] as const) {
    paintFace(c.p, c.face("hat", s), () => grain(color, r, 8));
  }
  paintFace(c.p, c.face("hat", "front"), (x, y) =>
    y === 0 || x === 0 || x === 7 ? grain(shade(color, -10), r, 6) : null,
  );
}

/** Bare forearms (or whole arms) in skin colour from row `from`. */
function bareArms(c: SkinCanvas, skin: Color, from: number, r: () => number) {
  around(c, ARMS, (_x, y) => (y >= from ? grain(skin, r, 6) : null));
  for (const id of ARMS) paintFace(c.p, c.face(id, "bottom"), shade(skin, -10));
  if (from === 0) for (const id of ARMS) paintFace(c.p, c.face(id, "top"), skin);
}

/** Bare legs from row `from` down to the socks at `to` (exclusive). */
function bareLegs(c: SkinCanvas, skin: Color, from: number, to: number, r: () => number) {
  around(c, LEGS, (_x, y) => (y >= from && y < to ? grain(skin, r, 6) : null));
}

/** 3×5 digits for shirt numbers. */
const DIGITS: Record<string, string[]> = {
  "0": ["###", "#.#", "#.#", "#.#", "###"],
  "1": [".#.", "##.", ".#.", ".#.", "###"],
  "2": ["###", "..#", "###", "#..", "###"],
  "3": ["###", "..#", "###", "..#", "###"],
  "7": ["###", "..#", ".#.", ".#.", ".#."],
};

function number(
  c: SkinCanvas,
  box: string,
  side: "front" | "back",
  n: string,
  x0: number,
  y0: number,
  col: Color,
) {
  const f = c.face(box, side);
  [...n].forEach((d, i) =>
    DIGITS[d]?.forEach((row, dy) =>
      [...row].forEach(
        (ch, dx) => ch === "#" && putPx(c.p, f.x + x0 + i * 4 + dx, f.y + y0 + dy, col),
      ),
    ),
  );
}

/** Small 2×2 check / plaid pattern. */
const check =
  (a: Color, b: Color, r: () => number, size = 2): FacePainter =>
  (x, y) =>
    grain((Math.floor(x / size) + Math.floor(y / size)) % 2 ? a : b, r, 8);

function plaid(a: Color, line: Color, r: () => number): FacePainter {
  return (x, y) => {
    const v = x % 4 === 1;
    const h = y % 4 === 1;
    return grain(v && h ? shade(line, -20) : v || h ? line : a, r, 8);
  };
}

export const LIBRARY_SKINS: SkinPreset[] = [
  // ------------------------------------------------------------ professions
  {
    id: "chef",
    model: "classic",
    draw: person(
      "classic",
      {
        skin: "#e2b08c",
        hair: "#3b2618",
        hairStyle: "short",
        eyes: "#4a6b2f",
        shirt: "#f4f4f2",
        longSleeves: true,
        pants: "#2b2b2b",
        shoes: "#1c1c1c",
        seed: 101,
      },
      (c, r) => {
        // Tall white toque.
        cap(c, hex("#ffffff"), 3, r, hex("#dcdcdc"));
        // Double-breasted buttons and a red neckerchief.
        paintFace(c.p, c.face("body", "front"), (x, y) =>
          y < 2 && x > 1 && x < 6
            ? hex("#c62828")
            : (x === 2 || x === 5) && y > 2 && y < 10 && y % 2 === 1
              ? hex("#3a3a3a")
              : null,
        );
        // Houndstooth trousers.
        around(c, LEGS, (x, y) =>
          y < 10
            ? check(hex("#2b2b2b"), hex("#d9d9d9"), r, 1)(x, y, c.face("rLeg", "front"))
            : null,
        );
        around(c, ARMS, (_x, y) => (y === 9 ? hex("#dadada") : null));
      },
    ),
  },
  {
    id: "doctor",
    model: "classic",
    draw: person(
      "classic",
      {
        skin: "#b97b55",
        hair: "#1e1410",
        hairStyle: "short",
        eyes: "#2f2014",
        shirt: "#4fa3c7",
        longSleeves: false,
        pants: "#4fa3c7",
        shoes: "#f0f0f0",
        seed: 102,
      },
      (c, r) => {
        const coat = hex("#f7f7f7");
        // Open white coat on the jacket layer.
        paintFace(c.p, c.face("jacket", "front"), (x) =>
          x < 2 || x > 5 ? grain(coat, r, 4) : null,
        );
        for (const s of ["back", "left", "right"] as const)
          paintFace(c.p, c.face("jacket", s), () => grain(coat, r, 4));
        for (const id of ["rSleeve", "lSleeve"])
          paintBox(c.p, c.box(id), (_x, y) => (y < 10 ? grain(coat, r, 4) : null), [...SIDES]);
        // Stethoscope around the neck.
        const steel = hex("#5d6670");
        const f = c.face("body", "front");
        for (const [x, y] of [
          [2, 0],
          [2, 1],
          [2, 2],
          [3, 3],
          [4, 3],
          [5, 2],
          [5, 1],
          [5, 0],
        ] as const)
          putPx(c.p, f.x + x, f.y + y, steel);
        putPx(c.p, f.x + 4, f.y + 4, hex("#c0c6cc"));
        // Name badge on the coat.
        const j = c.face("jacket", "front");
        putPx(c.p, j.x + 6, j.y + 3, hex("#2f7bd8"));
        putPx(c.p, j.x + 7, j.y + 3, hex("#2f7bd8"));
      },
    ),
  },
  {
    id: "firefighter",
    model: "classic",
    draw: person(
      "classic",
      {
        skin: "#d9a07a",
        hair: "#5a3a22",
        hairStyle: "short",
        eyes: "#3c5a8a",
        shirt: "#c6a14b",
        longSleeves: true,
        pants: "#c6a14b",
        shoes: "#1d1d1d",
        seed: 103,
      },
      (c, r) => {
        // Red helmet with a gold shield.
        cap(c, hex("#c62323"), 2, r, hex("#8f1515"));
        const hf = c.face("hat", "front");
        for (const [x, y] of [
          [3, 0],
          [4, 0],
          [3, 1],
          [4, 1],
        ] as const)
          putPx(c.p, hf.x + x, hf.y + y, hex("#f2c230"));
        // Reflective stripes.
        const yellow = hex("#e9f23a");
        const silver = hex("#d6dadf");
        around(c, ["body"], (_x, y) => (y === 8 ? silver : y === 7 || y === 9 ? yellow : null));
        around(c, ARMS, (_x, y) => (y === 6 ? silver : y === 5 || y === 7 ? yellow : null));
        around(c, LEGS, (_x, y) => (y === 7 ? silver : y === 6 || y === 8 ? yellow : null));
        around(c, ARMS, (_x, y) => (y >= 10 ? hex("#2a2a2a") : null)); // gloves
        // Buckles down the front.
        paintFace(c.p, c.face("body", "front"), (x, y) =>
          x === 3 && y % 3 === 1 && y < 7 ? hex("#2a2a2a") : null,
        );
      },
    ),
  },
  {
    id: "farmer",
    model: "classic",
    draw: person(
      "classic",
      {
        skin: "#e0a679",
        hair: "#9a5a2a",
        hairStyle: "short",
        eyes: "#3d7a3a",
        shirt: "#b8332e",
        longSleeves: false,
        pants: "#3e6aa8",
        shoes: "#5a3b22",
        seed: 104,
      },
      (c, r) => {
        // Plaid shirt.
        around(c, ["body"], plaid(hex("#b8332e"), hex("#f0d8a8"), r));
        around(c, ARMS, (x, y) =>
          y < 4 ? plaid(hex("#b8332e"), hex("#f0d8a8"), r)(x, y, c.face("rArm", "front")) : null,
        );
        // Denim overalls with straps.
        const denim = hex("#3e6aa8");
        paintFace(c.p, c.face("body", "front"), (x, y) =>
          y >= 4 ? grain(denim, r, 8) : x === 1 || x === 6 ? denim : null,
        );
        paintFace(c.p, c.face("body", "back"), (x, y) =>
          y >= 6 || ((x === 1 || x === 6) && y < 6) ? grain(denim, r, 8) : null,
        );
        paintFace(c.p, c.face("body", "front"), (x, y) =>
          (x === 1 || x === 6) && y === 4 ? hex("#e1c25a") : null,
        );
        // Straw hat with a brim.
        cap(c, hex("#e6c46e"), 2, r, hex("#b7923f"));
        paintFace(c.p, c.face("hat", "front"), (_x, y) => (y === 1 ? hex("#b8332e") : null));
      },
    ),
  },
  {
    id: "builder",
    model: "classic",
    draw: person(
      "classic",
      {
        skin: "#c88a62",
        hair: "#2e2018",
        hairStyle: "short",
        eyes: "#4a3424",
        shirt: "#6b7480",
        longSleeves: false,
        pants: "#33507a",
        shoes: "#7a4a22",
        seed: 105,
      },
      (c, r) => {
        // Hi-vis vest on the jacket layer.
        const vest = hex("#ff7a12");
        const stripe = hex("#e3e6ea");
        paintFace(c.p, c.face("jacket", "front"), (x, y) =>
          x === 3 || x === 4 ? null : y === 7 ? stripe : grain(vest, r, 6),
        );
        for (const s of ["back", "left", "right"] as const) {
          paintFace(c.p, c.face("jacket", s), (_x, y) => (y === 7 ? stripe : grain(vest, r, 6)));
        }
        // Tool belt.
        around(c, ["body"], (x, y) =>
          y === 10 ? (x % 3 === 0 ? hex("#c9c9c9") : hex("#6b4423")) : null,
        );
        // Yellow hard hat.
        cap(c, hex("#f7c71f"), 2, r, hex("#c79c10"));
      },
    ),
  },
  {
    id: "scientist",
    model: "classic",
    draw: person(
      "classic",
      {
        skin: "#f0c8a8",
        hair: "#e4e4e4",
        hairStyle: "long",
        eyes: "#4a78a8",
        shirt: "#6a3fa0",
        longSleeves: true,
        pants: "#3a3a46",
        shoes: "#2a2a2a",
        seed: 106,
      },
      (c, r) => {
        // Wild white hair on the hat layer.
        paintFace(c.p, c.face("hat", "top"), (x, y) =>
          (x + y) % 3 ? grain(hex("#f2f2f2"), r, 10) : null,
        );
        for (const s of ["left", "right", "back"] as const) {
          paintFace(c.p, c.face("hat", s), (x, y) =>
            y < 3 && (x + y) % 2 === 0 ? hex("#f2f2f2") : null,
          );
        }
        // Goggles on the forehead.
        const hf = c.face("hat", "front");
        for (let x = 0; x < 8; x++) putPx(c.p, hf.x + x, hf.y + 2, hex("#3a3a3a"));
        for (const x of [1, 2, 5, 6]) putPx(c.p, hf.x + x, hf.y + 2, hex("#7fd8e8"));
        // Long lab coat.
        const coat = hex("#f5f5f5");
        paintFace(c.p, c.face("jacket", "front"), (x) =>
          x < 3 || x > 4 ? grain(coat, r, 4) : null,
        );
        for (const s of ["back", "left", "right"] as const)
          paintFace(c.p, c.face("jacket", s), () => grain(coat, r, 4));
        for (const id of ["rSleeve", "lSleeve"])
          paintBox(c.p, c.box(id), (_x, y) => (y < 10 ? grain(coat, r, 4) : null), [...SIDES]);
        for (const id of ["rPants", "lPants"])
          paintBox(c.p, c.box(id), (_x, y) => (y < 4 ? grain(coat, r, 4) : null), [...SIDES]);
        // Bubbling flask in the pocket.
        const j = c.face("jacket", "front");
        putPx(c.p, j.x + 1, j.y + 5, hex("#9ad0d8"));
        putPx(c.p, j.x + 1, j.y + 6, hex("#3ee07a"));
      },
    ),
  },
  // ------------------------------------------------------------ fantasy
  {
    id: "vampire",
    model: "classic",
    draw: person(
      "classic",
      {
        skin: "#e9ddd8",
        hair: "#121216",
        hairStyle: "short",
        eyes: "#c3161c",
        shirt: "#16161c",
        longSleeves: true,
        pants: "#16161c",
        shoes: "#0c0c0e",
        seed: 107,
      },
      (c, r) => {
        const f = c.face("head", "front");
        // Widow's peak, red eyes, fangs.
        putPx(c.p, f.x + 3, f.y + 2, hex("#121216"));
        putPx(c.p, f.x + 4, f.y + 2, hex("#121216"));
        for (const x of [2, 3, 4, 5]) putPx(c.p, f.x + x, f.y + 6, hex("#5a1418"));
        putPx(c.p, f.x + 3, f.y + 6, hex("#ffffff"));
        putPx(c.p, f.x + 4, f.y + 6, hex("#ffffff"));
        // White shirt, red ascot, high red collar.
        paintFace(c.p, c.face("body", "front"), (x, y) =>
          (x === 3 || x === 4) && y < 7 ? (y < 2 ? hex("#a8121a") : hex("#efefef")) : null,
        );
        paintFace(c.p, c.face("jacket", "front"), (x, y) =>
          y < 2 && (x < 2 || x > 5) ? hex("#a8121a") : null,
        );
        for (const s of ["left", "right", "back"] as const) {
          paintFace(c.p, c.face("jacket", s), (_x, y) => (y < 2 ? hex("#a8121a") : null));
        }
        // Long cloak falls on the back.
        paintFace(c.p, c.face("jacket", "back"), (_x, y) =>
          y >= 2 ? grain(hex("#0e0e12"), r, 4) : null,
        );
        paintFace(c.p, c.face("body", "front"), (x, y) =>
          (x === 2 || x === 5) && y < 10 ? hex("#2a0a0e") : null,
        );
      },
    ),
  },
  {
    id: "elf",
    model: "slim",
    draw: person(
      "slim",
      {
        skin: "#f1d1b4",
        hair: "#f0d77a",
        hairStyle: "long",
        eyes: "#3fa66a",
        shirt: "#2f7a3e",
        longSleeves: true,
        pants: "#cbb98a",
        shoes: "#6a4426",
        seed: 108,
      },
      (c, r) => {
        // Pointed ears on the hat layer.
        for (const s of ["left", "right"] as const) {
          const hs = c.face("hat", s);
          putPx(c.p, hs.x + 4, hs.y + 3, hex("#f1d1b4"));
          putPx(c.p, hs.x + 3, hs.y + 4, hex("#f1d1b4"));
          putPx(c.p, hs.x + 4, hs.y + 4, hex("#e2bf9e"));
        }
        // Leaf circlet.
        for (const s of SIDES) {
          paintFace(c.p, c.face("hat", s), (x, y) =>
            y === 1 ? (x % 2 ? hex("#57b84a") : hex("#2f7a3e")) : null,
          );
        }
        // Tunic with a belt and a leaf pattern.
        around(c, ["body"], (x, y) =>
          y === 8 ? hex("#5a3a1e") : y === 9 && x === 3 ? hex("#d9b23a") : null,
        );
        paintFace(c.p, c.face("jacket", "front"), (x, y) =>
          y > 9 && (x + y) % 2 === 0 ? hex("#2f7a3e") : null,
        );
        paintFace(c.p, c.face("body", "front"), (x, y) =>
          y < 2 && (x === 3 || x === 4) ? hex("#f1d1b4") : null,
        );
        // Quiver strap across the chest.
        const f = c.face("body", "front");
        for (let i = 0; i < 8; i++) putPx(c.p, f.x + i, f.y + 1 + i, hex("#7a4a26"));
        // Tall boots.
        around(c, LEGS, (_x, y) => (y >= 7 ? grain(hex("#6a4426"), r, 8) : null));
      },
    ),
  },
  {
    id: "necromancer",
    model: "classic",
    draw: person(
      "classic",
      {
        skin: "#c9c6bd",
        hair: "#2a1d36",
        hairStyle: "none",
        eyes: "#45ff8a",
        shirt: "#3a1f52",
        longSleeves: true,
        pants: "#3a1f52",
        shoes: "#1a1024",
        seed: 109,
      },
      (c, r) => {
        // Skull-like face: dark sockets around glowing eyes, teeth.
        const f = c.face("head", "front");
        for (const [x, y] of [
          [1, 3],
          [2, 3],
          [5, 3],
          [6, 3],
          [1, 4],
          [6, 4],
        ] as const)
          putPx(c.p, f.x + x, f.y + y, hex("#1c1c1c"));
        putPx(c.p, f.x + 2, f.y + 4, hex("#45ff8a"));
        putPx(c.p, f.x + 5, f.y + 4, hex("#45ff8a"));
        for (const x of [2, 3, 4, 5])
          putPx(c.p, f.x + x, f.y + 6, x % 2 ? hex("#efefe4") : hex("#2a2a2a"));
        hood(c, hex("#2c163f"), r);
        // Robe trim and a bone belt.
        const trim = hex("#b38bd9");
        around(c, ["body"], (x, y) => (y === 9 ? (x % 2 ? hex("#efefe4") : hex("#8a8a80")) : null));
        paintFace(c.p, c.face("jacket", "front"), (x, y) =>
          (x === 3 || x === 4) && y > 1 ? trim : null,
        );
        for (const id of ["rPants", "lPants"])
          paintBox(
            c.p,
            c.box(id),
            (_x, y) => (y < 9 ? grain(hex("#3a1f52"), r, 6) : y === 9 ? trim : null),
            [...SIDES],
          );
        around(c, ARMS, (_x, y) => (y >= 10 ? grain(hex("#c9c6bd"), r, 6) : null));
      },
    ),
  },
  {
    id: "fairy",
    model: "slim",
    draw: person(
      "slim",
      {
        skin: "#f6d6c4",
        hair: "#f08aa8",
        hairStyle: "long",
        eyes: "#7a4ad0",
        shirt: "#c99bf0",
        longSleeves: false,
        pants: "#f6d6c4",
        shoes: "#f3a6c8",
        seed: 110,
      },
      (c, r) => {
        // Flower crown.
        for (const s of SIDES) {
          paintFace(c.p, c.face("hat", s), (x, y) =>
            y === 1
              ? x % 3 === 0
                ? hex("#ffe36a")
                : x % 3 === 1
                  ? hex("#ff8fb8")
                  : hex("#7ad08a")
              : null,
          );
        }
        // Wings painted on the back.
        const wing = hex("#bfeaff");
        const edge = hex("#7fc8f0");
        paintFace(c.p, c.face("jacket", "back"), (x, y) => {
          const left = x < 4 ? 3 - x : x - 4;
          if (y < 1 || y > 9) return null;
          const w = y < 5 ? 3 - Math.abs(y - 3) : 2 - Math.abs(y - 7);
          return left <= w ? (left === w ? edge : wing) : null;
        });
        // Skirt of the dress over the legs; bare legs below.
        for (const id of ["rPants", "lPants"]) {
          paintBox(
            c.p,
            c.box(id),
            (x, y) =>
              y < 4 ? grain(y === 3 && x % 2 ? hex("#e7c3ff") : hex("#c99bf0"), r, 6) : null,
            [...SIDES],
          );
        }
        around(c, ["body"], (x, y) => (y > 4 && (x + y) % 4 === 0 ? hex("#f2e4ff") : null));
        bareArms(c, hex("#f6d6c4"), 2, r);
        // Sparkles.
        const jf = c.face("jacket", "front");
        putPx(c.p, jf.x + 2, jf.y + 6, hex("#fff6a8"));
        putPx(c.p, jf.x + 5, jf.y + 8, hex("#fff6a8"));
      },
    ),
  },
  // ------------------------------------------------------------ sports
  {
    id: "basketball",
    model: "classic",
    draw: person(
      "classic",
      {
        skin: "#8a5a3c",
        hair: "#141010",
        hairStyle: "short",
        eyes: "#241810",
        shirt: "#7a2fd0",
        longSleeves: false,
        pants: "#7a2fd0",
        shoes: "#f2f2f2",
        seed: 111,
      },
      (c, r) => {
        const skin = hex("#8a5a3c");
        // Sleeveless jersey with a number.
        bareArms(c, skin, 0, r);
        around(c, ["body"], (x, y, f) =>
          f.name === "front" && y === 0 && x > 1 && x < 6 ? skin : null,
        );
        number(c, "body", "front", "23", 0, 3, hex("#ffc533"));
        number(c, "body", "back", "23", 0, 3, hex("#ffc533"));
        around(c, ["body"], (_x, y) => (y === 11 ? hex("#ffc533") : null));
        // Shorts, bare knees, high socks.
        bareLegs(c, skin, 6, 8, r);
        around(c, LEGS, (_x, y) =>
          y >= 8 && y < 11 ? hex("#f4f4f4") : y === 5 ? hex("#ffc533") : null,
        );
        // Headband and wristbands.
        for (const s of SIDES)
          paintFace(c.p, c.face("hat", s), (_x, y) => (y === 2 ? hex("#f4f4f4") : null));
        around(c, ARMS, (_x, y) => (y === 8 ? hex("#f4f4f4") : null));
      },
    ),
  },
  {
    id: "tennis",
    model: "slim",
    draw: person(
      "slim",
      {
        skin: "#efc09a",
        hair: "#7a4a24",
        hairStyle: "long",
        eyes: "#3a6aa0",
        shirt: "#fafafa",
        longSleeves: false,
        pants: "#efc09a",
        shoes: "#fafafa",
        seed: 112,
      },
      (c) => {
        // Green visor.
        for (const s of SIDES)
          paintFace(c.p, c.face("hat", s), (_x, y) => (y === 2 ? hex("#2fb36a") : null));
        const hf = c.face("hat", "front");
        for (let x = 1; x < 7; x++) putPx(c.p, hf.x + x, hf.y + 3, hex("#25945a"));
        // Ponytail at the back.
        paintFace(c.p, c.face("hat", "back"), (x, y) =>
          (x === 3 || x === 4) && y >= 4 ? hex("#6a3e1c") : null,
        );
        // Polo collar and green trim, pleated skirt.
        paintFace(c.p, c.face("body", "front"), (x, y) =>
          y === 0 && (x === 3 || x === 4)
            ? hex("#efc09a")
            : y === 1 && (x === 2 || x === 5)
              ? hex("#2fb36a")
              : null,
        );
        around(c, ARMS, (_x, y) => (y === 3 ? hex("#2fb36a") : null));
        for (const id of ["rPants", "lPants"]) {
          paintBox(
            c.p,
            c.box(id),
            (x, y) => (y < 4 ? (x % 2 ? hex("#ececec") : hex("#fafafa")) : null),
            [...SIDES],
          );
        }
        around(c, LEGS, (_x, y) =>
          y >= 9 && y < 10 ? hex("#fafafa") : y >= 10 ? hex("#e8e8e8") : null,
        );
        // Sweatband.
        around(c, ["lArm"], (_x, y) => (y === 8 ? hex("#2fb36a") : null));
      },
    ),
  },
  {
    id: "boxer",
    model: "classic",
    draw: person(
      "classic",
      {
        skin: "#d39a72",
        hair: "#201812",
        hairStyle: "short",
        eyes: "#2a2a2a",
        shirt: "#d39a72",
        longSleeves: false,
        pants: "#d11f2a",
        shoes: "#151515",
        seed: 113,
      },
      (c, r) => {
        const skin = hex("#d39a72");
        // Bare chest with muscle shading.
        bareArms(c, skin, 0, r);
        paintFace(c.p, c.face("body", "front"), (x, y) =>
          (y === 3 && x > 0 && x < 7) ||
          (x === 3 && y > 4 && y < 10) ||
          ((y === 6 || y === 8) && x > 1 && x < 6)
            ? shade(skin, -22)
            : null,
        );
        // Shorts with a white waistband, bare legs, boots.
        around(c, ["body"], (_x, y) =>
          y >= 10 ? (y === 10 ? hex("#fafafa") : hex("#d11f2a")) : null,
        );
        bareLegs(c, skin, 5, 9, r);
        around(c, LEGS, (_x, y) => (y >= 9 ? grain(hex("#151515"), r, 6) : null));
        // Big red gloves.
        around(c, ARMS, (_x, y) =>
          y >= 8 ? grain(y === 8 ? hex("#fafafa") : hex("#d11f2a"), r, 8) : null,
        );
        for (const id of ARMS) paintFace(c.p, c.face(id, "bottom"), hex("#b0141e"));
        for (const id of ["rSleeve", "lSleeve"])
          paintBox(c.p, c.box(id), (_x, y) => (y >= 9 ? hex("#d11f2a") : null), [
            ...SIDES,
            "bottom",
          ]);
      },
    ),
  },
  {
    id: "skier",
    model: "classic",
    draw: person(
      "classic",
      {
        skin: "#e8b896",
        hair: "#c88a3a",
        hairStyle: "short",
        eyes: "#2a5a9a",
        shirt: "#16a3a3",
        longSleeves: true,
        pants: "#1c1f26",
        shoes: "#3a3f4a",
        seed: 114,
      },
      (c, r) => {
        // Bobble hat.
        cap(c, hex("#e0303a"), 3, r);
        for (const s of SIDES)
          paintFace(c.p, c.face("hat", s), (x, y) =>
            y === 2 ? (x % 2 ? hex("#fafafa") : hex("#e0303a")) : null,
          );
        const top = c.face("hat", "top");
        for (const [x, y] of [
          [3, 3],
          [4, 3],
          [3, 4],
          [4, 4],
        ] as const)
          putPx(c.p, top.x + x, top.y + y, hex("#fafafa"));
        // Mirrored goggles.
        const hf = c.face("hat", "front");
        for (let x = 0; x < 8; x++) putPx(c.p, hf.x + x, hf.y + 3, hex("#1a1a1a"));
        for (let x = 1; x < 7; x++)
          putPx(c.p, hf.x + x, hf.y + 4, x < 4 ? hex("#ff9a1f") : hex("#ffd23a"));
        // Jacket stripe and zip.
        around(c, ["body"], (_x, y) => (y === 5 ? hex("#e22a8a") : null));
        around(c, ARMS, (_x, y) => (y === 4 ? hex("#e22a8a") : y >= 10 ? hex("#1c1f26") : null));
        paintFace(c.p, c.face("body", "front"), (x, y) =>
          x === 4 && y !== 5 ? hex("#0f7f7f") : null,
        );
      },
    ),
  },
  // ------------------------------------------------------------ seasons
  {
    id: "winter",
    model: "classic",
    draw: person(
      "classic",
      {
        skin: "#f0c6a6",
        hair: "#4a2c18",
        hairStyle: "short",
        eyes: "#3a7ab0",
        shirt: "#24345a",
        longSleeves: true,
        pants: "#3a3a42",
        shoes: "#5a3a22",
        seed: 115,
      },
      (c, r) => {
        // Puffer coat quilting.
        around(c, ["body", ...ARMS], (_x, y) => (y % 3 === 2 ? shade(hex("#24345a"), -18) : null));
        // Striped scarf.
        const red = hex("#d8323a");
        const white = hex("#f4f4f4");
        around(c, ["jacket"], (x, y, f) =>
          y < 2
            ? (x + y) % 2
              ? red
              : white
            : f.name === "front" && x === 5 && y < 6
              ? y % 2
                ? red
                : white
              : null,
        );
        // Knit beanie with a pom-pom.
        cap(c, hex("#2f6fb8"), 2, r, hex("#245a96"));
        const top = c.face("hat", "top");
        for (const [x, y] of [
          [3, 3],
          [4, 3],
          [3, 4],
          [4, 4],
        ] as const)
          putPx(c.p, top.x + x, top.y + y, white);
        // Mittens and fur boots.
        around(c, ARMS, (_x, y) => (y >= 10 ? red : null));
        around(c, LEGS, (_x, y) => (y === 9 ? hex("#e8dccb") : null));
        // Rosy cheeks.
        const f = c.face("head", "front");
        putPx(c.p, f.x + 1, f.y + 5, hex("#e89a8a"));
        putPx(c.p, f.x + 6, f.y + 5, hex("#e89a8a"));
      },
    ),
  },
  {
    id: "spring",
    model: "slim",
    draw: person(
      "slim",
      {
        skin: "#f4cfb0",
        hair: "#e8c45a",
        hairStyle: "long",
        eyes: "#4a9a5a",
        shirt: "#fff3b0",
        longSleeves: false,
        pants: "#f4cfb0",
        shoes: "#7ac06a",
        seed: 116,
      },
      (c, r) => {
        // Floral dress.
        const dot: FacePainter = (x, y) =>
          (x * 3 + y * 5) % 7 === 0
            ? hex("#ff8fb0")
            : (x * 5 + y * 3) % 11 === 0
              ? hex("#8ad0ff")
              : null;
        around(c, ["body"], dot);
        for (const id of ["rPants", "lPants"]) {
          paintBox(
            c.p,
            c.box(id),
            (x, y, f) => (y < 5 ? (dot(x, y, f) ?? grain(hex("#fff3b0"), r, 6)) : null),
            [...SIDES],
          );
        }
        // Green cardigan sleeves.
        around(c, ARMS, (_x, y) => (y < 4 ? hex("#9ad88a") : null));
        // Flower in the hair.
        const hs = c.face("hat", "right");
        for (const [x, y, col] of [
          [4, 1, "#ff6f9a"],
          [3, 2, "#ff6f9a"],
          [5, 2, "#ff6f9a"],
          [4, 3, "#ff6f9a"],
          [4, 2, "#ffe36a"],
        ] as const) {
          putPx(c.p, hs.x + x, hs.y + y, hex(col));
        }
      },
    ),
  },
  {
    id: "summer",
    model: "classic",
    draw: person(
      "classic",
      {
        skin: "#d79a6c",
        hair: "#2a1a10",
        hairStyle: "short",
        eyes: "#2a2a2a",
        shirt: "#18a7a0",
        longSleeves: false,
        pants: "#c9b07a",
        shoes: "#e65a2a",
        seed: 117,
      },
      (c, r) => {
        const skin = hex("#d79a6c");
        // Hawaiian shirt.
        const flower: FacePainter = (x, y) =>
          (x + y * 3) % 6 === 0 ? hex("#ffb23a") : (x * 2 + y) % 7 === 3 ? hex("#ff5a7a") : null;
        around(c, ["body"], flower);
        around(c, ARMS, (x, y, f) => (y < 4 ? flower(x, y, f) : null));
        paintFace(c.p, c.face("body", "front"), (x, y) =>
          (x === 3 || x === 4) && y < 3 ? skin : null,
        );
        // Shorts, bare legs, flip-flops.
        bareLegs(c, skin, 6, 11, r);
        around(c, LEGS, (_x, y) => (y === 11 ? hex("#e65a2a") : null));
        // Sunglasses.
        const hf = c.face("hat", "front");
        for (let x = 0; x < 8; x++)
          putPx(c.p, hf.x + x, hf.y + 4, x === 3 || x === 4 ? hex("#1a1a1a") : hex("#111827"));
        for (const x of [1, 2, 5, 6]) putPx(c.p, hf.x + x, hf.y + 3, hex("#1a1a1a"));
      },
    ),
  },
  {
    id: "autumn",
    model: "classic",
    draw: person(
      "classic",
      {
        skin: "#eab896",
        hair: "#a8401c",
        hairStyle: "short",
        eyes: "#5a7a2a",
        shirt: "#e3a21a",
        longSleeves: true,
        pants: "#5a4030",
        shoes: "#3f7a3a",
        seed: 118,
      },
      (c, r) => {
        // Raincoat with a hood and toggles.
        hood(c, hex("#e3a21a"), r);
        paintFace(c.p, c.face("body", "front"), (x, y) =>
          x === 4 ? hex("#b47f10") : x === 5 && y % 3 === 1 ? hex("#5a3a1a") : null,
        );
        // Knitted orange scarf.
        around(c, ["jacket"], (x, y) =>
          y < 2 ? ((x + y) % 2 ? hex("#d8601e") : hex("#b84a14")) : null,
        );
        // A falling leaf on the coat.
        const j = c.face("jacket", "front");
        for (const [x, y] of [
          [1, 6],
          [2, 6],
          [1, 7],
          [2, 5],
        ] as const)
          putPx(c.p, j.x + x, j.y + y, hex("#c2321a"));
        // Rubber boots.
        around(c, LEGS, (_x, y) => (y >= 7 ? grain(hex("#3f7a3a"), r, 8) : null));
      },
    ),
  },
  // ------------------------------------------------------------ the world
  {
    id: "miner",
    model: "classic",
    draw: person(
      "classic",
      {
        skin: "#d8a27e",
        hair: "#3a2a1e",
        hairStyle: "short",
        eyes: "#3a5a7a",
        shirt: "#4a4f57",
        longSleeves: true,
        pants: "#6a4a30",
        shoes: "#2e2218",
        seed: 119,
      },
      (c, r) => {
        // Helmet with a headlamp.
        cap(c, hex("#e0b020"), 2, r, hex("#a07a10"));
        const hf = c.face("hat", "front");
        for (const [x, y, col] of [
          [3, 0, "#fff8c0"],
          [4, 0, "#fff8c0"],
          [3, 1, "#ffe066"],
          [4, 1, "#ffe066"],
          [2, 1, "#555555"],
          [5, 1, "#555555"],
        ] as const) {
          putPx(c.p, hf.x + x, hf.y + y, hex(col));
        }
        // Soot on the face.
        const f = c.face("head", "front");
        putPx(c.p, f.x + 1, f.y + 6, hex("#8a6a52"));
        putPx(c.p, f.x + 6, f.y + 5, hex("#8a6a52"));
        // Suspenders and a lamp battery belt.
        paintFace(c.p, c.face("body", "front"), (x, y) =>
          (x === 2 || x === 5) && y < 9 ? hex("#2a1a10") : y === 9 ? hex("#1e1e1e") : null,
        );
        paintFace(c.p, c.face("body", "back"), (x, y) =>
          y === 9 ? hex("#1e1e1e") : x > 2 && x < 5 && y > 6 && y < 9 ? hex("#3a3a3a") : null,
        );
        // Dusty knees and gloves.
        around(c, LEGS, (x, y) => (y === 6 && x % 2 ? hex("#8a7a6a") : null));
        around(c, ARMS, (_x, y) => (y >= 10 ? hex("#7a5a3a") : null));
      },
    ),
  },
  {
    id: "lumberjack",
    model: "classic",
    draw: person(
      "classic",
      {
        skin: "#e0a880",
        hair: "#6a3a18",
        hairStyle: "short",
        eyes: "#3a5a2a",
        shirt: "#c21d1d",
        longSleeves: true,
        pants: "#355a8a",
        shoes: "#4a2e18",
        seed: 120,
      },
      (c, r) => {
        // Buffalo check shirt.
        around(c, ["body", ...ARMS], (x, y) =>
          y < 10 ? check(hex("#c21d1d"), hex("#1a1414"), r)(x, y, c.face("body", "front")) : null,
        );
        // Full beard.
        const f = c.face("head", "front");
        const beard = hex("#6a3a18");
        for (let y = 5; y < 8; y++)
          for (let x = 0; x < 8; x++) {
            if (y === 6 && x > 2 && x < 5) continue;
            putPx(c.p, f.x + x, f.y + y, grain(beard, r, 10));
          }
        putPx(c.p, f.x + 3, f.y + 6, hex("#8a3a2a"));
        putPx(c.p, f.x + 4, f.y + 6, hex("#8a3a2a"));
        // Green beanie.
        cap(c, hex("#2f5a2a"), 2, r, hex("#244a20"));
        // Rolled cuffs and work boots.
        around(c, LEGS, (_x, y) =>
          y === 9 ? hex("#5a7ab0") : y >= 10 ? grain(hex("#4a2e18"), r, 8) : null,
        );
        around(c, ARMS, (_x, y) => (y >= 10 ? hex("#e0a880") : null));
      },
    ),
  },
];

/** Smooth value noise for capes. */
function noise(seed: number): (x: number, y: number) => number {
  const r = rng(seed);
  const grid = Array.from({ length: 64 }, () => r());
  const at = (x: number, y: number) => grid[((y & 7) * 8 + (x & 7)) & 63]!;
  return (x, y) => {
    const x0 = Math.floor(x);
    const y0 = Math.floor(y);
    const fx = x - x0;
    const fy = y - y0;
    const top = at(x0, y0) * (1 - fx) + at(x0 + 1, y0) * fx;
    const bot = at(x0, y0 + 1) * (1 - fx) + at(x0 + 1, y0 + 1) * fx;
    return top * (1 - fy) + bot * fy;
  };
}

/** A pixel-art stamp centred on the cape. */
function stamp(
  art: string[],
  colors: Record<string, string>,
  x: number,
  y: number,
  w: number,
  h: number,
  yOff = 0,
): Color | null {
  const gx = x - Math.floor((w - (art[0]?.length ?? 0)) / 2);
  const gy = y - Math.floor((h - art.length) / 2) - yOff;
  const ch = art[gy]?.[gx];
  return ch && colors[ch] ? hex(colors[ch]) : null;
}

const SNOWFLAKE = ["...#...", ".#.#.#.", "..###..", "#######", "..###..", ".#.#.#.", "...#..."];

const STAR = ["...#...", "...#...", "..###..", "#######", ".#####.", ".##.##.", "##...##"];

const FEATHER = [
  "...o..",
  "..oyo.",
  "..oyo.",
  ".oyyo.",
  ".oyyo.",
  ".oyyo.",
  "oyyyo.",
  "oyyo..",
  ".oo...",
  "..y...",
];

const RUNES = [
  ["#.#", "###", "#.#"],
  ["##.", "#.#", "##."],
  [".#.", "###", ".#."],
  ["#..", "###", "..#"],
  ["###", ".#.", "#.#"],
];

export const LIBRARY_CAPES: CapePreset[] = [
  {
    id: "ocean",
    draw: () => {
      const r = rng(201);
      return drawCape({
        lining: hex("#0b2a4a"),
        front: (x, y, _w, h) => {
          const crest = Math.round(Math.sin(x * 0.9 + y * 0.6) * 1.2);
          if ((y + crest) % 4 === 0 && y > 2) return hex("#e8f8ff");
          return grain(mix(hex("#4fc3e8"), hex("#0b2a6a"), y / h), r, 8);
        },
      });
    },
  },
  {
    id: "lava",
    draw: () => {
      const n = noise(202);
      const r = rng(203);
      return drawCape({
        lining: hex("#1a0a06"),
        front: (x, y) => {
          const v = n(x * 0.7, y * 0.5);
          if (v > 0.62) return mix(hex("#ff5a0a"), hex("#ffd23a"), (v - 0.62) * 2.6);
          if (v > 0.56) return hex("#b8200a");
          return grain(hex("#2a1a16"), r, 12);
        },
      });
    },
  },
  {
    id: "ore",
    draw: () => {
      const r = rng(204);
      const spots = [
        [2, 3],
        [6, 5],
        [3, 10],
        [7, 12],
        [5, 8],
      ];
      return drawCape({
        lining: hex("#5a5a5a"),
        front: (x, y) => {
          for (const [sx, sy] of spots) {
            const d = Math.abs(x - sx!) + Math.abs(y - sy!);
            if (d === 0) return hex("#d8fbff");
            if (d === 1 && (x + y) % 2) return hex("#3ed8e0");
          }
          return grain(hex("#7f7f7f"), r, 26);
        },
      });
    },
  },
  {
    id: "sakura",
    draw: () => {
      const r = rng(205);
      return drawCape({
        lining: hex("#f6c6d6"),
        front: (x, y, w, h) => {
          const branch = Math.round(w - 1 - y * 0.55);
          if (x === branch || (y === 6 && x > branch - 3 && x < branch)) return hex("#5a3a2a");
          const blossom = (x * 7 + y * 13) % 9 === 0 || (Math.abs(x - branch) <= 1 && r() > 0.5);
          if (blossom) return r() > 0.3 ? hex("#ff9ac0") : hex("#fff0f6");
          return mix(hex("#dff2ff"), hex("#ffe4ee"), y / h);
        },
      });
    },
  },
  {
    id: "autumnLeaves",
    draw: () => {
      const r = rng(206);
      const leaves = ["#e05a1a", "#c2321a", "#f2b22a", "#9a4a1a"].map(hex);
      return drawCape({
        lining: hex("#4a2a16"),
        front: (x, y) =>
          (x * 5 + y * 3) % 4 === 0 || r() > 0.8
            ? leaves[Math.floor(r() * leaves.length)]!
            : grain(hex("#6a3e1e"), r, 10),
      });
    },
  },
  {
    id: "snowflake",
    draw: () => {
      const r = rng(207);
      return drawCape({
        lining: hex("#1a2a5a"),
        front: (x, y, w, h) =>
          stamp(SNOWFLAKE, { "#": "#ffffff" }, x, y, w, h) ??
          ((x * 7 + y * 3) % 13 === 0
            ? hex("#cfe8ff")
            : grain(mix(hex("#1a2a5a"), hex("#2f4f9a"), y / h), r, 6)),
      });
    },
  },
  {
    id: "sun",
    draw: () =>
      drawCape({
        lining: hex("#2a8ad0"),
        front: (x, y, w, h) => {
          const dx = x + 0.5 - w / 2;
          const dy = y + 0.5 - h * 0.4;
          const d = Math.hypot(dx, dy);
          if (d < 2.6) return hex("#ffe14a");
          if (d < 3.3) return hex("#ffb52a");
          const ray = Math.abs((Math.atan2(dy, dx) * 4) / Math.PI) % 1 < 0.25;
          if (ray && d < 5.2) return hex("#ffd84a");
          return mix(hex("#7fd0ff"), hex("#2a8ad0"), y / h);
        },
      }),
  },
  {
    id: "dragon",
    draw: () =>
      drawCape({
        lining: hex("#123a1a"),
        front: (x, y, _w, h) => {
          const sx = (x + (Math.floor(y / 2) % 2)) % 2;
          const base = mix(hex("#3fbf4a"), hex("#145a20"), y / h);
          return y % 2 === 1 && sx === 0 ? shade(base, -40) : sx ? shade(base, 18) : base;
        },
      }),
  },
  {
    id: "runes",
    draw: () => {
      const r = rng(209);
      return drawCape({
        lining: hex("#1a0e2e"),
        front: (x, y) => {
          const rune = RUNES[Math.floor(y / 4) % RUNES.length]!;
          const col = Math.floor(x / 5);
          const lx = x - col * 5 - 1;
          const ly = (y % 4) - 0;
          const on = lx >= 0 && lx < 3 && ly < 3 && rune[(ly + col) % 3]?.[lx] === "#";
          return on ? hex("#c08aff") : grain(hex("#2a1646"), r, 8);
        },
      });
    },
  },
  {
    id: "phoenix",
    draw: () =>
      drawCape({
        lining: hex("#5a0e0a"),
        front: (x, y, w, h) =>
          stamp(FEATHER, { o: "#ffd23a", y: "#ff8a1a" }, x, y, w, h) ??
          mix(hex("#d81e1e"), hex("#ff7a1a"), y / h),
      }),
  },
  {
    id: "racing",
    draw: () =>
      drawCape({
        lining: hex("#1a1a1a"),
        front: (x, y, _w, h) => {
          if (y >= h - 2) return (x + y) % 2 ? hex("#111111") : hex("#fafafa");
          if (x === 3 || x === 6) return hex("#e01e2a");
          if (x === 4 || x === 5) return hex("#fafafa");
          return hex("#1f4fd0");
        },
      }),
  },
  {
    id: "champion",
    draw: () =>
      drawCape({
        lining: hex("#0f1f5a"),
        front: (x, y, w, h) => {
          const star = stamp(STAR, { "#": "#ffd23a" }, x, y, w, h, -2);
          if (star) return star;
          if ((x === 1 || x === w - 2) && y > 4 && y < 13 && y % 2 === 0) return hex("#7fbf3a");
          if (y === h - 3 && x > 1 && x < w - 2) return hex("#ffd23a");
          return mix(hex("#1f3fa0"), hex("#0f1f5a"), y / h);
        },
      }),
  },
  {
    id: "moon",
    draw: () => {
      const r = rng(213);
      return drawCape({
        lining: hex("#0a0e24"),
        front: (x, y, w) => {
          const d = Math.hypot(x + 0.5 - w / 2, y + 0.5 - 5);
          if (d < 3.2) return (x * 3 + y * 5) % 7 === 0 ? hex("#c8c8b8") : hex("#f2f0dc");
          if (d < 3.9) return hex("#3a3e5a");
          return r() > 0.93 ? hex("#ffffff") : hex("#0f1638");
        },
      });
    },
  },
  {
    id: "circuit",
    draw: () => {
      const r = rng(214);
      return drawCape({
        lining: hex("#0a2a16"),
        front: (x, y) => {
          if ((y % 4 === 1 && x % 7 !== 3) || (x % 3 === 1 && y % 5 === 2)) return hex("#d8b03a");
          if (x % 3 === 1 && y % 4 === 1) return hex("#f2e08a");
          return grain(hex("#0f4a24"), r, 8);
        },
      });
    },
  },
  {
    id: "camo",
    draw: () => {
      const n = noise(215);
      const m = noise(216);
      return drawCape({
        lining: hex("#3a3a24"),
        front: (x, y) => {
          const a = n(x * 0.45, y * 0.45);
          const b = m(x * 0.45 + 3, y * 0.45);
          if (a > 0.62) return hex("#2e3a1e");
          if (b > 0.6) return hex("#7a6a3e");
          if (a < 0.32) return hex("#4f5f2a");
          return hex("#6a7a3a");
        },
      });
    },
  },
];
