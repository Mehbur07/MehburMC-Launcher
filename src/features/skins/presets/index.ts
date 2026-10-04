// Original MehburMC preset skins and capes, drawn in code (no third-party art).

import type { SkinModel } from "../../../lib/ipc/bindings/SkinModel";
import type { Pixels } from "../editor/ops";
import { putPx } from "../editor/ops";
import {
  type Color,
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

export interface SkinPreset {
  id: string;
  model: SkinModel;
  draw: () => Pixels;
}

export interface CapePreset {
  id: string;
  draw: () => Pixels;
}

const LIMBS = ["rArm", "lArm", "rLeg", "lLeg"];

function person(model: SkinModel, paint: (c: SkinCanvas) => void): () => Pixels {
  return () => {
    const c = skinCanvas(model);
    paint(c);
    return c.p;
  };
}

/** Metal plating with darker edges, used by the knight and the robot. */
function plate(
  base: Color,
  r: () => number,
): (x: number, y: number, f: { w: number; h: number }) => Color {
  return (x, y, f) => {
    const edge = x === 0 || y === 0 || x === f.w - 1 || y === f.h - 1;
    return grain(shade(base, edge ? -28 : y < 2 ? 14 : 0), r, 8);
  };
}

/** 6×5 "M" for the chest of the MehburMC skin. */
const CHEST_M = ["#....#", "##..##", "#.##.#", "#....#", "#....#"];

export const SKIN_PRESETS: SkinPreset[] = [
  {
    // Launcher mascot; matches the "mehbur" cape and ships in every library.
    id: "mehbur",
    model: "classic",
    draw: person("classic", (c) => {
      drawPerson(c, {
        skin: "#e0ac85",
        hair: "#1c1c24",
        hairStyle: "short",
        eyes: "#22d3ee",
        shirt: "#0f1b33",
        longSleeves: true,
        pants: "#121419",
        shoes: "#f2f2f2",
        seed: 12,
      });
      const r = rng(121);
      const cyan = hex("#22d3ee");
      const deep = hex("#0a1224");
      // Jacket: logo, collar and hem.
      paintFace(c.p, c.face("body", "front"), (x, y) =>
        y >= 2 && y < 7 && x >= 1 && x < 7 && CHEST_M[y - 2]?.[x - 1] === "#"
          ? cyan
          : y === 0 && (x < 2 || x > 5)
            ? cyan
            : y === 11
              ? cyan
              : null,
      );
      paintFace(c.p, c.face("body", "back"), (x, y) =>
        y === 11 ? cyan : y > 1 && y < 10 && x > 0 && x < 7 ? grain(deep, r, 6) : null,
      );
      for (const side of ["left", "right"] as const) {
        paintFace(c.p, c.face("body", side), (_x, y) => (y === 11 ? cyan : null));
      }
      // Sleeves: cyan stripe on the outside, cyan cuffs.
      paintFace(c.p, c.face("rArm", "right"), (x, y) => (x === 1 && y < 9 ? cyan : null));
      paintFace(c.p, c.face("lArm", "left"), (x, y) => (x === 2 && y < 9 ? cyan : null));
      for (const id of ["rArm", "lArm"]) {
        paintBox(c.p, c.box(id), (_x, y, f) =>
          f.name !== "top" && f.name !== "bottom" && y === 9 ? cyan : null,
        );
      }
      // Trousers: side stripes; shoes with cyan soles.
      paintFace(c.p, c.face("rLeg", "right"), (x, y) => (x === 1 && y < 10 ? cyan : null));
      paintFace(c.p, c.face("lLeg", "left"), (x, y) => (x === 2 && y < 10 ? cyan : null));
      for (const id of ["rLeg", "lLeg"]) {
        paintBox(c.p, c.box(id), (_x, y, f) =>
          f.name !== "top" && f.name !== "bottom" && y === 11 ? cyan : null,
        );
      }
      // Headset on the hat layer: band over the top, ear cups with a glow.
      const band = hex("#16181f");
      paintFace(c.p, c.face("hat", "top"), (_x, y) => (y === 3 || y === 4 ? band : null));
      for (const side of ["right", "left"] as const) {
        paintFace(c.p, c.face("hat", side), (x, y) => {
          if (y < 3 && (x === 3 || x === 4)) return band;
          if (y >= 3 && y < 6 && x >= 2 && x < 6) {
            return y === 4 && (x === 3 || x === 4) ? cyan : band;
          }
          return null;
        });
      }
    }),
  },
  {
    id: "explorer",
    model: "classic",
    draw: person("classic", (c) => {
      drawPerson(c, {
        skin: "#c98e66",
        hair: "#4a2d17",
        hairStyle: "short",
        eyes: "#3b6ea8",
        shirt: "#5c7a3a",
        longSleeves: false,
        pants: "#b49a6a",
        shoes: "#4b3221",
        seed: 1,
      });
      const r = rng(11);
      // Hat with a brim on the overlay layer.
      const hat = hex("#8a6a3e");
      paintFace(c.p, c.face("hat", "top"), () => grain(hat, r));
      for (const side of ["front", "back", "left", "right"] as const) {
        paintFace(c.p, c.face("hat", side), (_x, y) =>
          y < 2 ? grain(hat, r) : y === 2 ? hex("#3d2a16") : null,
        );
      }
      // Backpack straps and belt.
      const strap = hex("#3d2a16");
      paintFace(c.p, c.face("body", "front"), (x, y) =>
        (x === 1 || x === 6) && y < 9 ? strap : y === 8 ? hex("#2a1d10") : null,
      );
      paintFace(c.p, c.face("body", "back"), (x, y) =>
        x > 0 && x < 7 && y > 1 && y < 10 ? grain(hex("#7b5530"), r, 12) : null,
      );
    }),
  },
  {
    id: "knight",
    model: "classic",
    draw: person("classic", (c) => {
      const r = rng(2);
      const steel = hex("#9aa3ad");
      for (const id of ["head", "body", ...LIMBS]) paintBox(c.p, c.box(id), plate(steel, r));
      // Helmet visor slit and breathing holes.
      paintFace(c.p, c.face("head", "front"), (x, y) =>
        y === 4 && x > 0 && x < 7 ? hex("#1a1c20") : y === 6 && x % 2 === 1 ? hex("#3a3f46") : null,
      );
      // Red plume on the hat layer.
      paintFace(c.p, c.face("hat", "top"), (x) => (x === 3 || x === 4 ? hex("#c0282d") : null));
      paintFace(c.p, c.face("hat", "back"), (x, y) =>
        (x === 3 || x === 4) && y < 3 ? hex("#a11f24") : null,
      );
      // Tabard with a gold trim.
      const red = hex("#b3242a");
      const gold = hex("#e0b43c");
      for (const side of ["front", "back"] as const) {
        paintFace(c.p, c.face("jacket", side), (x, y) =>
          x > 0 && x < 7 && y < 11
            ? x === 1 || x === 6 || y === 10
              ? gold
              : grain(red, r, 10)
            : null,
        );
      }
    }),
  },
  {
    id: "ninja",
    model: "classic",
    draw: person("classic", (c) => {
      const r = rng(3);
      const black = hex("#1c1c22");
      for (const id of ["head", "body", ...LIMBS]) {
        paintBox(c.p, c.box(id), () => grain(black, r, 8));
      }
      // Eye band.
      const skin = hex("#d6a27c");
      paintFace(c.p, c.face("head", "front"), (x, y) =>
        y === 3 || y === 4 ? (y === 4 && (x === 2 || x === 5) ? hex("#2b2b2b") : skin) : null,
      );
      // Red headband with tails.
      const band = hex("#c4202a");
      for (const side of ["front", "back", "left", "right"] as const) {
        paintFace(c.p, c.face("hat", side), (x, y) =>
          y === 2 ? band : side === "back" && (x === 3 || x === 4) && y > 2 && y < 7 ? band : null,
        );
      }
      // Belt.
      paintBox(c.p, c.box("body"), (_x, y) => (y === 7 ? band : null), [
        "front",
        "back",
        "left",
        "right",
      ]);
    }),
  },
  {
    id: "astronaut",
    model: "classic",
    draw: person("classic", (c) => {
      drawPerson(c, {
        skin: "#e0b08c",
        hair: "#2b1d12",
        hairStyle: "short",
        eyes: "#2d4f2a",
        shirt: "#e8ecf0",
        longSleeves: true,
        pants: "#e8ecf0",
        shoes: "#8a9099",
        seed: 4,
      });
      const r = rng(44);
      const suit = hex("#e8ecf0");
      for (const id of LIMBS) {
        paintBox(c.p, c.box(id), (_x, y) => (y >= 10 ? null : grain(suit, r, 6)));
      }
      paintBox(c.p, c.box("rArm"), (_x, y) => (y >= 10 ? hex("#c8ccd2") : null));
      paintBox(c.p, c.box("lArm"), (_x, y) => (y >= 10 ? hex("#c8ccd2") : null));
      // Helmet: white frame, tinted visor.
      const glass = hex("#1d3557");
      paintBox(c.p, c.box("hat"), (x, y, f) => {
        if (f.name !== "front") return grain(suit, r, 6);
        if (x === 0 || x === 7 || y === 0 || y === 7) return suit;
        return x === 2 && y === 2 ? hex("#a8d0f0") : mix(glass, hex("#3a6ea5"), y / 8);
      });
      // Orange shoulder patches and the chest panel.
      for (const id of ["rArm", "lArm"]) {
        paintFace(c.p, c.face(id, "front"), (_x, y) => (y === 1 ? hex("#f07f22") : null));
      }
      paintFace(c.p, c.face("body", "front"), (x, y) =>
        y > 2 && y < 6 && x > 1 && x < 6
          ? x === 2 && y === 3
            ? hex("#e63946")
            : x === 3 && y === 3
              ? hex("#2a9d8f")
              : hex("#6c757d")
          : null,
      );
      // Life-support backpack.
      paintFace(c.p, c.face("jacket", "back"), (x, y) =>
        x > 0 && x < 7 && y > 0 && y < 10 ? grain(hex("#adb5bd"), r, 10) : null,
      );
    }),
  },
  {
    id: "robot",
    model: "classic",
    draw: person("classic", (c) => {
      const r = rng(5);
      const steel = hex("#7d8a97");
      for (const id of ["head", "body", ...LIMBS]) paintBox(c.p, c.box(id), plate(steel, r));
      const glow = hex("#3df2ff");
      paintFace(c.p, c.face("head", "front"), (x, y) =>
        y === 3 && (x === 1 || x === 2 || x === 5 || x === 6)
          ? glow
          : y === 6 && x > 1 && x < 6
            ? x % 2
              ? hex("#2b3138")
              : hex("#aab4be")
            : null,
      );
      // Antenna on the hat layer.
      paintFace(c.p, c.face("hat", "top"), (x, y) => (x === 4 && y === 4 ? hex("#ff4d4d") : null));
      // Chest lights.
      paintFace(c.p, c.face("body", "front"), (x, y) =>
        y > 2 && y < 7 && x > 1 && x < 6
          ? y === 3 && x < 5
            ? ([hex("#ff4d4d"), hex("#ffd166"), hex("#06d6a0")][x - 2] ?? null)
            : hex("#2b3138")
          : null,
      );
      paintFace(c.p, c.face("jacket", "front"), (x, y) =>
        y === 5 && x > 2 && x < 5 ? glow : null,
      );
    }),
  },
  {
    id: "hoodieBlue",
    model: "slim",
    draw: person("slim", (c) => {
      drawPerson(c, {
        skin: "#f1c6a5",
        hair: "#5a3620",
        hairStyle: "long",
        eyes: "#2f7d4f",
        shirt: "#2f6fd6",
        longSleeves: true,
        pants: "#253447",
        shoes: "#f4f4f4",
        seed: 6,
      });
      hood(c, hex("#2f6fd6"));
    }),
  },
  {
    id: "hoodiePink",
    model: "slim",
    draw: person("slim", (c) => {
      drawPerson(c, {
        skin: "#f6d3b8",
        hair: "#e8c56d",
        hairStyle: "long",
        eyes: "#4a7bd0",
        shirt: "#ec6fa8",
        longSleeves: true,
        pants: "#f2f2f2",
        shoes: "#ec6fa8",
        seed: 7,
      });
      hood(c, hex("#ec6fa8"));
    }),
  },
  {
    id: "pirate",
    model: "classic",
    draw: person("classic", (c) => {
      drawPerson(c, {
        skin: "#c68a5e",
        hair: "#1e1410",
        hairStyle: "short",
        eyes: "#3b2a1a",
        shirt: "#f2efe6",
        longSleeves: true,
        pants: "#5a3b22",
        shoes: "#1b1b1b",
        seed: 8,
      });
      const r = rng(88);
      // Striped shirt.
      paintBox(c.p, c.box("body"), (_x, y) => (y % 3 === 1 ? hex("#1f3a68") : null), [
        "front",
        "back",
        "left",
        "right",
      ]);
      // Bandana, eye patch and beard.
      const red = hex("#b3202a");
      paintBox(c.p, c.box("hat"), (x, y, f) =>
        f.name === "top" || y < 2 || (f.name === "back" && y < 4 && (x === 3 || x === 4))
          ? grain(red, r, 12)
          : null,
      );
      paintFace(c.p, c.face("head", "front"), (x, y) =>
        (y === 4 && (x === 5 || x === 6)) || (y === 3 && x > 2)
          ? hex("#121212")
          : y === 7 || (y === 6 && (x < 3 || x > 4))
            ? hex("#2a1a10")
            : null,
      );
      // Belt with a gold buckle.
      paintFace(c.p, c.face("body", "front"), (x, y) =>
        y === 9 ? (x === 3 || x === 4 ? hex("#e0b43c") : hex("#2a1a10")) : null,
      );
    }),
  },
  {
    id: "wizard",
    model: "classic",
    draw: person("classic", (c) => {
      drawPerson(c, {
        skin: "#e7bf9c",
        hair: "#d9d9d9",
        hairStyle: "long",
        eyes: "#5a3a8a",
        shirt: "#4b2a7b",
        longSleeves: true,
        pants: "#4b2a7b",
        shoes: "#2a1640",
        seed: 9,
      });
      const r = rng(99);
      const robe = hex("#4b2a7b");
      const star = hex("#ffd966");
      // Long white beard.
      paintFace(c.p, c.face("head", "front"), (x, y) =>
        y >= 5 && x > 0 && x < 7 ? grain(hex("#ededed"), r, 10) : null,
      );
      paintFace(c.p, c.face("body", "front"), (x, y) =>
        y < 5 && x > 1 && x < 6 ? grain(hex("#ededed"), r, 10) : null,
      );
      // Hood with stars, and a robe over the legs.
      paintBox(c.p, c.box("hat"), (x, y, f) => {
        if (f.name === "front") return y < 2 || x === 0 || x === 7 ? grain(robe, r, 10) : null;
        if (f.name === "bottom") return null;
        return (x * 7 + y * 3) % 11 === 0 ? star : grain(robe, r, 10);
      });
      for (const id of ["rPants", "lPants"]) {
        paintBox(c.p, c.box(id), (_x, y, f) =>
          f.name === "top" || f.name === "bottom" || y > 8 ? null : grain(robe, r, 10),
        );
      }
      paintFace(c.p, c.face("jacket", "front"), (x, y) =>
        y === 8 && x > 0 && x < 7 ? star : null,
      );
    }),
  },
  {
    id: "cyber",
    model: "slim",
    draw: person("slim", (c) => {
      drawPerson(c, {
        skin: "#b98463",
        hair: "#14141a",
        hairStyle: "short",
        eyes: "#14141a",
        shirt: "#16161d",
        longSleeves: true,
        pants: "#16161d",
        shoes: "#16161d",
        seed: 10,
      });
      const neon = hex("#39ff88");
      // Visor and neon circuit lines.
      paintFace(c.p, c.face("hat", "front"), (x, y) => (y === 4 && x > 0 && x < 7 ? neon : null));
      paintFace(c.p, c.face("hat", "right"), (x, y) => (y === 4 && x > 4 ? neon : null));
      paintFace(c.p, c.face("hat", "left"), (x, y) => (y === 4 && x < 3 ? neon : null));
      paintBox(c.p, c.box("body"), (x, y, f) =>
        f.name === "front" && (x === 3 || (y === 6 && x > 0)) ? neon : null,
      );
      for (const id of ["rArm", "lArm", "rLeg", "lLeg"]) {
        paintFace(c.p, c.face(id, "front"), (x, y) => (x === 1 && y < 10 ? neon : null));
      }
    }),
  },
  {
    id: "footballer",
    model: "classic",
    draw: person("classic", (c) => {
      drawPerson(c, {
        skin: "#d9a37f",
        hair: "#1f140d",
        hairStyle: "short",
        eyes: "#3b2a1a",
        shirt: "#d71920",
        longSleeves: false,
        pants: "#f5f5f5",
        shoes: "#151515",
        seed: 11,
      });
      const white = hex("#f5f5f5");
      const red = hex("#d71920");
      // Shorts end at the knee, then red socks.
      for (const id of ["rLeg", "lLeg"]) {
        paintBox(c.p, c.box(id), (_x, y, f) =>
          f.name === "top" || f.name === "bottom" ? null : y >= 5 && y < 10 ? red : null,
        );
        paintBox(c.p, c.box(id), (_x, y) => (y >= 5 && y < 7 ? hex("#d9a37f") : null), [
          "front",
          "back",
          "left",
          "right",
        ]);
      }
      // White collar stripe and number 7 on the back.
      paintFace(c.p, c.face("body", "front"), (x, y) => (y === 0 && x > 2 && x < 5 ? white : null));
      const seven = ["####", "...#", "..#.", ".#..", ".#..", ".#.."];
      paintFace(c.p, c.face("body", "back"), (x, y) =>
        y > 1 && y < 8 && x > 1 && x < 6 && seven[y - 2]?.[x - 2] === "#" ? white : null,
      );
    }),
  },
];

/** A hood on the overlay layer that frames the face. */
function hood(c: SkinCanvas, color: Color) {
  const r = rng(77);
  paintBox(c.p, c.box("hat"), (x, y, f) => {
    if (f.name === "bottom") return null;
    if (f.name === "front") return y === 0 || x === 0 || x === 7 ? grain(color, r, 8) : null;
    return grain(shade(color, -6), r, 8);
  });
  // Drawstrings.
  const front = c.face("body", "front");
  putPx(c.p, front.x + 3, front.y + 1, hex("#f4f4f4"));
  putPx(c.p, front.x + 4, front.y + 1, hex("#f4f4f4"));
  putPx(c.p, front.x + 3, front.y + 2, hex("#f4f4f4"));
  putPx(c.p, front.x + 4, front.y + 2, hex("#f4f4f4"));
  // Front pocket.
  paintFace(c.p, front, (x, y) => (y >= 7 && y < 10 && x > 0 && x < 7 ? shade(color, -22) : null));
}

/** Crescent and star, hand-placed for the 10 px wide cape face. */
const CRESCENT = [
  "..####....",
  ".##.......",
  "##......#.",
  "##.....###",
  "##......#.",
  ".##.......",
  "..####....",
];

const crescentFlag = (x: number, y: number, w: number, h: number): Color => {
  const gx = x - Math.floor((w - 10) / 2);
  const gy = y - Math.floor((h - CRESCENT.length) / 2);
  const on = gy >= 0 && gy < CRESCENT.length && gx >= 0 && gx < 10 && CRESCENT[gy]?.[gx] === "#";
  return on ? hex("#ffffff") : hex("#e30a17");
};

const M_GLYPH = ["#...#", "##.##", "#.#.#", "#...#", "#...#"];

export const CAPE_PRESETS: CapePreset[] = [
  {
    id: "mehbur",
    draw: () =>
      drawCape({
        lining: hex("#0d1b2a"),
        front: (x, y, w, h) => {
          const cyan = hex("#22d3ee");
          if (x === 0 || x === w - 1 || y === 0 || y === h - 1) return cyan;
          const gx = x - Math.floor((w - 5) / 2);
          const gy = y - Math.floor((h - 5) / 2);
          if (gx >= 0 && gx < 5 && gy >= 0 && gy < 5 && M_GLYPH[gy]?.[gx] === "#") return cyan;
          return mix(hex("#1b2a4a"), hex("#0b1020"), y / h);
        },
      }),
  },
  {
    id: "turkey",
    draw: () => drawCape({ lining: hex("#b0070f"), front: crescentFlag }),
  },
  {
    id: "sunset",
    draw: () =>
      drawCape({
        lining: hex("#3a1c5c"),
        front: (_x, y, _w, h) => {
          const t = y / (h - 1);
          return t < 0.5
            ? mix(hex("#5b2a86"), hex("#e2557a"), t * 2)
            : mix(hex("#e2557a"), hex("#ffb347"), (t - 0.5) * 2);
        },
      }),
  },
  {
    id: "flame",
    draw: () => {
      const r = rng(21);
      return drawCape({
        lining: hex("#3a0a06"),
        front: (x, y, _w, h) => {
          const t = y / (h - 1);
          const wave = Math.sin(x * 1.7) * 0.12 + (r() - 0.5) * 0.15;
          const heat = Math.max(0, Math.min(1, t + wave - 0.15));
          return heat < 0.35
            ? mix(hex("#3a0a06"), hex("#b3200e"), heat / 0.35)
            : heat < 0.7
              ? mix(hex("#b3200e"), hex("#ff7b00"), (heat - 0.35) / 0.35)
              : mix(hex("#ff7b00"), hex("#ffe066"), (heat - 0.7) / 0.3);
        },
      });
    },
  },
  {
    id: "galaxy",
    draw: () => {
      const r = rng(31);
      return drawCape({
        lining: hex("#0b0820"),
        front: (x, y, _w, h) => {
          const nebula = Math.sin(x * 0.6 + y * 0.35) * 0.5 + 0.5;
          const base = mix(
            hex("#0b0820"),
            hex("#3b1f6e"),
            nebula * 0.7 * (1 - Math.abs(y / h - 0.5)),
          );
          const roll = r();
          return roll > 0.94 ? hex("#ffffff") : roll > 0.9 ? hex("#9fd3ff") : base;
        },
      });
    },
  },
  {
    id: "checker",
    draw: () =>
      drawCape({
        lining: hex("#1a1a1a"),
        front: (x, y) =>
          (Math.floor(x / 2) + Math.floor(y / 2)) % 2 ? hex("#f2f2f2") : hex("#1a1a1a"),
      }),
  },
  {
    id: "rainbow",
    draw: () => {
      const colors = ["#e63946", "#f77f00", "#fcbf49", "#2a9d8f", "#3a86ff", "#8338ec"].map(hex);
      return drawCape({
        lining: hex("#222222"),
        front: (_x, y, _w, h) =>
          colors[Math.min(colors.length - 1, Math.floor((y / h) * colors.length))] ?? colors[0]!,
      });
    },
  },
  {
    id: "forest",
    draw: () => {
      const r = rng(41);
      return drawCape({
        lining: hex("#3b2a16"),
        front: (x, y, w, h) =>
          x === 0 || x === w - 1 || y === h - 1
            ? hex("#5a3b1e")
            : grain(r() > 0.75 ? hex("#3f8f3a") : hex("#2d6a2a"), r, 16),
      });
    },
  },
  {
    id: "ice",
    draw: () => {
      const r = rng(51);
      return drawCape({
        lining: hex("#bfe3f5"),
        front: (x, y, _w, h) => {
          const base = mix(hex("#e6f6ff"), hex("#5fb3e6"), y / h);
          return (x + y) % 5 === 0 && r() > 0.4 ? hex("#ffffff") : grain(base, r, 10);
        },
      });
    },
  },
  {
    id: "gold",
    draw: () => {
      const r = rng(61);
      return drawCape({
        lining: hex("#7a5a12"),
        front: (x, y, w, h) => {
          const dx = Math.abs(x + 0.5 - w / 2);
          const dy = Math.abs(y + 0.5 - h / 2);
          if (dx / 3 + dy / 4.5 < 1) return mix(hex("#b9f2ff"), hex("#2bb3c7"), dy / 4.5);
          if (dx / 3 + dy / 4.5 < 1.35) return hex("#7a5a12");
          return grain(mix(hex("#ffd95a"), hex("#c9961a"), y / h), r, 14);
        },
      });
    },
  },
];
