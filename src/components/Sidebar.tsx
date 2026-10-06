import {
  Boxes,
  Compass,
  Download,
  Play,
  Server,
  Settings,
  Shirt,
  Sparkles,
  SquareTerminal,
  UserRound,
  Users,
  type LucideIcon,
} from "lucide-react";
import { motion } from "motion/react";
import { useTranslation } from "react-i18next";

import { useApp, type View } from "../stores/app";
import { totalUnread, useFriends } from "../stores/friends";
import { isActive, useTasks } from "../stores/tasks";

const ITEMS: { view: View; icon: LucideIcon }[] = [
  { view: "home", icon: Play },
  { view: "instances", icon: Boxes },
  { view: "browse", icon: Compass },
  { view: "accounts", icon: UserRound },
  { view: "skins", icon: Shirt },
  { view: "friends", icon: Users },
  { view: "servers", icon: Server },
  { view: "downloads", icon: Download },
  { view: "console", icon: SquareTerminal },
];

export function Sidebar() {
  const { t } = useTranslation();
  const view = useApp((s) => s.view);
  const setView = useApp((s) => s.setView);
  const whatsNewUnseen = useApp((s) => s.whatsNewUnseen);
  const unread = useFriends((s) => totalUnread(s.friends));
  const busy = useTasks(
    (s) => Object.values(s.tasks).filter((t) => t.status === "preparing").length,
  );
  const playing = useTasks((s) =>
    Object.values(s.tasks).some((t) => isActive(t) && t.status === "playing"),
  );

  const item = (v: View, Icon: LucideIcon) => {
    // The instance detail page belongs to "instances".
    const active = v === view || (v === "instances" && view === "instance");
    return (
      <li key={v}>
        <button
          type="button"
          onClick={() => setView(v)}
          aria-current={active ? "page" : undefined}
          className={`group relative flex w-full items-center gap-3 rounded-md px-3 py-2.5 text-left font-display text-[15px] font-semibold tracking-wide transition-colors ${
            active ? "text-accent" : "text-muted hover:bg-surface-2 hover:text-fg"
          }`}
        >
          {active && (
            <motion.span
              layoutId="nav-active"
              className="absolute inset-0 rounded-md bg-accent/10 neon-ring"
              transition={{ type: "spring", stiffness: 500, damping: 40 }}
            />
          )}
          <Icon size={18} className={`relative ${active ? "neon-drop" : ""}`} />
          <span className="relative">{t(`nav.${v}`)}</span>
          {v === "downloads" && busy > 0 && (
            <span
              key={busy}
              className="pop-in relative ml-auto rounded-full bg-accent px-1.5 text-[11px] font-bold text-on-accent"
            >
              {busy}
            </span>
          )}
          {v === "friends" && unread > 0 && (
            <span
              key={unread}
              className="pop-in relative ml-auto rounded-full bg-accent px-1.5 text-[11px] font-bold text-on-accent"
            >
              {unread}
            </span>
          )}
          {v === "whatsNew" && whatsNewUnseen && (
            <span className="relative ml-auto h-2 w-2 rounded-full bg-accent shadow-[0_0_8px_rgb(var(--mc-accent-rgb))]" />
          )}
          {v === "console" && playing && (
            <span className="relative ml-auto h-2 w-2 animate-pulse rounded-full bg-success" />
          )}
        </button>
      </li>
    );
  };

  return (
    <nav className="relative z-10 flex w-56 shrink-0 flex-col border-r border-line bg-surface-1/80 p-3 backdrop-blur">
      <ul className="flex flex-col gap-1">{ITEMS.map(({ view: v, icon }) => item(v, icon))}</ul>
      <ul className="mt-auto flex flex-col gap-1 border-t border-line pt-3">
        {item("whatsNew", Sparkles)}
        {item("settings", Settings)}
      </ul>
    </nav>
  );
}
