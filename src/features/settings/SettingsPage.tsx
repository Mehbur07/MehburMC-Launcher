import { Check, LogOut, RefreshCw, ShieldCheck } from "lucide-react";
import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { Badge, Button, ConfirmDialog, TextInput, Toggle } from "../../components/ui";
import type { Accent } from "../../lib/ipc/bindings/Accent";
import type { Language } from "../../lib/ipc/bindings/Language";
import type { LaunchBehavior } from "../../lib/ipc/bindings/LaunchBehavior";
import { useApp } from "../../stores/app";
import { useAuth } from "../../stores/auth";
import { useUpdate } from "../../stores/update";
import { DataFolderSection } from "./DataFolderSection";

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
  const [cfKey, setCfKey] = useState(settings?.curseforgeApiKey ?? "");
  const upd = useUpdate();

  if (!settings || !boot) return null;

  const saveCfKey = () => {
    const v = cfKey.trim();
    if (v !== (settings.curseforgeApiKey ?? "")) {
      void update({ curseforgeApiKey: v || undefined });
    }
  };

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-6 pb-8">
      <h1 className="font-display text-3xl font-bold tracking-wide">{t("settings.title")}</h1>

      <AccountSection />

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

      <Section title={t("settings.game")}>
        <Row label={t("settings.launchBehavior")} hint={t("settings.launchBehaviorHint")}>
          <select
            value={settings.launchBehavior}
            onChange={(e) => void update({ launchBehavior: e.target.value as LaunchBehavior })}
            className="rounded-md border border-line bg-surface-2 px-3 py-2 text-sm focus:border-accent"
          >
            {(["minimize", "close", "keepOpen"] as const).map((b) => (
              <option key={b} value={b}>
                {t(`settings.launch.${b}`)}
              </option>
            ))}
          </select>
        </Row>
        <Row
          label={t("settings.defaultMemory")}
          hint={t("settings.defaultMemoryHint", {
            gb: (settings.defaultMemoryMb / 1024).toFixed(1),
          })}
        >
          <input
            type="range"
            min={1024}
            max={16384}
            step={512}
            value={Math.min(settings.defaultMemoryMb, 16384)}
            onChange={(e) => void update({ defaultMemoryMb: Number(e.target.value) })}
            aria-label={t("settings.defaultMemory")}
            className="w-48 accent-[var(--mc-accent)]"
          />
        </Row>
        <Row label={t("settings.concurrency")} hint={t("settings.concurrencyHint")}>
          <input
            type="number"
            min={1}
            max={32}
            value={settings.downloadConcurrency}
            onChange={(e) => {
              const v = Math.round(Number(e.target.value));
              if (v >= 1 && v <= 32) void update({ downloadConcurrency: v });
            }}
            aria-label={t("settings.concurrency")}
            className="w-20 rounded-md border border-line bg-surface-2 px-3 py-2 text-sm focus:border-accent"
          />
        </Row>
      </Section>

      <Section title={t("settings.dataFolder")}>
        <DataFolderSection />
      </Section>

      <Section title={t("settings.advanced")}>
        <Row label={t("settings.debugLogging")} hint={t("settings.debugLoggingHint")}>
          <Toggle
            checked={settings.debugLogging}
            onChange={(v) => void update({ debugLogging: v })}
            label={t("settings.debugLogging")}
          />
        </Row>
        <div className="flex flex-col gap-2 py-3">
          <div>
            <div className="text-sm font-medium">{t("settings.curseforgeKey")}</div>
            <div className="mt-0.5 text-xs text-muted">{t("settings.curseforgeKeyHint")}</div>
          </div>
          <TextInput
            type="password"
            value={cfKey}
            onChange={(e) => setCfKey(e.target.value)}
            onBlur={saveCfKey}
            autoComplete="off"
            spellCheck={false}
            aria-label={t("settings.curseforgeKey")}
            className="font-mono"
          />
        </div>
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
        <Row label={t("update.auto")} hint={t("update.autoHint")}>
          <Toggle
            checked={settings.checkUpdates}
            onChange={(v) => void update({ checkUpdates: v })}
            label={t("update.auto")}
          />
        </Row>
        <div className="flex flex-wrap items-center gap-3 py-3 text-sm">
          <Button size="sm" disabled={upd.checking} onClick={() => void upd.check()}>
            <RefreshCw size={13} className={upd.checking ? "animate-spin" : ""} />
            {t("update.checkNow")}
          </Button>
          {upd.info && (
            <span className={upd.info.status === "available" ? "text-accent" : "text-muted"}>
              {t(`update.status.${upd.info.status}`, { version: upd.info.version ?? "" })}
            </span>
          )}
          {upd.info?.status === "available" && (
            <Button
              size="sm"
              variant="primary"
              disabled={upd.installing !== null}
              onClick={() => void upd.install()}
            >
              {t("update.install")}
            </Button>
          )}
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

/** The MehburMC account the launcher is signed in with (K73). */
function AccountSection() {
  const { t } = useTranslation();
  const auth = useAuth((s) => s.status);
  const signOut = useAuth((s) => s.signOut);
  const [confirm, setConfirm] = useState(false);
  if (!auth?.signedIn) return null;
  return (
    <Section title={t("settings.account.title")}>
      <Row label={auth.email ?? ""} hint={t("settings.account.hint")}>
        <div className="flex items-center gap-2">
          {auth.rank > 0 && <Badge tone="accent">{t(`admin.rank.${auth.rank}`)}</Badge>}
          <Button variant="ghost" onClick={() => setConfirm(true)}>
            <LogOut size={14} />
            {t("auth.signOut")}
          </Button>
        </div>
      </Row>
      <ConfirmDialog
        open={confirm}
        title={t("settings.account.signOutTitle")}
        message={t("settings.account.signOutMessage")}
        confirmLabel={t("auth.signOut")}
        onClose={() => setConfirm(false)}
        onConfirm={() => void signOut()}
      />
    </Section>
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
