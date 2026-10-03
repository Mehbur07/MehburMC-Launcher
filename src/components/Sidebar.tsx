import {
  Boxes,
  Compass,
  Download,
  Play,
  Settings,
  Shirt,
  SquareTerminal,
  UserRound,
  type LucideIcon,
} from "lucide-react";
import { motion } from "motion/react";
import { useTranslation } from "react-i18next";

import { useApp, type View } from "../stores/app";

const ITEMS: { view: View; icon: LucideIcon }[] = [
  { view: "home", icon: Play },
  { view: "instances", icon: Boxes },
  { view: "browse", icon: Compass },
  { view: "accounts", icon: UserRound },
  { view: "skins", icon: Shirt },
  { view: "downloads", icon: Download },
  { view: "console", icon: SquareTerminal },
];

export function Sidebar() {
  const { t } = useTranslation();
  const view = useApp((s) => s.view);
  const setView = useApp((s) => s.setView);

  const item = (v: View, Icon: LucideIcon) => {
    const active = v === view;
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
        </button>
      </li>
    );
  };

  return (
    <nav className="relative z-10 flex w-56 shrink-0 flex-col border-r border-line bg-surface-1/80 p-3 backdrop-blur">
      <ul className="flex flex-col gap-1">{ITEMS.map(({ view: v, icon }) => item(v, icon))}</ul>
      <ul className="mt-auto flex flex-col gap-1 border-t border-line pt-3">
        {item("settings", Settings)}
      </ul>
    </nav>
  );
}
