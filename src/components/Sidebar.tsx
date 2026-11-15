import {
  Boxes,
  Compass,
  Download,
  LifeBuoy,
  Menu,
  Play,
  Server,
  Settings,
  ShieldAlert,
  Shirt,
  Sparkles,
  SquareTerminal,
  ToggleRight,
  UserRound,
  Users,
  type LucideIcon,
} from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { useApp, type View } from "../stores/app";
import { LOCAL_LOCKED, useAuth } from "../stores/auth";
import { totalUnread, useFriends } from "../stores/friends";
import { useSupport } from "../stores/support";
import { isActive, useTasks } from "../stores/tasks";

type NavItem = { view: View; icon: LucideIcon };

const ITEMS: NavItem[] = [
  { view: "home", icon: Play },
  { view: "instances", icon: Boxes },
  { view: "browse", icon: Compass },
  { view: "modToggle", icon: ToggleRight },
  { view: "accounts", icon: UserRound },
  { view: "skins", icon: Shirt },
  { view: "friends", icon: Users },
  { view: "servers", icon: Server },
  { view: "downloads", icon: Download },
  { view: "console", icon: SquareTerminal },
];

/** Pages, badge numbers and dots shared by the sidebar and the phone menu. */
function useNavState() {
  const view = useApp((s) => s.view);
  const setView = useApp((s) => s.setView);
  const whatsNewUnseen = useApp((s) => s.whatsNewUnseen);
  const unread = useFriends((s) => totalUnread(s.friends, s.groups));
  const busy = useTasks(
    (s) => Object.values(s.tasks).filter((t) => t.status === "preparing").length,
  );
  const isAdmin = useAuth((s) => (s.status?.rank ?? 0) > 0);
  const support = useSupport((s) => s.unread);
  const local = useAuth((s) => s.status?.localAccount ?? false);
  const playing = useTasks((s) =>
    Object.values(s.tasks).some((t) => isActive(t) && t.status === "playing"),
  );

  const main = ITEMS.filter(({ view: v }) => !local || !LOCAL_LOCKED.has(v));
  const extra: NavItem[] = [
    ...(isAdmin ? [{ view: "admin" as const, icon: ShieldAlert }] : []),
    ...(!local ? [{ view: "support" as const, icon: LifeBuoy }] : []),
    { view: "whatsNew", icon: Sparkles },
    { view: "settings", icon: Settings },
  ];
  /** Number next to a page (0 = none). */
  const count = (v: View): number => {
    switch (v) {
      case "downloads":
        return busy;
      case "friends":
        return unread;
      case "support":
        return support.mine;
      case "admin":
        return support.admin;
      default:
        return 0;
    }
  };
  /** Small dot next to a page: new release notes, game running. */
  const dot = (v: View): "accent" | "success" | null =>
    v === "whatsNew" && whatsNewUnseen ? "accent" : v === "console" && playing ? "success" : null;
  return { view, setView, local, main, extra, count, dot };
}

type NavState = ReturnType<typeof useNavState>;

/** The instance detail page belongs to "instances". */
const isCurrent = (v: View, view: View) => v === view || (v === "instances" && view === "instance");

function NavButton({
  nav,
  item: { view: v, icon: Icon },
  layoutId,
  onNavigate,
}: {
  nav: NavState;
  item: NavItem;
  layoutId: string;
  /** Replaces the direct page switch (the phone menu closes first). */
  onNavigate?: (view: View) => void;
}) {
  const { t } = useTranslation();
  const active = isCurrent(v, nav.view);
  const n = nav.count(v);
  const dot = nav.dot(v);
  return (
    <li>
      <button
        type="button"
        onClick={() => (onNavigate ? onNavigate(v) : nav.setView(v))}
        aria-current={active ? "page" : undefined}
        className={`group relative flex w-full items-center gap-3 rounded-md px-3 py-2.5 text-left font-display text-[15px] font-semibold tracking-wide transition-colors ${
          active ? "text-accent" : "text-muted hover:bg-surface-2 hover:text-fg"
        }`}
      >
        {active && (
          <motion.span
            layoutId={layoutId}
            className="absolute inset-0 rounded-md bg-accent/10 neon-ring"
            transition={{ type: "spring", stiffness: 500, damping: 40 }}
          />
        )}
        <Icon size={18} className={`relative ${active ? "neon-drop" : ""}`} />
        <span className="relative">{t(`nav.${v}`)}</span>
        {n > 0 && (
          <span
            key={n}
            className="pop-in relative ml-auto rounded-full bg-accent px-1.5 text-[11px] font-bold text-on-accent"
          >
            {n}
          </span>
        )}
        {dot === "accent" && (
          <span className="relative ml-auto h-2 w-2 rounded-full bg-accent shadow-[0_0_8px_rgb(var(--mc-accent-rgb))]" />
        )}
        {dot === "success" && (
          <span className="relative ml-auto h-2 w-2 animate-pulse rounded-full bg-success" />
        )}
      </button>
    </li>
  );
}

export function Sidebar() {
  const nav = useNavState();
  return (
    <nav className="relative z-10 flex w-56 shrink-0 flex-col border-r border-line bg-surface-1/80 p-3 backdrop-blur">
      <ul className="flex flex-col gap-1">
        {nav.main.map((item) => (
          <NavButton key={item.view} nav={nav} item={item} layoutId="nav-active" />
        ))}
      </ul>
      <ul className="mt-auto flex flex-col gap-1 border-t border-line pt-3">
        {nav.extra.map((item) => (
          <NavButton key={item.view} nav={nav} item={item} layoutId="nav-active" />
        ))}
      </ul>
    </nav>
  );
}

