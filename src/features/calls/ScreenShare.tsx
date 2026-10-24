import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Maximize2,
  Minimize2,
  MonitorOff,
  MonitorUp,
  ScreenShare as ScreenIcon,
  Volume2,
  X,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { IconButton } from "../../components/ui";
import type { CallMember } from "../../lib/ipc/bindings/CallMember";
import { type ScreenQuality } from "../../lib/voice/mesh";
import { localScreenStream, screenStream, useCalls } from "../../stores/calls";

const QUALITIES: ScreenQuality[] = ["720p30", "1080p30", "1080p60"];

/** Plays a stream in a muted `<video>` (screen audio plays through the store). */
function StreamVideo({
  stream,
  version,
  className,
  label,
}: {
  stream: MediaStream | null;
  version: number;
  className: string;
  label: string;
}) {
  const ref = useRef<HTMLVideoElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.srcObject = stream;
    void el.play?.()?.catch(() => undefined);
    return () => {
      el.srcObject = null;
    };
  }, [stream, version]);
  return <video ref={ref} autoPlay playsInline muted aria-label={label} className={className} />;
}

/** Call bar button: quality menu → screen picker; while sharing, stop. */
export function ShareButton() {
  const { t } = useTranslation();
  const sharing = useCalls((s) => s.sharing);
  const saved = useCalls((s) => s.shareQuality);
  const { startShare, stopShare } = useCalls.getState();
  const [menu, setMenu] = useState(false);
  const [quality, setQuality] = useState<ScreenQuality>(saved);

  if (sharing) {
    return (
      <IconButton
        label={t("calls.screen.stop")}
        onClick={stopShare}
        className="h-10 w-10 bg-accent/20 text-accent hover:text-accent"
      >
        <MonitorOff size={17} />
      </IconButton>
    );
  }
  return (
    <div className="relative">
      <IconButton
        label={t("calls.screen.share")}
        aria-expanded={menu}
        onClick={() => {
          setQuality(saved);
          setMenu(!menu);
        }}
        className="h-10 w-10 bg-surface-2"
      >
        <MonitorUp size={17} />
      </IconButton>
      {menu && (
        <div
          role="dialog"
          aria-label={t("calls.screen.quality")}
          className="pop-in absolute bottom-12 left-1/2 z-10 w-56 -translate-x-1/2 rounded-lg border border-line bg-surface-1 p-3 shadow-lg"
        >
          <p className="mb-2 text-xs font-semibold">{t("calls.screen.quality")}</p>
          <fieldset className="flex flex-col gap-1.5">
            {QUALITIES.map((q) => (
              <label key={q} className="flex items-center gap-2 text-xs">
                <input
                  type="radio"
                  name="share-quality"
                  checked={quality === q}
                  onChange={() => setQuality(q)}
                  className="accent-[rgb(var(--mc-accent-rgb))]"
                />
                {t(`calls.screen.qualities.${q}`)}
              </label>
            ))}
          </fieldset>
          <p className="mt-2 text-[11px] text-muted">{t("calls.screen.audioHint")}</p>
          <button
            type="button"
            onClick={() => {
              setMenu(false);
              void startShare(quality);
            }}
            className="mt-3 w-full rounded-md bg-accent px-3 py-1.5 text-xs font-semibold text-black hover:bg-accent/85"
          >
            {t("calls.screen.start")}
          </button>
        </div>
      )}
    </div>
  );
}

/** Small preview of our own share inside the call bar. */
export function LocalSharePreview() {
  const { t } = useTranslation();
  const sharing = useCalls((s) => s.sharing);
  const withAudio = useCalls((s) => s.sharingAudio);
  if (!sharing) return null;
  return (
    <div className="mt-3 flex items-center gap-2 rounded-md border border-accent/40 bg-accent/10 p-1.5">
      <StreamVideo
        stream={localScreenStream()}
        version={0}
        label={t("calls.screen.preview")}
        className="h-10 w-16 shrink-0 rounded bg-black object-contain"
      />
      <div className="min-w-0 flex-1 text-[11px]">
        <p className="font-semibold text-accent">{t("calls.screen.sharing")}</p>
        <p className="flex items-center gap-1 text-muted">
          {t(`calls.screen.qualities.${sharing}`)}
          {withAudio && <Volume2 size={11} aria-label={t("calls.screen.withAudio")} />}
        </p>
      </div>
    </div>
  );
}

