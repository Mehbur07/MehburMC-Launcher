import {
  AlertTriangle,
  Copy,
  Cpu,
  FileText,
  Layers,
  MemoryStick,
  Puzzle,
  SquareTerminal,
  Wrench,
  type LucideIcon,
} from "lucide-react";
import { motion } from "motion/react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button, Modal } from "../../components/ui";
import { repair } from "../../lib/actions";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { CrashKind } from "../../lib/ipc/bindings/CrashKind";
import { useApp } from "../../stores/app";
import { useCrash } from "../../stores/crash";

const ICONS: Record<CrashKind, LucideIcon> = {
  outOfMemory: MemoryStick,
  heapReserve: MemoryStick,
  javaTooOld: Cpu,
  javaTooNew: Cpu,
  missingDependency: Puzzle,
  incompatibleMods: Puzzle,
  duplicateMod: Layers,
  mixinFailure: Puzzle,
  missingClass: Puzzle,
  graphicsDriver: Cpu,
  nativeCrash: AlertTriangle,
  corruptFile: Wrench,
};

/** Which instance tab helps with each problem. */
const FIX_TAB: Partial<Record<CrashKind, string>> = {
  outOfMemory: "settings",
  heapReserve: "settings",
  javaTooOld: "settings",
  javaTooNew: "settings",
  missingDependency: "mods",
  incompatibleMods: "mods",
  duplicateMod: "mods",
  mixinFailure: "mods",
  missingClass: "mods",
};

/** Shown after a game crash (or for a crash report from the Logs tab). */
export function CrashDialog() {
  const { t } = useTranslation();
  const info = useCrash((s) => s.info);
  const closeStore = useCrash((s) => s.close);
  const openInstance = useApp((s) => s.openInstance);
  const openConsole = useApp((s) => s.openConsole);
  const [copied, setCopied] = useState(false);
  const close = () => {
    setCopied(false);
    closeStore();
  };

  if (!info) return null;

  const first = info.diagnoses[0]?.kind;
  const fixTab = first ? FIX_TAB[first] : undefined;

  const open = (path: string | null) => {
    if (!path) return;
    ipc.openDataFile(path).catch((e) => useApp.setState({ notice: toErrorPayload(e) }));
  };

  const report = () =>
    [
      `${info.instanceName} — exit ${info.exitCode ?? "?"}`,
      info.summary ?? "",
      ...info.diagnoses.map((d) => `${d.kind}${d.detail ? `: ${d.detail}` : ""}`),
      info.crashReport ?? "",
    ]
      .filter(Boolean)
      .join("\n");

  return (
    <Modal
      open
      wide
      onClose={close}
      title={t("crash.title", { name: info.instanceName })}
      footer={
        <>
          <Button
            variant="ghost"
            onClick={() => {
              void navigator.clipboard.writeText(report()).then(() => setCopied(true));
            }}
          >
            <Copy size={14} />
            {copied ? t("common.copied") : t("common.copyDetails")}
          </Button>
          {info.crashReport && (
            <Button variant="ghost" onClick={() => open(info.crashReport)}>
              <FileText size={14} />
              {t("crash.openReport")}
            </Button>
          )}
          {info.exitCode !== null && (
            <Button
              variant="ghost"
              onClick={() => {
                openConsole(info.instanceId);
                close();
              }}
            >
              <SquareTerminal size={14} />
              {t("home.showConsole")}
            </Button>
          )}
          {first === "corruptFile" ? (
            <Button
              variant="primary"
              onClick={() => {
                void repair(info.instanceId);
                close();
              }}
            >
              <Wrench size={14} />
              {t("crash.repair")}
            </Button>
          ) : (
            fixTab && (
              <Button
                variant="primary"
                onClick={() => {
                  openInstance(info.instanceId, fixTab);
                  close();
                }}
              >
                {t(`crash.fix.${fixTab}`)}
              </Button>
            )
          )}
        </>
      }
    >
      <div className="flex flex-col gap-4">
        <p className="text-sm text-muted">
          {info.exitCode !== null
            ? t("crash.exitCode", { code: info.exitCode })
            : t("crash.fromReport")}
        </p>

        {info.summary && (
          <pre
            data-selectable
            className="max-h-28 overflow-auto rounded-md border border-danger/30 bg-danger/5 px-3 py-2 font-mono text-xs whitespace-pre-wrap text-danger"
          >
            {info.summary}
          </pre>
        )}

        {info.diagnoses.length === 0 ? (
          <div className="rounded-md border border-line bg-surface-2/60 p-4 text-sm">
            <div className="font-semibold">{t("crash.unknownTitle")}</div>
            <p className="mt-1 text-muted">{t("crash.unknownHint")}</p>
          </div>
        ) : (
          <ul className="flex flex-col gap-2">
            {info.diagnoses.map((d, i) => {
              const Icon = ICONS[d.kind];
              return (
                <motion.li
                  key={`${d.kind}-${i}`}
                  initial={{ opacity: 0, x: -8 }}
                  animate={{ opacity: 1, x: 0 }}
                  transition={{ delay: 0.05 * i, duration: 0.18 }}
                  className={`flex gap-3 rounded-md border p-3 ${
                    i === 0 ? "border-warn/50 bg-warn/5" : "border-line bg-surface-2/60"
                  }`}
                >
                  <Icon
                    size={20}
                    className={`mt-0.5 shrink-0 ${i === 0 ? "text-warn" : "text-muted"}`}
                  />
                  <div className="min-w-0">
                    <div className="font-semibold">{t(`crash.kind.${d.kind}.title`)}</div>
                    <p className="mt-0.5 text-sm text-muted">
                      {t(`crash.kind.${d.kind}.hint`, { detail: d.detail ?? "" })}
                    </p>
                    {d.detail && (
                      <code
                        data-selectable
                        className="mt-1.5 block rounded bg-bg/70 px-2 py-1 text-xs break-all text-fg/80"
                      >
                        {d.detail}
                      </code>
                    )}
                  </div>
                </motion.li>
              );
            })}
          </ul>
        )}

        {(info.hsErr || info.logFile) && (
          <div className="flex flex-wrap gap-2 text-xs">
            {info.logFile && (
              <button
                type="button"
                onClick={() => open(info.logFile)}
                className="text-accent hover:underline"
              >
                logs/latest.log
              </button>
            )}
            {info.hsErr && (
              <button
                type="button"
                onClick={() => open(info.hsErr)}
                className="text-accent hover:underline"
              >
                {info.hsErr.split(/[\\/]/).pop()}
              </button>
            )}
          </div>
        )}
      </div>
    </Modal>
  );
}
