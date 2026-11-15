import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import type { PhoneRenderer } from "../../lib/ipc/bindings/PhoneRenderer";
import type { PhoneSettings } from "../../lib/ipc/bindings/PhoneSettings";
import { useApp } from "../../stores/app";

type PercentKey = Exclude<keyof PhoneSettings, "renderer">;

/** Sınırlar launcher-core'daki `PHONE_*_RANGE` sabitleriyle aynı (A3, K105). */
const SLIDERS: { key: PercentKey; min: number; max: number; step: number }[] = [
  { key: "resolutionPercent", min: 25, max: 100, step: 5 },
  { key: "sensitivityPercent", min: 25, max: 300, step: 5 },
  { key: "controlsSizePercent", min: 50, max: 200, step: 10 },
  { key: "controlsOpacityPercent", min: 10, max: 100, step: 5 },
];

/** Telefonda oyun ayarları: yalnız Android'de gösterilir. */
export function PhoneSection() {
  const { t } = useTranslation();
  const settings = useApp((s) => s.settings);
  const update = useApp((s) => s.updateSettings);
  if (!settings) return null;
  const phone = settings.phone;
  const set = (patch: Partial<PhoneSettings>) => void update({ phone: { ...phone, ...patch } });

  return (
    <div className="flex flex-col gap-4">
      <Field label={t("settings.phone.renderer")} hint={t("settings.phone.rendererHint")}>
        <select
          value={phone.renderer}
          onChange={(e) => set({ renderer: e.target.value as PhoneRenderer })}
          aria-label={t("settings.phone.renderer")}
          className="rounded-md border border-line bg-surface-2 px-3 py-2 text-sm focus:border-accent"
        >
          {(["auto", "mobileGlues", "gl4es"] as const).map((r) => (
            <option key={r} value={r}>
              {t(`settings.phone.renderers.${r}`)}
            </option>
          ))}
        </select>
      </Field>
      {SLIDERS.map(({ key, min, max, step }) => (
        <Field
          key={key}
          label={`${t(`settings.phone.${key}`)}: ${t("settings.phone.percent", { value: phone[key] })}`}
          hint={t(`settings.phone.${key}Hint`)}
        >
          <input
            type="range"
            min={min}
            max={max}
            step={step}
            value={phone[key]}
            onChange={(e) => set({ [key]: Number(e.target.value) })}
            aria-label={t(`settings.phone.${key}`)}
            className="w-full accent-[var(--mc-accent)]"
          />
        </Field>
      ))}
      <p className="text-xs text-muted">{t("settings.phone.nextGame")}</p>
    </div>
  );
}

function Field({ label, hint, children }: { label: string; hint: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <span className="text-sm font-medium">{label}</span>
      {children}
      <span className="text-xs text-muted">{hint}</span>
    </div>
  );
}
