import {
  Brush,
  Eraser,
  FlipHorizontal2,
  Grid3x3,
  PaintBucket,
  Pipette,
  Redo2,
  Save,
  Slash,
  SquareDashed,
  Trash2,
  Undo2,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button, IconButton, TextInput } from "../../../components/ui";
import type { SkinModel } from "../../../lib/ipc/bindings/SkinModel";
import type { TextureKind } from "../../../lib/ipc/bindings/TextureKind";
import { drawCape, drawPerson, hex, skinCanvas } from "../presets/paint";
import { fromDataUri, toCanvas, toDataUri } from "./canvas";
import { type Box, type Layer, boxesFor, faces, locate } from "./layout";
import {
  History,
  type Pixels,
  type Rgba,
  TRANSPARENT,
  blank,
  clearLayer,
  clone,
  floodFill,
  getPx,
  hexToRgba,
  linePoints,
  rgbaToHex,
  stamp,
} from "./ops";

export type Tool = "pencil" | "eraser" | "fill" | "picker" | "line";

export interface EditorSeed {
  /** Changes whenever a new texture is opened, resetting the editor. */
  key: string;
  kind: TextureKind;
  name: string;
  model: SkinModel;
  dataUri: string | null;
}

/** Display pixels per texture pixel (the canvas is scaled down by CSS). */
const CELL = 12;
const PALETTE = [
  "#000000", "#3f3f46", "#71717a", "#a1a1aa", "#e4e4e7", "#ffffff",
  "#7f1d1d", "#dc2626", "#f97316", "#facc15", "#84cc16", "#16a34a",
  "#0e7490", "#06b6d4", "#2563eb", "#4f46e5", "#9333ea", "#db2777",
  "#5b3a29", "#8b5a3c", "#c68a5e", "#e0b08c", "#f6d3b8", "#fde68a",
]; // prettier-ignore
const PART_COLORS: Record<string, string> = {
  head: "#f59e0b",
  body: "#22c55e",
  rightArm: "#3b82f6",
  leftArm: "#a855f7",
  rightLeg: "#ef4444",
  leftLeg: "#14b8a6",
  cape: "#f59e0b",
  elytra: "#3b82f6",
};
const DRAFT_KEY = "mehbur.skinEditor.draft";

interface Draft {
  kind: TextureKind;
  name: string;
  model: SkinModel;
  dataUri: string;
}

function readDraft(kind: TextureKind): Draft | null {
  try {
    const d = JSON.parse(localStorage.getItem(`${DRAFT_KEY}.${kind}`) ?? "null") as Draft | null;
    return d && typeof d.dataUri === "string" && d.dataUri.startsWith("data:image/png") ? d : null;
  } catch {
    return null;
  }
}

function writeDraft(d: Draft | null, kind: TextureKind) {
  try {
    if (d) localStorage.setItem(`${DRAFT_KEY}.${kind}`, JSON.stringify(d));
    else localStorage.removeItem(`${DRAFT_KEY}.${kind}`);
  } catch {
    // Storage may be unavailable; drafts are only a convenience.
  }
}

/** A plain person to start drawing on. */
export function templateSkin(model: SkinModel): Pixels {
  const c = skinCanvas(model);
  drawPerson(c, {
    skin: "#d9a37f",
    hair: "#4a2d17",
    hairStyle: "short",
    eyes: "#3b6ea8",
    shirt: "#3aa8a0",
    longSleeves: false,
    pants: "#3b4a8a",
    shoes: "#5a5a5a",
    seed: 1,
  });
  return c.p;
}

/** A plain cape with a lining, so every face is already opaque. */
export function templateCape(): Pixels {
  return drawCape({ lining: hex("#7f1d1d"), front: () => hex("#dc2626") });
}

function templateFor(kind: TextureKind, model: SkinModel): Pixels {
  return kind === "skin" ? templateSkin(model) : templateCape();
}

function emptyFor(kind: TextureKind): Pixels {
  return kind === "skin" ? blank(64, 64) : blank(64, 32);
}

