import { AnimatePresence, MotionConfig, motion } from "motion/react";
import { lazy, Suspense, useEffect, type ComponentType } from "react";
import { useTranslation } from "react-i18next";

import { ErrorNotice } from "../components/ErrorNotice";
import { Logo } from "../components/Logo";
import { NeonBackground } from "../components/NeonBackground";
import { Sidebar } from "../components/Sidebar";
import { TitleBar } from "../components/TitleBar";
import { HomePage } from "../features/home/HomePage";
import { applyLanguage } from "../i18n";
import { ipc } from "../lib/ipc";
import { useAccounts } from "../stores/accounts";
import { canUse, useAuth } from "../stores/auth";
import { AuthScreen, BanScreen } from "../features/auth/AuthScreen";
import { useApp, type View } from "../stores/app";
import { useInstances } from "../stores/instances";
import { useFriends } from "../stores/friends";
import { useSkins } from "../stores/skins";
import { useUpdate } from "../stores/update";
import { CrashDialog } from "../features/crash/CrashDialog";
import { connectEvents } from "./events";

// Every screen except Home is loaded on first use (three.js for skins,
// dnd-kit, the virtualised console, …), keeping the startup bundle small.
const named = <K extends string>(load: () => Promise<Record<K, ComponentType>>, key: K) =>
  lazy(() => load().then((m) => ({ default: m[key] })));
const InstancesPage = named(() => import("../features/instances/InstancesPage"), "InstancesPage");
const InstanceDetail = named(
  () => import("../features/instance-detail/InstanceDetail"),
  "InstanceDetail",
);
const AccountsPage = named(() => import("../features/accounts/AccountsPage"), "AccountsPage");
const DownloadsPage = named(() => import("../features/downloads/DownloadsPage"), "DownloadsPage");
const ConsolePage = named(() => import("../features/console/ConsolePage"), "ConsolePage");
const SettingsPage = named(() => import("../features/settings/SettingsPage"), "SettingsPage");
const BrowsePage = named(() => import("../features/browse/BrowsePage"), "BrowsePage");
const CreateWizard = named(() => import("../features/instances/CreateWizard"), "CreateWizard");
const SkinsPage = lazy(() => import("../features/skins/SkinsPage"));
const FriendsPage = named(() => import("../features/friends/FriendsPage"), "FriendsPage");
const ModTogglePage = named(() => import("../features/modtoggle/ModTogglePage"), "ModTogglePage");
const ServersPage = named(() => import("../features/servers/ServersPage"), "ServersPage");
const WhatsNewPage = named(() => import("../features/whatsnew/WhatsNewPage"), "WhatsNewPage");
const AdminPage = named(() => import("../features/admin/AdminPage"), "AdminPage");

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
      return <SkinsPage />;
    case "friends":
      return <FriendsPage />;
    case "modToggle":
      return <ModTogglePage />;
    case "servers":
      return <ServersPage />;
    case "whatsNew":
      return <WhatsNewPage />;
    case "admin":
      return <AdminPage />;
  }
}

const pageFallback = <Logo className="mx-auto mt-24 h-12 w-12 animate-pulse rounded-lg" />;

export function App() {
  const { t } = useTranslation();
  const { status, settings, view, notice, fatal, load, dismissNotice, wizardOpen } = useApp();
  const loadInstances = useInstances((s) => s.load);
  const loadAccounts = useAccounts((s) => s.load);
  const loadSkins = useSkins((s) => s.load);
  const checkUpdate = useUpdate((s) => s.check);

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
    if (useApp.getState().settings?.checkUpdates) void checkUpdate();
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [status, loadInstances, loadAccounts, loadSkins, checkUpdate]);

  // MehburMC account (K73): saved state first (works offline), then the
  // server's view of rank and ban; re-checked every 5 minutes.
  const auth = useAuth((s) => s.status);
  const loadAuth = useAuth((s) => s.load);
  const usable = canUse(auth);
  useEffect(() => {
    if (status !== "ready") return;
    // Private MehburMC textures follow the server's grants (K74).
    const check = () =>
      loadAuth(true).then(async () => {
        if (!canUse(useAuth.getState().status)) return;
        if (await ipc.syncPrivateTextures().catch(() => false)) void loadSkins();
      });
    void loadAuth(false).then(check);
    const id = window.setInterval(() => void check(), 5 * 60_000);
    return () => window.clearInterval(id);
  }, [status, loadAuth, loadSkins]);

  // Friend list (unread badge) every 20 s once friends are on.
  const friendsStatus = useFriends((s) => s.status);
  const refreshFriends = useFriends((s) => s.refresh);
  useEffect(() => {
    if (status !== "ready" || !usable) return;
    void friendsStatus();
    const id = window.setInterval(() => void refreshFriends(), 20_000);
    return () => window.clearInterval(id);
  }, [status, usable, friendsStatus, refreshFriends]);

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
            <Logo className="h-20 w-20 animate-pulse rounded-xl" />
          </div>
        )}

        {status === "fatal" && fatal && (
          <div className="mx-auto flex max-w-2xl flex-1 flex-col justify-center gap-4 p-8">
            <h1 className="font-display text-3xl font-bold text-danger">{t("startup.title")}</h1>
            <p className="text-sm text-muted">{t("startup.hint")}</p>
            <ErrorNotice error={fatal} />
          </div>
        )}

        {status === "ready" && settings && !usable && (
          <div className="relative flex min-h-0 flex-1">
            {settings.backgroundEffects && <NeonBackground accent={settings.accent} />}
            {auth === null ? (
              <div className="grid flex-1 place-items-center">
                <Logo className="h-20 w-20 animate-pulse rounded-xl" />
              </div>
            ) : auth.signedIn ? (
              <BanScreen status={auth} />
            ) : (
              <AuthScreen status={auth} />
            )}
          </div>
        )}

        {status === "ready" && settings && usable && (
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
                  <Suspense fallback={pageFallback}>
                    <Page view={view} />
                  </Suspense>
                </motion.div>
              </AnimatePresence>
            </main>
            {wizardOpen && (
              <Suspense fallback={null}>
                <CreateWizard />
              </Suspense>
            )}
            <CrashDialog />
          </div>
        )}
      </div>
    </MotionConfig>
  );
}
