import { Check, Pipette } from "lucide-react";
import { useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { useTranslation } from "react-i18next";

import {
  accentTokens,
  applyAccent,
  hexToHsv,
  hsvToHex,
  parseHex,
  readableAccent,
  type Hsv,
} from "../../lib/accentColor";
import type { Accent } from "../../lib/ipc/bindings/Accent";
import { useApp } from "../../stores/app";

/** Swatch colors mirror tokens.css presets. */
const ACCENTS: { id: Accent; color: string }[] = [
  { id: "cyan", color: "#00f0ff" },
  { id: "magenta", color: "#ff3df0" },
  { id: "green", color: "#39ff88" },
  { id: "purple", color: "#b77cff" },
];

const HUES = "#f00, #ff0, #0f0, #0ff, #00f, #f0f, #f00";

const clamp01 = (x: number) => Math.min(1, Math.max(0, x));

export function AccentSetting() {
  const { t } = useTranslation();
  const accent = useApp((s) => s.settings?.accent);
  const custom = useApp((s) => s.settings?.accentColor);
  const update = useApp((s) => s.updateSettings);
  const [open, setOpen] = useState(false);

  const preset = ACCENTS.find((a) => a.id === accent)?.color ?? "#00f0ff";
  const shown = custom ? accentTokens(custom) : null;

  return (
    <div className="py-3">
      <div className="flex items-center justify-between gap-6">
        <div className="text-sm font-medium">{t("settings.accent")}</div>
        <div className="flex gap-2" role="radiogroup" aria-label={t("settings.accent")}>
          {ACCENTS.map(({ id, color }) => {
            const active = !custom && accent === id;
            return (
              <button
                key={id}
                type="button"
                role="radio"
                aria-checked={active}
                title={t(`settings.accents.${id}`)}
                aria-label={t(`settings.accents.${id}`)}
                onClick={() => {
                  setOpen(false);
                  void update({ accent: id, accentColor: undefined });
                }}
                className="grid h-8 w-8 place-items-center rounded-full transition-transform hover:scale-110"
                style={{
                  background: color,
                  boxShadow: active
                    ? `0 0 0 2px var(--mc-bg), 0 0 0 4px ${color}, 0 0 16px ${color}`
                    : undefined,
                }}
              >
                {active && <Check size={16} className="text-black" />}
              </button>
            );
          })}
          <button
            type="button"
            role="radio"
            aria-checked={!!custom}
            aria-expanded={open}
            title={t("settings.accents.custom")}
            aria-label={t("settings.accents.custom")}
            onClick={() => setOpen((o) => !o)}
            className="grid h-8 w-8 place-items-center rounded-full transition-transform hover:scale-110"
            style={{
              background: shown?.["--mc-accent"] ?? `conic-gradient(${HUES})`,
              boxShadow: shown
                ? `0 0 0 2px var(--mc-bg), 0 0 0 4px ${shown["--mc-accent"]}, 0 0 16px ${shown["--mc-accent"]}`
                : undefined,
            }}
          >
            {shown ? (
              <Check size={16} style={{ color: shown["--mc-on-accent"] }} />
            ) : (
              <Pipette size={14} className="text-black" />
            )}
          </button>
        </div>
      </div>
      {open && (
        <ColorPanel
          initial={custom ?? preset}
          onCommit={(hex) => {
            if (hex !== custom) void update({ accentColor: hex });
          }}
        />
      )}
    </div>
  );
}

/** Shows every change at once; saves when a drag or a key press ends. */
function ColorPanel({ initial, onCommit }: { initial: string; onCommit: (hex: string) => void }) {
  const { t } = useTranslation();
  const [hsv, setHsv] = useState<Hsv>(() => hexToHsv(initial));
  const [text, setText] = useState(initial);
  const pad = useRef<HTMLDivElement>(null);
  const hex = parseHex(text) ?? hsvToHex(hsv);
  const lightened = readableAccent(hex) !== hex;

  const change = (next: Hsv) => {
    const value = hsvToHex(next);
    setHsv(next);
    setText(value);
    applyAccent(value);
  };

  const fromPointer = (e: PointerEvent<HTMLDivElement>) => {
    const r = pad.current?.getBoundingClientRect();
    if (!r || r.width === 0 || r.height === 0) return;
    change({
      h: hsv.h,
      s: clamp01((e.clientX - r.left) / r.width),
      v: 1 - clamp01((e.clientY - r.top) / r.height),
    });
  };

  const padKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = e.shiftKey ? 0.1 : 0.02;
    const move: Record<string, Partial<Hsv>> = {
      ArrowLeft: { s: clamp01(hsv.s - step) },
      ArrowRight: { s: clamp01(hsv.s + step) },
      ArrowDown: { v: clamp01(hsv.v - step) },
      ArrowUp: { v: clamp01(hsv.v + step) },
    };
    if (!move[e.key]) return;
    e.preventDefault();
    change({ ...hsv, ...move[e.key] });
  };

  return (
    <div className="pop-in mt-3 flex flex-col gap-3 rounded-md border border-line bg-surface-2 p-3">
      <div
        ref={pad}
        role="slider"
        tabIndex={0}
        aria-label={t("settings.accentPicker.pad")}
        aria-valuetext={hex}
        aria-valuenow={Math.round(hsv.s * 100)}
        onPointerDown={(e) => {
          e.currentTarget.setPointerCapture(e.pointerId);
          fromPointer(e);
        }}
        onPointerMove={(e) => {
          if (e.currentTarget.hasPointerCapture(e.pointerId)) fromPointer(e);
        }}
        onPointerUp={() => onCommit(hex)}
        onKeyDown={padKey}
        onKeyUp={(e) => {
          if (e.key.startsWith("Arrow")) onCommit(hex);
        }}
        className="relative h-36 cursor-crosshair touch-none rounded-md"
        style={{
          background: `linear-gradient(to top, #000, transparent), linear-gradient(to right, #fff, hsl(${hsv.h} 100% 50%))`,
        }}
      >
        <span
          className="pointer-events-none absolute h-3.5 w-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-[0_0_0_1px_rgb(0_0_0/0.6)]"
          style={{ left: `${hsv.s * 100}%`, top: `${(1 - hsv.v) * 100}%`, background: hex }}
        />
      </div>
      <input
        type="range"
        min={0}
        max={360}
        value={hsv.h}
        aria-label={t("settings.accentPicker.hue")}
        onChange={(e) => change({ ...hsv, h: Number(e.target.value) })}
        onPointerUp={() => onCommit(hex)}
        onKeyUp={(e) => {
          if (e.key.startsWith("Arrow")) onCommit(hex);
        }}
        className="h-2.5 w-full cursor-pointer appearance-none rounded-full [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:border-2 [&::-webkit-slider-thumb]:border-white [&::-webkit-slider-thumb]:bg-transparent [&::-webkit-slider-thumb]:shadow-[0_0_0_1px_rgb(0_0_0/0.6)]"
        style={{ background: `linear-gradient(to right, ${HUES})` }}
      />
      <label className="flex items-center gap-3 text-xs text-muted">
        {t("settings.accentPicker.hex")}
        <input
          value={text}
          maxLength={7}
          spellCheck={false}
          onChange={(e) => {
            setText(e.target.value);
            const value = parseHex(e.target.value);
            if (!value) return;
            setHsv(hexToHsv(value));
            applyAccent(value);
            onCommit(value);
          }}
          aria-invalid={!parseHex(text)}
          className="w-24 rounded-md border border-line bg-surface-1 px-2 py-1 font-mono text-sm text-text uppercase focus:border-accent aria-invalid:border-danger"
        />
      </label>
      {lightened && <p className="text-xs text-muted">{t("settings.accentPicker.lightened")}</p>}
    </div>
  );
}
