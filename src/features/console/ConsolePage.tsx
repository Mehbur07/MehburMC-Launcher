import { useVirtualizer } from "@tanstack/react-virtual";
import {
  AlertOctagon,
  ArrowDownToLine,
  ClipboardCopy,
  Eraser,
  FileWarning,
  Save,
  Search,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { Badge, Button, EmptyState, TextInput } from "../../components/ui";
import { openFolder } from "../../lib/actions";
import { formatTime } from "../../lib/format";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { LogLevel } from "../../lib/ipc/bindings/LogLevel";
import { lineClass } from "./lineClass";
import { useApp } from "../../stores/app";
import { useInstances } from "../../stores/instances";
import { severity, useLogs, type LogLine } from "../../stores/logs";
import { activeTaskFor, useTasks } from "../../stores/tasks";

type Filter = "all" | "warn" | "error";
const RANK: Record<LogLevel, number> = { trace: 0, debug: 1, info: 2, warn: 3, error: 4, fatal: 5 };
const EMPTY: LogLine[] = [];

export function filterLines(lines: LogLine[], filter: Filter, query: string) {
  const min = filter === "all" ? 0 : RANK[filter];
  const q = query.trim().toLowerCase();
  return lines.filter((l) => RANK[severity(l)] >= min && (!q || l.text.toLowerCase().includes(q)));
}

const asText = (lines: LogLine[], locale: string) =>
  lines
    .map((l) => `[${formatTime(l.timeMs, locale)}]${l.thread ? ` [${l.thread}]` : ""} ${l.text}`)
    .join("\n");

export function ConsolePage() {
  const { t, i18n } = useTranslation();
  const instances = useInstances((s) => s.instances);
  const fallback = useInstances((s) => s.selected);
  const chosen = useApp((s) => s.consoleInstance);
  const instanceId = chosen ?? fallback;
  const lines = useLogs((s) => (instanceId ? (s.lines[instanceId] ?? EMPTY) : EMPTY));
  const exit = useLogs((s) => (instanceId ? s.exits[instanceId] : undefined));
  const clear = useLogs((s) => s.clear);
  const running = useTasks((s) => activeTaskFor(s.tasks, instanceId)?.status === "playing");

  const [filter, setFilter] = useState<Filter>("all");
  const [query, setQuery] = useState("");
  const [follow, setFollow] = useState(true);
  const scroller = useRef<HTMLDivElement>(null);

  const visible = useMemo(() => filterLines(lines, filter, query), [lines, filter, query]);
  const virtual = useVirtualizer({
    count: visible.length,
    getScrollElement: () => scroller.current,
    estimateSize: () => 20,
    overscan: 30,
  });

  useEffect(() => {
    if (follow && visible.length) virtual.scrollToIndex(visible.length - 1, { align: "end" });
  }, [follow, visible.length, virtual]);

  const onScroll = () => {
    const el = scroller.current;
    if (!el) return;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
    if (atBottom !== follow) setFollow(atBottom);
  };

  const copyAll = () =>
    void navigator.clipboard.writeText(asText(visible, i18n.language)).catch(() => {});
  const saveLog = async () => {
    try {
      const path = await ipc.pickPath("consoleLog", `mehbur-${instanceId ?? "console"}.log`);
      if (!path) return;
      await ipc.saveTextFile(asText(lines, i18n.language));
    } catch (e) {
      useApp.setState({ notice: toErrorPayload(e) });
    }
  };

  const crashed = exit && exit.code !== 0 && exit.code !== null;

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <h1 className="mr-2 font-display text-3xl font-bold tracking-wide">{t("nav.console")}</h1>
        <select
          value={instanceId ?? ""}
          onChange={(e) => useApp.setState({ consoleInstance: e.target.value || null })}
          className="rounded-md border border-line bg-surface-2 px-3 py-1.5 text-sm"
          aria-label={t("console.instance")}
        >
          {instances.map((i) => (
            <option key={i.id} value={i.id}>
              {i.name}
            </option>
          ))}
        </select>
        {running && <Badge tone="success">{t("console.running")}</Badge>}
        <div className="ml-auto flex items-center gap-2">
          <div className="flex overflow-hidden rounded-md border border-line" role="group">
            {(["all", "warn", "error"] as const).map((f) => (
              <button
                key={f}
                type="button"
                aria-pressed={filter === f}
                onClick={() => setFilter(f)}
                className={`px-2.5 py-1 text-xs ${filter === f ? "bg-accent/15 text-accent" : "text-muted hover:text-fg"}`}
              >
                {t(`console.filter.${f}`)}
              </button>
            ))}
          </div>
          <div className="relative w-48">
            <Search
              size={14}
              className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-muted"
            />
            <TextInput
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t("console.search")}
              className="py-1.5 pl-8 text-xs"
            />
          </div>
        </div>
      </div>

      {crashed && (
        <div
          role="alert"
          className="flex items-center gap-3 rounded-md border border-danger/50 bg-danger/10 px-4 py-3 text-sm"
        >
          <AlertOctagon size={18} className="shrink-0 text-danger" />
          <div className="flex-1">
            <div className="font-semibold">{t("console.crashed", { code: exit.code })}</div>
            {exit.crashReport && (
              <div className="truncate text-xs text-muted">{exit.crashReport}</div>
            )}
          </div>
          {exit.crashReport && instanceId && (
            <Button
              size="sm"
              variant="danger"
              onClick={() => void openFolder(instanceId, "crashReports")}
            >
              <FileWarning size={14} />
              {t("console.openCrash")}
            </Button>
          )}
        </div>
      )}

      <div className="relative min-h-0 flex-1 overflow-hidden rounded-md border border-line bg-bg/85">
        {lines.length === 0 ? (
          <div className="grid h-full place-items-center p-6">
            <EmptyState icon={<Search size={28} />} title={t("console.empty")}>
              <p>{t("console.emptyHint")}</p>
            </EmptyState>
          </div>
        ) : (
          <div
            ref={scroller}
            onScroll={onScroll}
            className="h-full overflow-y-auto px-3 py-2 font-mono text-xs"
            data-selectable
          >
            <div style={{ height: virtual.getTotalSize(), position: "relative" }}>
              {virtual.getVirtualItems().map((row) => {
                const l = visible[row.index]!;
                return (
                  <div
                    key={l.seq}
                    data-index={row.index}
                    ref={virtual.measureElement}
                    className={`absolute inset-x-0 leading-5 whitespace-pre-wrap ${lineClass(severity(l))}`}
                    style={{ transform: `translateY(${row.start}px)` }}
                  >
                    <span className="text-muted/70 select-none">
                      {formatTime(l.timeMs, i18n.language)}{" "}
                    </span>
                    {l.thread && <span className="text-accent-2/80">[{l.thread}] </span>}
                    {l.text}
                  </div>
                );
              })}
            </div>
          </div>
        )}
        {!follow && lines.length > 0 && (
          <button
            type="button"
            onClick={() => setFollow(true)}
            className="absolute right-4 bottom-4 flex items-center gap-1.5 rounded-full bg-accent px-3 py-1.5 text-xs font-semibold text-on-accent shadow-lg"
          >
            <ArrowDownToLine size={14} />
            {t("console.follow")}
          </button>
        )}
      </div>

      <div className="flex items-center justify-between text-xs text-muted">
        <span>{t("console.lines", { shown: visible.length, total: lines.length })}</span>
        <div className="flex gap-2">
          <Button size="sm" variant="ghost" onClick={copyAll} disabled={!visible.length}>
            <ClipboardCopy size={14} />
            {t("console.copy")}
          </Button>
          <Button size="sm" variant="ghost" onClick={() => void saveLog()} disabled={!lines.length}>
            <Save size={14} />
            {t("console.save")}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => instanceId && clear(instanceId)}
            disabled={!lines.length}
          >
            <Eraser size={14} />
            {t("console.clear")}
          </Button>
        </div>
      </div>
    </div>
  );
}