// Telefonda alt sekmelerde görünen sayfalar; gerisi Menü çekmecesinde.
const TABS: View[] = ["home", "instances", "browse", "friends"];
// Yerel hesapta Mod Tarayıcı ve Arkadaşlar kapalı (K82).
const LOCAL_TABS: View[] = ["home", "instances", "skins", "modToggle"];

/**
 * Telefon menüsü (K103): altta dört sekme + Menü. Menü bütün sayfaları
 * aşağıdan açılan bir çekmecede gösterir; arka plana dokunmak ya da Android'in
 * geri tuşu çekmeceyi kapatır.
 */
export function MobileNav() {
  const { t } = useTranslation();
  const nav = useNavState();
  const [open, setOpen] = useState(false);
  // Çekmeceden seçilen sayfa: çekmecenin geçmiş kaydı silindikten sonra açılır.
  const pending = useRef<View | null>(null);
  const setView = nav.setView;
  const all = [...nav.main, ...nav.extra];
  const tabs = (nav.local ? LOCAL_TABS : TABS)
    .map((v) => all.find((i) => i.view === v))
    .filter((i) => i !== undefined);
  const tabViews = new Set(tabs.map((i) => i.view));
  // Menüdeki bir sayfa açıksa Menü düğmesi seçili görünür; orada bir rozet
  // varsa düğmede küçük bir nokta çıkar.
  const menuActive = !tabs.some((i) => isCurrent(i.view, nav.view));
  const menuBadge = all.some(
    ({ view: v }) => !tabViews.has(v) && (nav.count(v) > 0 || nav.dot(v) === "accent"),
  );

  // Geri tuşu (WebView geçmişi): çekmece açıkken bir geçmiş kaydı eklenir,
  // geri gelince çekmece kapanır. Başka yoldan kapanırsa (seçim, arka plan)
  // kayıt geri alınır, seçilen sayfa ondan sonra açılır; böylece geçmişte
  // fazladan kayıt kalmaz.
  useEffect(() => {
    if (!open) return;
    window.history.pushState({ drawer: true }, "");
    const onPop = () => setOpen(false);
    window.addEventListener("popstate", onPop);
    return () => {
      window.removeEventListener("popstate", onPop);
      const go = () => {
        const v = pending.current;
        pending.current = null;
        if (v) setView(v);
      };
      if ((window.history.state as { drawer?: boolean } | null)?.drawer) {
        window.addEventListener("popstate", go, { once: true });
        window.history.back();
      } else {
        go();
      }
    };
  }, [open, setView]);

  return (
    <>
      <nav
        aria-label={t("nav.menu")}
        className="relative z-30 flex shrink-0 border-t border-line bg-surface-1/95 backdrop-blur"
      >
        {tabs.map(({ view: v, icon: Icon }) => {
          const active = isCurrent(v, nav.view);
          const n = nav.count(v);
          return (
            <button
              key={v}
              type="button"
              onClick={() => nav.setView(v)}
              aria-current={active ? "page" : undefined}
              className={`relative flex min-h-14 min-w-0 flex-1 flex-col items-center justify-center gap-0.5 font-display text-[12px] font-semibold tracking-wide ${
                active ? "text-accent" : "text-muted"
              }`}
            >
              {active && (
                <motion.span
                  layoutId="tab-active"
                  className="absolute inset-x-3 top-0 h-0.5 rounded-full bg-accent shadow-[0_0_8px_rgb(var(--mc-accent-rgb))]"
                />
              )}
              <span className="relative">
                <Icon size={20} className={active ? "neon-drop" : ""} />
                {n > 0 && (
                  <span className="absolute -top-1.5 -right-2.5 rounded-full bg-accent px-1 text-[10px] leading-4 font-bold text-on-accent">
                    {n}
                  </span>
                )}
              </span>
              <span className="max-w-full truncate px-1">{t(`nav.${v}`)}</span>
            </button>
          );
        })}
        <button
          type="button"
          onClick={() => setOpen(true)}
          aria-expanded={open}
          className={`relative flex min-h-14 min-w-0 flex-1 flex-col items-center justify-center gap-0.5 font-display text-[12px] font-semibold tracking-wide ${
            menuActive ? "text-accent" : "text-muted"
          }`}
        >
          <span className="relative">
            <Menu size={20} />
            {menuBadge && (
              <span className="absolute -top-0.5 -right-1 h-2 w-2 rounded-full bg-accent shadow-[0_0_8px_rgb(var(--mc-accent-rgb))]" />
            )}
          </span>
          <span>{t("nav.menu")}</span>
        </button>
      </nav>

      <AnimatePresence>
        {open && (
          <motion.div
            className="fixed inset-0 z-40 flex flex-col justify-end bg-black/60 backdrop-blur-sm"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={() => setOpen(false)}
          >
            <motion.div
              role="dialog"
              aria-label={t("nav.menu")}
              className="max-h-[85%] overflow-y-auto rounded-t-xl border-t border-accent/30 bg-surface-1 p-3 pb-4"
              initial={{ y: 40 }}
              animate={{ y: 0 }}
              exit={{ y: 40 }}
              transition={{ duration: 0.18, ease: "easeOut" }}
              onClick={(e) => e.stopPropagation()}
            >
              <div className="mx-auto mb-3 h-1 w-10 rounded-full bg-surface-3" />
              <ul className="grid grid-cols-1 gap-1 min-[400px]:grid-cols-2">
                {all.map((item) => (
                  <NavButton
                    key={item.view}
                    nav={nav}
                    item={item}
                    layoutId="drawer-active"
                    onNavigate={(v) => {
                      pending.current = v;
                      setOpen(false);
                    }}
                  />
                ))}
              </ul>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </>
  );
}
