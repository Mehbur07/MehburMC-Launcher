import { getCurrentWindow } from "@tauri-apps/api/window";
import { Copy, Minus, Square, X } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { UpdateButton } from "../features/update/UpdateButton";
import { useApp } from "../stores/app";
import { Logo } from "./Logo";

export function TitleBar() {
  const { t } = useTranslation();
  const [maximized, setMaximized] = useState(false);
  const version = useApp((s) => s.boot?.version);

  useEffect(() => {
    const win = getCurrentWindow();
    let unlisten: (() => void) | undefined;
    const sync = () => void win.isMaximized().then(setMaximized);
    sync();
    void win.onResized(sync).then((u) => (unlisten = u));
    return () => unlisten?.();
  }, []);

  const win = () => getCurrentWindow();

  return (
    <header
      data-tauri-drag-region
      className="relative z-20 flex h-10 shrink-0 items-center border-b border-line bg-surface-1/90 pl-3 backdrop-blur"
    >
      {/* Thin neon line under the title bar. */}
      <div className="pointer-events-none absolute inset-x-0 -bottom-px h-px bg-gradient-to-r from-transparent via-accent/70 to-transparent" />
      <Logo className="pointer-events-none h-6 w-6 rounded" />
      <span className="pointer-events-none ml-2 font-brand text-[13px] font-bold tracking-wider">
        MehburMC <span className="text-accent">Launcher</span>
      </span>
      {version && (
        <span
          title={t("titlebar.version")}
          className="pointer-events-none ml-2 rounded border border-line bg-surface-2/70 px-1.5 py-px font-mono text-[10px] leading-4 text-muted"
        >
          v{version}
        </span>
      )}
      <UpdateButton />

      <div className="ml-auto flex h-full">
        <WindowButton label={t("titlebar.minimize")} onClick={() => void win().minimize()}>
          <Minus size={16} />
        </WindowButton>
        <WindowButton
          label={maximized ? t("titlebar.restore") : t("titlebar.maximize")}
          onClick={() => void win().toggleMaximize()}
        >
          {maximized ? <Copy size={13} /> : <Square size={13} />}
        </WindowButton>
        <WindowButton label={t("titlebar.close")} onClick={() => void win().close()} danger>
          <X size={17} />
        </WindowButton>
      </div>
    </header>
  );
}

function WindowButton(props: {
  label: string;
  onClick: () => void;
  danger?: boolean;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={props.label}
      title={props.label}
      onClick={props.onClick}
      className={`grid h-full w-12 place-items-center text-muted transition-colors ${
        props.danger ? "hover:bg-danger hover:text-white" : "hover:bg-surface-3 hover:text-accent"
      }`}
    >
      {props.children}
    </button>
  );
}
