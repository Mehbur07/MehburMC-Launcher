import { Download, Sparkles, X } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useTranslation } from "react-i18next";

import { Button, IconButton, ProgressBar } from "../../components/ui";
import { useUpdate } from "../../stores/update";

/** Slim banner above the page content when a launcher update is available. */
export function UpdateBanner() {
  const { t } = useTranslation();
  const { info, installing, dismissed, install, dismiss } = useUpdate();
  const show = info?.status === "available" && !dismissed;

  return (
    <AnimatePresence>
      {show && (
        <motion.div
          initial={{ opacity: 0, y: -8 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -8 }}
          className="mb-4 flex items-center gap-3 rounded-lg border border-accent/40 bg-accent/10 px-4 py-2.5 text-sm backdrop-blur"
        >
          <Sparkles size={18} className="shrink-0 text-accent neon-drop" />
          <div className="min-w-0 flex-1">
            <div className="font-semibold">
              {t("update.available", { version: info.version, current: info.current })}
            </div>
            {installing !== null && (
              <div className="mt-1.5">
                <ProgressBar value={installing} />
              </div>
            )}
          </div>
          <Button
            size="sm"
            variant="primary"
            disabled={installing !== null}
            onClick={() => void install()}
          >
            <Download size={13} />
            {t("update.install")}
          </Button>
          <IconButton label={t("common.dismiss")} onClick={dismiss}>
            <X size={15} />
          </IconButton>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
