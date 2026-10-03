import { FileText, RefreshCw, Stethoscope } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Badge, Button, EmptyState } from "../../components/ui";
import { formatBytes, formatDate } from "../../lib/format";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { FileEntry } from "../../lib/ipc/bindings/FileEntry";
import type { Folder } from "../../lib/ipc/bindings/Folder";
import type { Instance } from "../../lib/ipc/bindings/Instance";
import { useApp } from "../../stores/app";
import { useCrash } from "../../stores/crash";
import { lineClass } from "../console/ConsolePage";

type LogFile = FileEntry & { folder: Folder };

/** Game log files (logs/) and crash reports (crash-reports/). */
export function LogsTab({ inst }: { inst: Instance }) {
  const { t, i18n } = useTranslation();
  const [files, setFiles] = useState<LogFile[] | null>(null);
  const [current, setCurrent] = useState<LogFile | null>(null);
  const [content, setContent] = useState<string>("");

  const refresh = useCallback(() => {
    Promise.all([
      ipc.listInstanceFiles(inst.id, "crashReports").catch(() => []),
      ipc.listInstanceFiles(inst.id, "logs").catch(() => []),
    ]).then(([crashes, logs]) => {
      const all: LogFile[] = [
        ...crashes.filter((f) => !f.isDir).map((f) => ({ ...f, folder: "crashReports" as const })),
        ...logs
          .filter((f) => !f.isDir && !f.name.endsWith(".gz"))
          .map((f) => ({ ...f, folder: "logs" as const })),
      ];
      setFiles(all);
      setCurrent((c) => c ?? all.find((f) => f.name === "latest.log") ?? all[0] ?? null);
    });
  }, [inst.id]);
  useEffect(refresh, [refresh]);

  useEffect(() => {
    if (!current) return;
    ipc
      .readInstanceLog(inst.id, current.folder, current.name)
      .then(setContent)
      .catch((e) => setContent(toErrorPayload(e).detail));
  }, [inst.id, current]);

  if (files?.length === 0) {
    return <EmptyState icon={<FileText size={30} />} title={t("files.empty.logs")} />;
  }

  return (
    <div className="grid h-full min-h-[420px] grid-cols-[240px_minmax(0,1fr)] gap-3">
      <div className="flex flex-col gap-2 overflow-y-auto">
        <Button size="sm" variant="ghost" onClick={refresh} className="self-start">
          <RefreshCw size={14} />
          {t("common.refresh")}
        </Button>
        {files?.map((f) => (
          <button
            key={`${f.folder}/${f.name}`}
            type="button"
            onClick={() => setCurrent(f)}
            className={`rounded-md border px-3 py-2 text-left text-xs transition-colors ${
              current?.name === f.name && current.folder === f.folder
                ? "border-accent/50 bg-accent/10"
                : "border-line hover:border-accent/30"
            }`}
          >
            <div className="flex items-center gap-1.5 truncate font-medium">
              {f.folder === "crashReports" && <Badge tone="danger">{t("logs.crash")}</Badge>}
              <span className="truncate">{f.name}</span>
            </div>
            <div className="mt-0.5 text-muted">
              {formatBytes(f.size)} · {formatDate(f.modified, i18n.language)}
            </div>
          </button>
        ))}
      </div>
      <div className="flex min-h-0 flex-col gap-2">
        {current?.folder === "crashReports" && (
          <Button
            size="sm"
            variant="primary"
            className="self-end"
            onClick={() =>
              void ipc
                .analyzeCrashReport(inst.id, current.name)
                .then(useCrash.getState().show)
                .catch((e) => useApp.setState({ notice: toErrorPayload(e) }))
            }
          >
            <Stethoscope size={14} />
            {t("crash.analyze")}
          </Button>
        )}
        <pre
          data-selectable
          className="min-h-0 flex-1 overflow-auto rounded-md border border-line bg-bg/80 p-3 font-mono text-xs leading-relaxed whitespace-pre-wrap"
        >
          {content.split("\n").map((l, i) => (
            <div
              key={i}
              className={lineClass(
                /\b(ERROR|FATAL|Exception)\b/.test(l)
                  ? "error"
                  : /\bWARN\b/.test(l)
                    ? "warn"
                    : "info",
              )}
            >
              {l || " "}
            </div>
          ))}
        </pre>
      </div>
    </div>
  );
}
