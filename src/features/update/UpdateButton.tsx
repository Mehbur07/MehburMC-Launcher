import { Download, LoaderCircle } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useTranslation } from "react-i18next";

import { useUpdate } from "../../stores/update";

/**
 * Title-bar button shown while a launcher update is available. It downloads
 * the signed installer, runs it and restarts into the new version.
 */
export function UpdateButton() {
  const { t } = useTranslation();
  const info = useUpdate((s) => s.info);
  const installing = useUpdate((s) => s.installing);
  const install = useUpdate((s) => s.install);
  const show = info?.status === "available";
  const busy = installing !== null;

  return (
    <AnimatePresence>
      {show && (
        <motion.button
          type="button"
          initial={{ opacity: 0, x: -6 }}
          animate={{ opacity: 1, x: 0 }}
          exit={{ opacity: 0, x: -6 }}
          disabled={busy}
          onClick={() => void install()}
          title={t("update.available", { version: info.version, current: info.current })}
          className="relative ml-2 flex h-6 items-center gap-1.5 overflow-hidden rounded-full border border-accent/60 bg-accent/15 px-2.5 text-[11px] font-bold tracking-wide text-accent shadow-[0_0_12px_rgb(var(--mc-accent-rgb)/0.35)] transition-colors not-disabled:hover:bg-accent not-disabled:hover:text-on-accent disabled:cursor-progress"
        >
          {busy && (
            // Download progress fills the pill from the left.
            <span
              aria-hidden
              className="absolute inset-y-0 left-0 bg-accent/30 transition-[width]"
              style={{ width: `${Math.round(installing * 100)}%` }}
            />
          )}
          {busy ? (
            <LoaderCircle size={12} className="relative animate-spin" />
          ) : (
            <Download size={12} className="relative" />
          )}
          <span className="relative">
            {busy
              ? t("update.installing", { pct: Math.round(installing * 100) })
              : t("update.button", { version: info.version })}
          </span>
        </motion.button>
      )}
    </AnimatePresence>
  );
}
