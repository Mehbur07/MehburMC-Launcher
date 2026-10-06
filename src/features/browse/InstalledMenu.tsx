import { CheckCircle2, ChevronDown, Trash2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "../../components/ui";

/**
 * "Installed" button of a search result; opens a small menu with "Delete".
 * `onDelete` only asks for confirmation — the caller deletes.
 */
export function InstalledMenu({
  disabled,
  locked,
  onDelete,
}: {
  /** Some installed files are switched off. */
  disabled: boolean;
  /** The game is running: content cannot be removed now. */
  locked: boolean;
  onDelete: () => void;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("mousedown", close);
    window.addEventListener("keydown", esc);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("keydown", esc);
    };
  }, [open]);

  return (
    <div ref={ref} className="relative">
      <Button
        size="sm"
        variant="secondary"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
        className="border-success/50 text-success hover:bg-success/10"
      >
        <CheckCircle2 size={13} />
        {disabled ? t("browse.installedOff") : t("browse.alreadyInstalled")}
        <ChevronDown size={12} className={`transition-transform ${open ? "rotate-180" : ""}`} />
      </Button>
      {open && (
        <div
          role="menu"
          className="pop-in absolute right-0 z-20 mt-1 min-w-36 overflow-hidden rounded-md border border-line bg-surface-1 shadow-[0_8px_24px_rgb(0_0_0/0.45)]"
        >
          <button
            type="button"
            role="menuitem"
            disabled={locked}
            title={locked ? t("browse.removeLocked") : undefined}
            onClick={() => {
              setOpen(false);
              onDelete();
            }}
            className="flex w-full items-center gap-2 px-3 py-2 text-left text-sm text-danger hover:bg-danger/10 disabled:cursor-not-allowed disabled:opacity-50"
          >
            <Trash2 size={14} />
            {t("browse.remove")}
          </button>
        </div>
      )}
    </div>
  );
}