async function setWindowFullscreen(on: boolean) {
  try {
    await getCurrentWindow().setFullscreen(on);
  } catch {
    // Outside Tauri (tests) or not allowed: the overlay still fills the window.
  }
}

/** Others' shared screens: a panel left of the call bar, or full screen. */
export function ScreenPanel({ members }: { members: CallMember[] }) {
  const { t } = useTranslation();
  const screens = useCalls((s) => s.screens);
  const sharers = useMemo(
    () => members.filter((m) => screens[m.id] !== undefined),
    [members, screens],
  );
  const [picked, setPicked] = useState<string | null>(null);
  const [mode, setMode] = useState<"panel" | "small" | "full" | "hidden">("panel");
  const shown = sharers.find((m) => m.id === picked) ?? sharers[0];
  const count = sharers.length;

  // A new share opens the panel again.
  const prev = useRef(0);
  useEffect(() => {
    if (count > prev.current) setMode((m) => (m === "hidden" ? "panel" : m));
    prev.current = count;
  }, [count]);

  // Full screen ends with Esc or when the share ends.
  const full = mode === "full" && shown !== undefined;
  useEffect(() => {
    if (!full) return;
    void setWindowFullscreen(true);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setMode("panel");
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      void setWindowFullscreen(false);
    };
  }, [full]);

  if (!shown) return null;
  const name = shown.nickname ?? shown.displayName;
  const title = t("calls.screen.watching", { name });

  if (mode === "hidden" || mode === "small") {
    return (
      <button
        type="button"
        onClick={() => setMode("panel")}
        className="pointer-events-auto fixed right-80 bottom-4 z-50 flex items-center gap-2 rounded-lg border border-accent/50 bg-surface-1/95 px-3 py-2 text-xs font-semibold text-accent neon-ring backdrop-blur"
      >
        <ScreenIcon size={14} />
        {count > 1 ? t("calls.screen.manySharing", { count }) : title}
      </button>
    );
  }

  const tabs = count > 1 && (
    <div className="flex flex-wrap gap-1" role="group" aria-label={t("calls.screen.pick")}>
      {sharers.map((m) => (
        <button
          key={m.id}
          type="button"
          aria-pressed={m.id === shown.id}
          onClick={() => setPicked(m.id)}
          className={`rounded px-2 py-0.5 text-[11px] ${m.id === shown.id ? "bg-accent/20 text-accent" : "text-muted hover:text-fg"}`}
        >
          {m.nickname ?? m.displayName}
        </button>
      ))}
    </div>
  );

  const video = (
    <StreamVideo
      stream={screenStream(shown.id)}
      version={screens[shown.id] ?? 0}
      label={title}
      className="h-full w-full bg-black object-contain"
    />
  );

  if (full) {
    return (
      <section
        aria-label={title}
        className="pointer-events-auto fixed inset-0 z-[60] flex flex-col bg-black"
      >
        <div className="absolute top-3 right-3 z-10 flex items-center gap-2 rounded-lg bg-black/60 px-2 py-1 text-xs text-white/90">
          <span className="font-semibold">{title}</span>
          {tabs}
          <IconButton
            label={t("calls.screen.exitFull")}
            onClick={() => setMode("panel")}
            className="h-7 w-7 text-white/90"
          >
            <Minimize2 size={14} />
          </IconButton>
        </div>
        {video}
      </section>
    );
  }

  return (
    <section
      aria-label={title}
      className="pointer-events-auto fixed right-80 bottom-4 z-50 flex w-[min(640px,calc(100vw-22rem))] flex-col overflow-hidden rounded-lg border border-line bg-surface-1/95 neon-ring backdrop-blur"
    >
      <div className="flex items-center gap-2 px-3 py-2">
        <ScreenIcon size={14} className="shrink-0 text-accent" />
        <p className="min-w-0 flex-1 truncate text-xs font-semibold">{title}</p>
        {tabs}
        <IconButton
          label={t("calls.screen.full")}
          onClick={() => setMode("full")}
          className="h-7 w-7"
        >
          <Maximize2 size={14} />
        </IconButton>
        <IconButton
          label={t("calls.screen.minimize")}
          onClick={() => setMode("small")}
          className="h-7 w-7"
        >
          <Minimize2 size={14} />
        </IconButton>
        <IconButton
          label={t("calls.screen.close")}
          onClick={() => setMode("hidden")}
          className="h-7 w-7"
        >
          <X size={14} />
        </IconButton>
      </div>
      <div className="aspect-video w-full bg-black">{video}</div>
    </section>
  );
}
