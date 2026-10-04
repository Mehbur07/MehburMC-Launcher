import { CheckCircle2, Download, Pause, Play, RotateCcw, Trash2, X } from "lucide-react";
import { useTranslation } from "react-i18next";

import { ErrorNotice } from "../../components/ErrorNotice";
import { Badge, Button, EmptyState, IconButton, ProgressBar } from "../../components/ui";
import { play, repair } from "../../lib/actions";
import { formatBytes, formatDuration } from "../../lib/format";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { TaskInfo } from "../../lib/ipc/bindings/TaskInfo";
import type { TaskStatus } from "../../lib/ipc/bindings/TaskStatus";
import { useApp } from "../../stores/app";
import { eta, fraction, isActive, useTasks } from "../../stores/tasks";
import { usePlayTimeUnits } from "../instances/loaderLabels";

const TONE: Record<TaskStatus, "accent" | "success" | "warn" | "danger" | "neutral"> = {
  preparing: "accent",
  playing: "success",
  completed: "success",
  failed: "danger",
  cancelled: "neutral",
  paused: "warn",
};

const notify = (e: unknown) => useApp.setState({ notice: toErrorPayload(e) });

export function DownloadsPage() {
  const { t } = useTranslation();
  const tasks = useTasks((s) => s.tasks);
  const setAll = useTasks((s) => s.setAll);
  const list = Object.values(tasks).sort((a, b) => b.startedAt - a.startedAt);
  const hasFinished = list.some((x) => !isActive(x));

  const clear = async () => {
    try {
      setAll(await ipc.clearTasks());
    } catch (e) {
      notify(e);
    }
  };

  return (
    <div className="mx-auto flex max-w-4xl flex-col gap-4 pb-6">
      <div className="flex items-center justify-between">
        <h1 className="font-display text-3xl font-bold tracking-wide">{t("nav.downloads")}</h1>
        <Button size="sm" variant="ghost" disabled={!hasFinished} onClick={() => void clear()}>
          <Trash2 size={14} />
          {t("downloads.clear")}
        </Button>
      </div>
      {list.length === 0 && (
        <EmptyState icon={<Download size={32} />} title={t("downloads.empty")}>
          <p>{t("downloads.emptyHint")}</p>
        </EmptyState>
      )}
      {list.map((task, i) => (
        <div key={task.id} className="rise-in" style={{ ["--i" as string]: i }}>
          <TaskRow task={task} />
        </div>
      ))}
    </div>
  );
}

function TaskRow({ task }: { task: TaskInfo }) {
  const { t } = useTranslation();
  const units = usePlayTimeUnits();
  const bps = useTasks((s) => s.speed[task.id]?.bps);
  const preparing = task.status === "preparing";
  const left = preparing ? eta(task, bps) : null;

  const cancel = (pause: boolean) => void ipc.cancelTask(task.id, pause).catch(notify);
  const restart = () => {
    if (!task.instanceId) return;
    void (task.kind === "repair" ? repair(task.instanceId) : play(task.instanceId));
  };

  return (
    <div className="rounded-lg border border-line bg-surface-1/85 p-4 backdrop-blur">
      <div className="flex items-center gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate font-display text-lg font-semibold">{task.title}</span>
            <Badge>{t(`downloads.kind.${task.kind}`)}</Badge>
            <Badge tone={TONE[task.status]}>{t(`downloads.status.${task.status}`)}</Badge>
          </div>
          {preparing && task.stage && (
            <div className="mt-0.5 text-xs text-muted">
              {t(`stage.${task.stage}`)}
              {task.total > 0 && ` · ${task.done}/${task.total}`}
              {task.bytesTotal > 0 &&
                ` · ${formatBytes(task.bytesDone)} / ${formatBytes(task.bytesTotal)}`}
            </div>
          )}
        </div>
        <div className="flex items-center gap-1">
          {preparing && (
            <>
              <IconButton label={t("downloads.pause")} onClick={() => cancel(true)}>
                <Pause size={16} />
              </IconButton>
              <IconButton
                label={t("common.cancel")}
                onClick={() => cancel(false)}
                className="hover:text-danger"
              >
                <X size={16} />
              </IconButton>
            </>
          )}
          {task.status === "paused" && (
            <Button size="sm" onClick={restart}>
              <Play size={14} />
              {t("downloads.resume")}
            </Button>
          )}
          {(task.status === "failed" || task.status === "cancelled") && (
            <Button size="sm" onClick={restart}>
              <RotateCcw size={14} />
              {t("common.retry")}
            </Button>
          )}
          {task.status === "completed" && <CheckCircle2 size={18} className="text-success" />}
        </div>
      </div>

      {preparing && (
        <div className="mt-3 flex items-center gap-3">
          <ProgressBar value={fraction(task)} />
          <span className="w-44 shrink-0 text-right text-xs text-muted tabular-nums">
            {bps ? `${formatBytes(bps)}/s` : ""}
            {left !== null && ` · ${formatDuration(left, units)}`}
          </span>
        </div>
      )}
      {task.status === "failed" && task.error && (
        <div className="mt-3">
          <ErrorNotice error={task.error} />
        </div>
      )}
    </div>
  );
}
