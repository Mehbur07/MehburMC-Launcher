import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { FolderInput, FolderOpen, RotateCcw, Undo2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button, Modal, ProgressBar } from "../../components/ui";
import { formatBytes } from "../../lib/format";
import { ipc, onDataMove, toErrorPayload } from "../../lib/ipc";
import type { ErrorPayload } from "../../lib/ipc/bindings/ErrorPayload";
import { useApp } from "../../stores/app";
import { isActive, useTasks } from "../../stores/tasks";

type Phase =
  | { step: "confirm"; dest: string }
  | { step: "moving"; dest: string; done: number; total: number }
  | { step: "done"; leftovers: number }
  | { step: "error"; error: ErrorPayload };

/** Data folder info + "Move data folder" (copy → verify → switch → delete). */
export function DataFolderSection() {
  const { t } = useTranslation();
  const boot = useApp((s) => s.boot);
  const busy = useTasks((s) => Object.values(s.tasks).some(isActive));
  const [phase, setPhase] = useState<Phase | null>(null);
  if (!boot) return null;

  const pick = async () => {
    const dest = await openDialog({ directory: true, multiple: false });
    if (typeof dest === "string") setPhase({ step: "confirm", dest });
  };

  const moveBack = async () => {
    try {
      setPhase({ step: "confirm", dest: await ipc.defaultDataFolder() });
    } catch (e) {
      useApp.setState({ notice: toErrorPayload(e) });
    }
  };

  const start = async (dest: string) => {
    setPhase({ step: "moving", dest, done: 0, total: 0 });
    const unlisten = await onDataMove((p) =>
      setPhase({ step: "moving", dest, done: p.done, total: p.total }),
    );
    try {
      const leftovers = await ipc.moveDataFolder(dest);
      setPhase({ step: "done", leftovers });
    } catch (e) {
      setPhase({ step: "error", error: toErrorPayload(e) });
    } finally {
      unlisten();
    }
  };

  const moving = phase?.step === "moving";

  return (
    <div className="flex flex-col gap-3 py-3">
      <div className="flex items-center gap-2 text-xs">
        <span className="rounded-full border border-accent/40 bg-accent/10 px-2 py-0.5 text-accent">
          {t(`settings.dataMode.${boot.paths?.mode ?? "standard"}`)}
        </span>
        {boot.paths?.redirected && (
          <span className="rounded-full border border-warn/40 bg-warn/10 px-2 py-0.5 text-warn">
            {t("settings.redirected")}
          </span>
        )}
      </div>
      <code data-selectable className="rounded-md bg-bg/70 px-3 py-2 text-xs break-all text-muted">
        {boot.paths?.content}
      </code>
      <div className="flex flex-wrap gap-2">
        <Button
          onClick={() =>
            void ipc.openDataDir().catch((e) => useApp.setState({ notice: toErrorPayload(e) }))
          }
        >
          <FolderOpen size={16} />
          {t("settings.openDataFolder")}
        </Button>
        <Button
          variant="ghost"
          disabled={busy}
          title={busy ? t("settings.moveBusy") : undefined}
          onClick={() => void pick()}
        >
          <FolderInput size={16} />
          {t("settings.moveDataFolder")}
        </Button>
        {boot.paths?.redirected && (
          <Button variant="ghost" disabled={busy} onClick={() => void moveBack()}>
            <Undo2 size={16} />
            {t("settings.moveBack")}
          </Button>
        )}
      </div>
      {boot.paths?.mode === "portable" && (
        <p className="text-xs text-muted">{t("settings.portableHint")}</p>
      )}

      <Modal
        open={phase !== null}
        onClose={() => !moving && setPhase(null)}
        title={t("settings.moveDataFolder")}
        footer={
          phase?.step === "confirm" ? (
            <>
              <Button variant="ghost" onClick={() => setPhase(null)}>
                {t("common.cancel")}
              </Button>
              <Button variant="primary" onClick={() => void start(phase.dest)}>
                {t("settings.moveStart")}
              </Button>
            </>
          ) : phase?.step === "done" ? (
            <Button variant="primary" onClick={() => void ipc.restartApp()}>
              <RotateCcw size={14} />
              {t("settings.restart")}
            </Button>
          ) : phase?.step === "error" ? (
            <Button variant="ghost" onClick={() => setPhase(null)}>
              {t("common.close")}
            </Button>
          ) : undefined
        }
      >
        {phase?.step === "confirm" && (
          <div className="flex flex-col gap-3 text-sm">
            <p className="text-muted">{t("settings.moveConfirm")}</p>
            <code data-selectable className="rounded-md bg-bg/70 px-3 py-2 text-xs break-all">
              {phase.dest}
            </code>
          </div>
        )}
        {phase?.step === "moving" && (
          <div className="flex flex-col gap-3 text-sm">
            <p className="text-muted">{t("settings.moving")}</p>
            <ProgressBar value={phase.total ? phase.done / phase.total : 0} />
            <p className="text-xs text-muted">
              {formatBytes(phase.done)} / {formatBytes(phase.total)}
            </p>
          </div>
        )}
        {phase?.step === "done" && (
          <p className="text-sm text-muted">
            {phase.leftovers > 0
              ? t("settings.movedLeftovers", { n: phase.leftovers })
              : t("settings.moved")}
          </p>
        )}
        {phase?.step === "error" && (
          <p className="text-sm text-warn">
            {t(`errors.${phase.error.code}`, {
              ...phase.error.params,
              defaultValue: phase.error.detail,
            })}
          </p>
        )}
      </Modal>
    </div>
  );
}
