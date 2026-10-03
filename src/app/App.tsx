import { AnimatePresence, MotionConfig, motion } from "motion/react";
import { lazy, Suspense, useEffect } from "react";
import { useTranslation } from "react-i18next";

import { ErrorNotice } from "../components/ErrorNotice";
import { Logo } from "../components/Logo";
import { NeonBackground } from "../components/NeonBackground";
import { Sidebar } from "../components/Sidebar";
import { TitleBar } from "../components/TitleBar";
import { AccountsPage } from "../features/accounts/AccountsPage";
import { ConsolePage } from "../features/console/ConsolePage";
import { DownloadsPage } from "../features/downloads/DownloadsPage";
import { HomePage } from "../features/home/HomePage";
import { InstanceDetail } from "../features/instance-detail/InstanceDetail";
import { CreateWizard } from "../features/instances/CreateWizard";
import { InstancesPage } from "../features/instances/InstancesPage";
import { BrowsePage } from "../features/browse/BrowsePage";
import { SettingsPage } from "../features/settings/SettingsPage";
import { applyLanguage } from "../i18n";
import { useAccounts } from "../stores/accounts";
import { useApp, type View } from "../stores/app";
import { useInstances } from "../stores/instances";
import { useSkins } from "../stores/skins";
import { connectEvents } from "./events";

// three.js + skinview3d are only loaded when the skins screen opens.
const SkinsPage = lazy(() => import("../features/skins/SkinsPage"));

function Page({ view }: { view: View }) {
  switch (view) {
    case "home":
      return <HomePage />;
    case "instances":
      return <InstancesPage />;
    case "instance":
      return <InstanceDetail />;
    case "accounts":
      return <AccountsPage />;
    case "downloads":
      return <DownloadsPage />;
    case "console":
      return <ConsolePage />;
    case "settings":
      return <SettingsPage />;
    case "browse":
      return <BrowsePage />;
    case "skins":
      return (
        <Suspense fallback={<Logo className="mx-auto mt-24 h-10 w-10 animate-pulse text-accent" />}>
          <SkinsPage />
        </Suspense>
      );
  }
}

export function App() {
  const { t } = useTranslation();
  const { status, settings, view, notice, fatal, load, dismissNotice } = useApp();
  const loadInstances = useInstances((s) => s.load);
  const loadAccounts = useAccounts((s) => s.load);
  const loadSkins = useSkins((s) => s.load);

  useEffect(() => {
    void load();
  }, [load]);

  // Once the backend is usable: subscribe to events, then load data.
  useEffect(() => {
    if (status !== "ready") return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void connectEvents().then((u) => {
      if (cancelled) u();
      else unlisten = u;
    });
    void loadInstances();
    void loadAccounts();
    void loadSkins();
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [status, loadInstances, loadAccounts, loadSkins]);

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
            <main className="relative z-0 flex min-w-0 flex-1 flex-col overflow-y-auto p-6">
              {notice && (
                <div className="mb-4">
                  <ErrorNotice error={notice} onDismiss={dismissNotice} />
                </div>
              )}
              <AnimatePresence mode="wait">
                <motion.div
                  key={view}
                  className="min-h-0 flex-1"
                  initial={{ opacity: 0, y: 8 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -8 }}
                  transition={{ duration: 0.16, ease: "easeOut" }}
                >
                  <Page view={view} />
                </motion.div>
              </AnimatePresence>
            </main>
            <CreateWizard />
          </div>
        )}
      </div>
    </MotionConfig>
  );
}