export function SkinEditor({
  seed,
  onLive,
  onSave,
  outerLayer,
  onOuterLayer,
}: {
  seed: EditorSeed;
  /** Called (throttled) with the current texture for the 3D preview. */
  onLive: (dataUri: string, model: SkinModel) => void;
  onSave: (name: string, dataUri: string, model: SkinModel) => Promise<boolean>;
  outerLayer: boolean;
  onOuterLayer: (v: boolean) => void;
}) {
  const { t } = useTranslation();
  const kind = seed.kind;
  const px = useRef<Pixels>(emptyFor(kind));
  const history = useRef(new History(100));
  const display = useRef<HTMLCanvasElement>(null);
  const [model, setModel] = useState<SkinModel>(seed.model);
  const [name, setName] = useState(seed.name);
  const [tool, setTool] = useState<Tool>("pencil");
  const [color, setColor] = useState("#dc2626");
  const [recent, setRecent] = useState<string[]>([]);
  const [size, setSize] = useState(1);
  const [mirror, setMirror] = useState(false);
  const [layer, setLayer] = useState<Layer | "all">("all");
  const [grid, setGrid] = useState(true);
  const [guides, setGuides] = useState(true);
  const [hover, setHover] = useState<{ x: number; y: number } | null>(null);
  const [tick, setTick] = useState(0);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [ready, setReady] = useState(false);

  const boxes = useMemo(() => boxesFor(kind, model), [kind, model]);
  const scope = useMemo(
    () => ({
      boxes,
      layer: kind === "skin" && layer !== "all" ? layer : null,
      mirror,
    }),
    [boxes, kind, layer, mirror],
  );

  // True once the user changed something; only then is a draft kept.
  const dirty = useRef(false);
  const changed = useCallback(() => {
    dirty.current = true;
    setSaved(false);
    setTick((n) => n + 1);
  }, []);

  // Load the seed (or a draft for a blank start).
  useEffect(() => {
    let alive = true;
    setReady(false);
    history.current.reset();
    setModel(seed.model);
    setName(seed.name);
    const draft = seed.dataUri ? null : readDraft(seed.kind);
    const src = seed.dataUri ?? draft?.dataUri ?? null;
    if (draft) {
      setModel(draft.model);
      setName(draft.name);
    }
    const load = src
      ? fromDataUri(src, seed.kind).catch(() => emptyFor(seed.kind))
      : Promise.resolve(templateFor(seed.kind, seed.model));
    void load.then((p) => {
      if (!alive) return;
      // An empty draft is not worth restoring; start from the template.
      const empty = draft && !p.data.some((v, i) => i % 4 === 3 && v > 0);
      px.current = empty ? templateFor(seed.kind, draft.model) : p;
      dirty.current = false;
      setReady(true);
      setTick((n) => n + 1);
    });
    return () => {
      alive = false;
    };
  }, [seed]);

  // Redraw the zoomed canvas.
  const frame = useRef(0);
  useEffect(() => {
    if (!ready) return;
    cancelAnimationFrame(frame.current);
    frame.current = requestAnimationFrame(() => {
      const c = display.current;
      if (c)
        render(c, px.current, boxes, {
          grid,
          guides,
          hover,
          showOverlay: kind === "cape" || outerLayer,
        });
    });
    return () => cancelAnimationFrame(frame.current);
  }, [tick, ready, boxes, grid, guides, hover, outerLayer, kind]);

  // Push the texture to the 3D preview, at most once per frame.
  const liveFrame = useRef(0);
  useEffect(() => {
    if (!ready) return;
    cancelAnimationFrame(liveFrame.current);
    liveFrame.current = requestAnimationFrame(() => onLive(toDataUri(px.current), model));
    return () => cancelAnimationFrame(liveFrame.current);
  }, [tick, ready, model, onLive]);

  // Persist a draft so an accidental close does not lose the drawing.
  useEffect(() => {
    if (!ready || !dirty.current) return;
    const id = window.setTimeout(
      () => writeDraft({ kind, name, model, dataUri: toDataUri(px.current) }, kind),
      400,
    );
    return () => window.clearTimeout(id);
  }, [tick, ready, kind, name, model]);

  const rgba = (): Rgba => hexToRgba(color);
  const pickColor = (hex: string) => {
    setColor(hex);
    setRecent((r) => [hex, ...r.filter((c) => c !== hex)].slice(0, 12));
  };

  const undo = useCallback(() => {
    if (history.current.undo(px.current)) changed();
  }, [changed]);
  const redo = useCallback(() => {
    if (history.current.redo(px.current)) changed();
  }, [changed]);

  // Keyboard shortcuts while the editor is on screen (not while typing).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement | null;
      if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA")) return;
      const k = e.key.toLowerCase();
      if ((e.ctrlKey || e.metaKey) && k === "z") {
        e.preventDefault();
        if (e.shiftKey) redo();
        else undo();
      } else if ((e.ctrlKey || e.metaKey) && k === "y") {
        e.preventDefault();
        redo();
      } else if (!e.ctrlKey && !e.metaKey && !e.altKey) {
        const map: Record<string, Tool> = {
          b: "pencil",
          e: "eraser",
          g: "fill",
          i: "picker",
          l: "line",
        };
        if (map[k]) setTool(map[k]);
        else if (k === "m") setMirror((m) => !m);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [undo, redo]);

  // Pointer drawing.
  const stroke = useRef<{
    last: [number, number];
    start: [number, number];
    before: Pixels;
    erase: boolean;
  } | null>(null);

  const cellAt = (e: React.PointerEvent<HTMLCanvasElement>): [number, number] => {
    const r = e.currentTarget.getBoundingClientRect();
    const p = px.current;
    const x = Math.floor(((e.clientX - r.left) / r.width) * p.width);
    const y = Math.floor(((e.clientY - r.top) / r.height) * p.height);
    return [Math.min(p.width - 1, Math.max(0, x)), Math.min(p.height - 1, Math.max(0, y))];
  };

  const paint = (x: number, y: number, erase: boolean) =>
    stamp(px.current, x, y, size, erase ? TRANSPARENT : rgba(), scope);

  const onDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (!ready || (e.button !== 0 && e.button !== 2)) return;
    e.preventDefault();
    const [x, y] = cellAt(e);
    const erase = e.button === 2 || tool === "eraser";
    if (tool === "picker" || e.altKey) {
      const c = getPx(px.current, x, y);
      if (c[3] > 0) pickColor(rgbaToHex(c));
      setTool((tl) => (tl === "picker" ? "pencil" : tl));
      return;
    }
    history.current.push(px.current);
    if (tool === "fill") {
      floodFill(px.current, x, y, erase ? TRANSPARENT : rgba(), scope);
      if (!erase) pickColor(color);
      changed();
      return;
    }
    e.currentTarget.setPointerCapture(e.pointerId);
    stroke.current = { last: [x, y], start: [x, y], before: clone(px.current), erase };
    paint(x, y, erase);
    changed();
  };

  const onMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const [x, y] = cellAt(e);
    if (!hover || hover.x !== x || hover.y !== y) setHover({ x, y });
    const s = stroke.current;
    if (!s) return;
    if (tool === "line") {
      px.current.data.set(s.before.data);
      for (const [lx, ly] of linePoints(s.start[0], s.start[1], x, y)) paint(lx, ly, s.erase);
    } else {
      for (const [lx, ly] of linePoints(s.last[0], s.last[1], x, y)) paint(lx, ly, s.erase);
    }
    s.last = [x, y];
    changed();
  };

  const onUp = () => {
    if (stroke.current && !stroke.current.erase) pickColor(color);
    stroke.current = null;
  };

  const replaceAll = (p: Pixels) => {
    history.current.push(px.current);
    px.current = p;
    changed();
  };

  const save = async () => {
    setSaving(true);
    const ok = await onSave(
      name.trim() || t(kind === "skin" ? "skins.skin" : "skins.cape"),
      toDataUri(px.current),
      model,
    );
    setSaving(false);
    if (ok) {
      setSaved(true);
      writeDraft(null, kind);
    }
  };

  const hoverFace = hover ? locate(boxes, hover.x, hover.y) : null;
  const tools: { id: Tool; icon: React.ReactNode; key: string }[] = [
    { id: "pencil", icon: <Brush size={15} />, key: "B" },
    { id: "eraser", icon: <Eraser size={15} />, key: "E" },
    { id: "fill", icon: <PaintBucket size={15} />, key: "G" },
    { id: "picker", icon: <Pipette size={15} />, key: "I" },
    { id: "line", icon: <Slash size={15} />, key: "L" },
  ];

  return (
    <div className="flex flex-col gap-3">
      {/* Toolbar */}
      <div className="flex flex-wrap items-center gap-1.5">
        <div role="radiogroup" aria-label={t("skins.editor.tools")} className="flex gap-0.5">
          {tools.map((tl) => (
            <IconButton
              key={tl.id}
              role="radio"
              aria-checked={tool === tl.id}
              label={`${t(`skins.editor.tool.${tl.id}`)} (${tl.key})`}
              onClick={() => setTool(tl.id)}
              className={tool === tl.id ? "bg-accent/15 text-accent" : ""}
            >
              {tl.icon}
            </IconButton>
          ))}
        </div>
        <span className="mx-1 h-5 w-px bg-line" />
        <label className="flex items-center gap-1 text-xs text-muted">
          {t("skins.editor.size")}
          <select
            value={size}
            onChange={(e) => setSize(Number(e.target.value))}
            className="rounded border border-line bg-surface-2 px-1 py-0.5 text-fg"
          >
            {[1, 2, 3].map((n) => (
              <option key={n} value={n}>
                {n}px
              </option>
            ))}
          </select>
        </label>
        <IconButton
          label={`${t("skins.editor.mirror")} (M)`}
          aria-pressed={mirror}
          onClick={() => setMirror(!mirror)}
          className={mirror ? "bg-accent/15 text-accent" : ""}
        >
          <FlipHorizontal2 size={15} />
        </IconButton>
        <IconButton
          label={t("skins.editor.grid")}
          aria-pressed={grid}
          onClick={() => setGrid(!grid)}
          className={grid ? "text-accent" : ""}
        >
          <Grid3x3 size={15} />
        </IconButton>
        <IconButton
          label={t("skins.editor.guides")}
          aria-pressed={guides}
          onClick={() => setGuides(!guides)}
          className={guides ? "text-accent" : ""}
        >
          <SquareDashed size={15} />
        </IconButton>
        <span className="mx-1 h-5 w-px bg-line" />
        <IconButton label={`${t("skins.editor.undo")} (Ctrl+Z)`} onClick={undo}>
          <Undo2 size={15} />
        </IconButton>
        <IconButton label={`${t("skins.editor.redo")} (Ctrl+Y)`} onClick={redo}>
          <Redo2 size={15} />
        </IconButton>
        <IconButton
          label={t("skins.editor.clear")}
          className="hover:text-danger"
          onClick={() => {
            const p = clone(px.current);
            clearLayer(p, boxes, scope.layer);
            replaceAll(p);
          }}
        >
          <Trash2 size={15} />
        </IconButton>
      </div>

      {kind === "skin" && (
        <div className="flex flex-wrap items-center gap-x-4 gap-y-2 text-xs">
          <div
            role="radiogroup"
            aria-label={t("skins.editor.layer")}
            className="flex items-center gap-1"
          >
            <span className="text-muted">{t("skins.editor.layer")}:</span>
            {(["all", "base", "overlay"] as const).map((l) => (
              <Button
                key={l}
                size="sm"
                role="radio"
                aria-checked={layer === l}
                variant={layer === l ? "secondary" : "ghost"}
                onClick={() => setLayer(l)}
              >
                {t(`skins.editor.layers.${l}`)}
              </Button>
            ))}
          </div>
          <label className="flex items-center gap-1.5 text-muted">
            <input
              type="checkbox"
              checked={outerLayer}
              onChange={(e) => onOuterLayer(e.target.checked)}
              className="accent-[rgb(var(--mc-accent-rgb))]"
            />
            {t("skins.editor.showOverlay")}
          </label>
          <div className="flex items-center gap-1">
            <span className="text-muted">{t("skins.editor.arms")}:</span>
            {(["classic", "slim"] as const).map((m) => (
              <Button
                key={m}
                size="sm"
                variant={model === m ? "secondary" : "ghost"}
                aria-pressed={model === m}
                onClick={() => {
                  setModel(m);
                  changed();
                }}
              >
                {t(`skins.model.${m}`)}
              </Button>
            ))}
          </div>
        </div>
      )}

      {/* Canvas */}
      <div className="rounded-md border border-line bg-surface-3/40 p-2">
        <canvas
          ref={display}
          aria-label={t("skins.editor.canvas")}
          className={`mx-auto block w-full touch-none select-none ${
            tool === "picker" ? "cursor-copy" : "cursor-crosshair"
          }`}
          style={{ maxWidth: kind === "skin" ? 560 : 640, imageRendering: "pixelated" }}
          onPointerDown={onDown}
          onPointerMove={onMove}
          onPointerUp={onUp}
          onPointerCancel={onUp}
          onPointerLeave={() => setHover(null)}
          onContextMenu={(e) => e.preventDefault()}
        />
        <p className="mt-1 h-4 text-center text-[11px] text-muted">
          {hover
            ? `${hover.x}, ${hover.y}${
                hoverFace
                  ? ` · ${t(`skins.editor.part.${hoverFace.box.part}`)} · ${t(
                      `skins.editor.face.${hoverFace.name}`,
                    )}${kind === "skin" ? ` · ${t(`skins.editor.layers.${hoverFace.box.layer}`)}` : ""}`
                  : ` · ${t("skins.editor.unused")}`
              }`
            : t("skins.editor.hint")}
        </p>
      </div>

      {/* Colours */}
      <div className="flex flex-wrap items-start gap-3">
        <label className="flex flex-col items-center gap-1 text-[11px] text-muted">
          <input
            type="color"
            value={color}
            aria-label={t("skins.editor.color")}
            onChange={(e) => setColor(e.target.value)}
            className="h-10 w-12 cursor-pointer rounded border border-line bg-transparent"
          />
          {color}
        </label>
        <div className="flex flex-col gap-1.5">
          <div className="grid grid-cols-12 gap-1">
            {PALETTE.map((c) => (
              <Swatch key={c} color={c} active={c === color} onPick={pickColor} />
            ))}
          </div>
          {recent.length > 0 && (
            <div className="flex items-center gap-1">
              <span className="mr-1 text-[11px] text-muted">{t("skins.editor.recent")}</span>
              {recent.map((c) => (
                <Swatch key={c} color={c} active={c === color} onPick={pickColor} />
              ))}
            </div>
          )}
        </div>
      </div>

      {/* Start from / save */}
      <div className="flex flex-wrap items-center gap-2 border-t border-line pt-3">
        <Button size="sm" variant="ghost" onClick={() => replaceAll(templateFor(kind, model))}>
          {t("skins.editor.template")}
        </Button>
        <Button size="sm" variant="ghost" onClick={() => replaceAll(emptyFor(kind))}>
          {t("skins.editor.blank")}
        </Button>
        <div className="ml-auto flex min-w-[220px] flex-1 items-center gap-2 sm:max-w-sm">
          <TextInput
            value={name}
            maxLength={48}
            placeholder={t("skins.editor.namePlaceholder")}
            aria-label={t("skins.editor.namePlaceholder")}
            onChange={(e) => setName(e.target.value)}
          />
          <Button
            variant="primary"
            className="shrink-0"
            disabled={saving || !ready}
            onClick={() => void save()}
          >
            <Save size={14} />
            {saved ? t("skins.editor.saved") : t("skins.editor.save")}
          </Button>
        </div>
      </div>
    </div>
  );
}

