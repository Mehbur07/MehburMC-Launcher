import { Boxes, MemoryStick, Play } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useApp } from "../../stores/app";

export function HomePage() {
  const { t } = useTranslation();
  const memoryMb = useApp((s) => s.settings?.defaultMemoryMb ?? 0);

  return (
    <div className="flex h-full flex-col gap-6">
      <section className="relative overflow-hidden rounded-lg border border-line bg-surface-1/80 p-8 backdrop-blur">
        <div className="pointer-events-none absolute -top-24 -right-24 h-72 w-72 rounded-full bg-accent/10 blur-3xl" />
        <div className="relative flex items-end justify-between gap-8">
          <div>
            <div className="mb-3 inline-flex items-center gap-2 rounded-full border border-line bg-surface-2 px-3 py-1 text-xs text-muted">
              <Boxes size={14} />
              {t("home.noInstance")}
            </div>
            <h1 className="font-display text-4xl font-bold tracking-wide">
              MehburMC <span className="text-accent neon-text">Launcher</span>
            </h1>
            <p className="mt-2 max-w-md text-sm text-muted">{t("home.noInstanceHint")}</p>
          </div>

          <div className="flex flex-col items-end gap-3">
            <div className="flex items-center gap-2 text-xs text-muted">
              <MemoryStick size={14} />
              {t("home.memory")}:{" "}
              <span className="font-semibold text-fg">{(memoryMb / 1024).toFixed(1)} GB</span>
            </div>
            <button
              type="button"
              disabled
              title={t("home.playDisabled")}
              className="play-pulse flex items-center gap-3 rounded-lg bg-accent px-10 py-4 font-brand text-xl font-bold tracking-[0.2em] text-on-accent transition-transform enabled:hover:scale-[1.03] disabled:cursor-not-allowed disabled:opacity-60"
            >
              <Play size={22} fill="currentColor" />
              {t("home.play")}
            </button>
            <span className="text-xs text-muted">{t("home.playDisabled")}</span>
          </div>
        </div>
      </section>
    </div>
  );
}
