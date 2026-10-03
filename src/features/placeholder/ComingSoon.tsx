import { Hourglass } from "lucide-react";
import { useTranslation } from "react-i18next";

import type { View } from "../../stores/app";

/** Phase in which each not-yet-built screen lands (ARCHITECTURE.md §12). */
const PHASE: Partial<Record<View, number>> = {
  instances: 3,
  downloads: 3,
  console: 3,
  accounts: 5,
  browse: 6,
  skins: 7,
};

export function ComingSoon({ view }: { view: View }) {
  const { t } = useTranslation();
  return (
    <div className="flex h-full flex-col">
      <h1 className="font-display text-3xl font-bold tracking-wide">{t(`nav.${view}`)}</h1>
      <div className="mt-6 flex flex-1 flex-col items-center justify-center rounded-lg border border-dashed border-line bg-surface-1/60 text-center backdrop-blur">
        <Hourglass className="text-accent neon-drop" size={36} />
        <p className="mt-4 font-display text-xl font-semibold">{t("comingSoon.title")}</p>
        <p className="mt-1 text-sm text-muted">
          {t("comingSoon.body", { phase: PHASE[view] ?? "?" })}
        </p>
      </div>
    </div>
  );
}
