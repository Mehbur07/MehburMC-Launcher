import { Check, FolderInput, FolderOpen, ShieldCheck } from "lucide-react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { Toggle } from "../../components/ui";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { Accent } from "../../lib/ipc/bindings/Accent";
import type { Language } from "../../lib/ipc/bindings/Language";
import { useApp } from "../../stores/app";

/** Swatch colors mirror tokens.css presets. */
const ACCENTS: { id: Accent; color: string }[] = [
  { id: "cyan", color: "#00f0ff" },
  { id: "magenta", color: "#ff3df0" },
  { id: "green", color: "#39ff88" },
  { id: "purple", color: "#b77cff" },
];

export function SettingsPage() {
  const { t } = useTranslation();
  const settings = useApp((s) => s.settings);
  const boot = useApp((s) => s.boot);
  const update = useApp((s) => s.updateSettings);

  if (!settings || !boot) return null;

  const openDataDir = async () => {
    try {
      await ipc.openDataDir();
    } catch (e) {
      useApp.setState({ notice: toErrorPayload(e) });
    }
  };

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-6 pb-8">
      <h1 className="font-display text-3xl font-bold tracking-wide">{t("settings.title")}</h1>

      <Section title={t("settings.appearance")}>
        <Row label={t("settings.language")}>
          <select
            value={settings.language}
            onChange={(e) => void update({ language: e.target.value as Language })}
            className="rounded-md border border-line bg-surface-2 px-3 py-2 text-sm focus:border-accent"
          >
            <option value="system">{t("settings.languageSystem")}</option>
            <option value="tr">Türkçe</option>
            <option value="en">English</option>
          </select>
        </Row>

        <Row label={t("settings.accent")}>
          <div className="flex gap-2" role="radiogroup" aria-label={t("settings.accent")}>
            {ACCENTS.map(({ id, color }) => {
              const active = settings.accent === id;
              return (
                <button
                  key={id}
                  type="button"
                  role="radio"
                  aria-checked={active}
                  title={t(`settings.accents.${id}`)}
                  aria-label={t(`settings.accents.${id}`)}
                  onClick={() => void update({ accent: id })}
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
          </div>
        </Row>

        <Row label={t("settings.backgroundEffects")} hint={t("settings.backgroundEffectsHint")}>
          <Toggle
            checked={settings.backgroundEffects}
            onChange={(v) => void update({ backgroundEffects: v })}
            label={t("settings.backgroundEffects")}
          />
        </Row>
      </Section>

      <Section title={t("settings.dataFolder")}>
        <div className="flex flex-col gap-3 py-3">
          <div className="flex items-center gap-2 text-xs">
            <span className="rounded-full border border-accent/40 bg-accent/10 px-2 py-0.5 text-accent">
              {t(`settings.dataMode.${boot.paths?.mode ?? "standard"}`)}
            </span>
            {boot.paths?.redirected && (
              <span className="rounded-full border border-warn/40 bg-warn/10 px-2 py-0.5 text-warn">
                {t("settings.redirected")}
              </span>
            )}
          </div>
          <code
            data-selectable
            className="rounded-md bg-bg/70 px-3 py-2 text-xs break-all text-muted"
          >
            {boot.paths?.content}
          </code>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => void openDataDir()}
              className="inline-flex items-center gap-2 rounded-md border border-accent/50 px-3 py-2 text-sm text-accent transition-colors hover:bg-accent/10"
            >
              <FolderOpen size={16} />
              {t("settings.openDataFolder")}
            </button>
            <button
              type="button"
              disabled
              title={t("settings.moveDataFolderSoon")}
              className="inline-flex cursor-not-allowed items-center gap-2 rounded-md border border-line px-3 py-2 text-sm text-muted opacity-60"
            >
              <FolderInput size={16} />
              {t("settings.moveDataFolder")}
            </button>
          </div>
        </div>
      </Section>

      <Section title={t("settings.advanced")}>
        <Row label={t("settings.debugLogging")} hint={t("settings.debugLoggingHint")}>
          <Toggle
            checked={settings.debugLogging}
            onChange={(v) => void update({ debugLogging: v })}
            label={t("settings.debugLogging")}
          />
        </Row>
      </Section>

      <Section title={t("settings.about")}>
        <div className="flex flex-col gap-2 py-3 text-sm">
          <span>
            {boot.appName} · {t("settings.version", { version: boot.version })}
          </span>
          <span className="inline-flex items-center gap-2 text-muted">
            <ShieldCheck size={16} className="text-success" />
            {t("settings.telemetry")}
          </span>
        </div>
      </Section>
    </div>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="rounded-lg border border-line bg-surface-1/80 px-5 py-3 backdrop-blur">
      <h2 className="border-b border-line pb-2 font-display text-sm font-semibold tracking-[0.15em] text-accent uppercase">
        {title}
      </h2>
      <div className="divide-y divide-line">{children}</div>
    </section>
  );
}

function Row({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-6 py-3">
      <div>
        <div className="text-sm font-medium">{label}</div>
        {hint && <div className="mt-0.5 text-xs text-muted">{hint}</div>}
      </div>
      {children}
    </div>
  );
}