function Swatch({
  color,
  active,
  onPick,
}: {
  color: string;
  active: boolean;
  onPick: (c: string) => void;
}) {
  return (
    <button
      type="button"
      aria-label={color}
      title={color}
      onClick={() => onPick(color)}
      className={`h-5 w-5 rounded-sm border ${active ? "border-accent ring-1 ring-accent" : "border-line"}`}
      style={{ background: color }}
    />
  );
}

/** Draws the zoomed texture with a transparency checkerboard, guides and grid. */
function render(
  canvas: HTMLCanvasElement,
  p: Pixels,
  boxes: Box[],
  o: {
    grid: boolean;
    guides: boolean;
    hover: { x: number; y: number } | null;
    showOverlay: boolean;
  },
) {
  const W = p.width * CELL;
  const H = p.height * CELL;
  if (canvas.width !== W) canvas.width = W;
  if (canvas.height !== H) canvas.height = H;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.imageSmoothingEnabled = false;
  // Checkerboard; unused areas darker.
  for (let y = 0; y < p.height; y++) {
    for (let x = 0; x < p.width; x++) {
      const used = locate(boxes, x, y) !== null;
      const light = (x + y) % 2 === 0;
      ctx.fillStyle = used ? (light ? "#3a3d45" : "#30333a") : light ? "#1b1c20" : "#17181b";
      ctx.fillRect(x * CELL, y * CELL, CELL, CELL);
    }
  }
  let src = p;
  if (!o.showOverlay) {
    src = clone(p);
    clearLayer(src, boxes, "overlay");
  }
  ctx.drawImage(toCanvas(src), 0, 0, W, H);

  if (o.grid) {
    ctx.strokeStyle = "rgba(255,255,255,0.07)";
    ctx.lineWidth = 1;
    ctx.beginPath();
    for (let x = 0; x <= p.width; x++) {
      ctx.moveTo(x * CELL + 0.5, 0);
      ctx.lineTo(x * CELL + 0.5, H);
    }
    for (let y = 0; y <= p.height; y++) {
      ctx.moveTo(0, y * CELL + 0.5);
      ctx.lineTo(W, y * CELL + 0.5);
    }
    ctx.stroke();
  }
  if (o.guides) {
    ctx.lineWidth = 2;
    for (const b of boxes) {
      ctx.strokeStyle = PART_COLORS[b.part] ?? "#ffffff";
      ctx.setLineDash(b.layer === "overlay" ? [6, 4] : []);
      for (const f of faces(b))
        ctx.strokeRect(f.x * CELL + 1, f.y * CELL + 1, f.w * CELL - 2, f.h * CELL - 2);
    }
    ctx.setLineDash([]);
  }
  if (o.hover) {
    ctx.strokeStyle = "#ffffff";
    ctx.lineWidth = 2;
    ctx.strokeRect(o.hover.x * CELL + 1, o.hover.y * CELL + 1, CELL - 2, CELL - 2);
  }
}
