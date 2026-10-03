import { AnimatePresence, MotionConfig, motion } from "motion/react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import { ErrorNotice } from "../components/ErrorNotice";
import { Logo } from "../components/Logo";
import { NeonBackground } from "../components/NeonBackground";
import { Sidebar } from "../components/Sidebar";
import { TitleBar } from "../components/TitleBar";
import { HomePage } from "../features/home/HomePage";
import { ComingSoon } from "../features/placeholder/ComingSoon";
import { SettingsPage } from "../features/settings/SettingsPage";
import { applyLanguage } from "../i18n";
import { useApp, type View } from "../stores/app";

function Page({ view }: { view: View }) {
  switch (view) {
    case "home":
      return <HomePage />;
    case "settings":
      return <SettingsPage />;
    default:
      return <ComingSoon view={view} />;
  }
}

export function App() {
  const { t } = useTranslation();
  const { status, settings, view, notice, fatal, load, dismissNotice } = useApp();

  useEffect(() => {
    void load();
  }, [load]);

  // Apply theme + language whenever settings change.
  useEffect(() => {
    if (!settings) return;
    document.documentElement.dataset.accent = settings.accent;
    applyLanguage(settings.language);
  }, [settings]);

  return (
    <MotionConfig reducedMotion="user">
      <div className="flex h-full flex-col overflow-hidden border border-line bg-bg">
        <TitleBar />

        {status === "loading" && (
          <div className="grid flex-1 place-items-center">
            <Logo className="h-14 w-14 animate-pulse text-accent neon-drop" />
          </div>
        )}

        {status === "fatal" && fatal && (
          <div className="mx-auto flex max-w-2xl flex-1 flex-col justify-center gap-4 p-8">
            <h1 className="font-display text-3xl font-bold text-danger">{t("startup.title")}</h1>
            <p className="text-sm text-muted">{t("startup.hint")}</p>
            <ErrorNotice error={fatal} />
          </div>
        )}

        {status === "ready" && settings && (
          <div className="relative flex min-h-0 flex-1">
            {settings.backgroundEffects && <NeonBackground accent={settings.accent} />}
            <Sidebar />
            <main className="relative z-0 min-w-0 flex-1 overflow-y-auto p-6">
              {notice && (
                <div className="mb-4">
                  <ErrorNotice error={notice} onDismiss={dismissNotice} />
                </div>
              )}
              <AnimatePresence mode="wait">
                <motion.div
                  key={view}
                  className="h-full"
                  initial={{ opacity: 0, y: 8 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -8 }}
                  transition={{ duration: 0.16, ease: "easeOut" }}
                >
                  <Page view={view} />
                </motion.div>
              </AnimatePresence>
            </main>
          </div>
        )}
      </div>
    </MotionConfig>
  );
}
