import { Clock, MemoryStick, Play, Plus, Square, SquareTerminal, Timer, X } from "lucide-react";
import { useTranslation } from "react-i18next";

import { InstanceIcon } from "../../components/InstanceIcon";
import { Badge, Button, IconButton, ProgressBar } from "../../components/ui";
import { play, stop } from "../../lib/actions";
import { formatBytes, formatDate, formatDuration } from "../../lib/format";
import { ipc } from "../../lib/ipc";
import { selectedAccount, useAccounts } from "../../stores/accounts";
import { useApp } from "../../stores/app";
import { selectedInstance, useInstances } from "../../stores/instances";
import { accountSkin, useSkins } from "../../stores/skins";
import { activeTaskFor, fraction, useTasks } from "../../stores/tasks";
import { loaderLabel, usePlayTimeUnits } from "../instances/loaderLabels";
import { SkinThumb } from "../skins/SkinThumb";
import { NewsFeed } from "./NewsFeed";

export function HomePage() {
  const { t, i18n } = useTranslation();
  const units = usePlayTimeUnits();
  const inst = useInstances(selectedInstance);
  const instances = useInstances((s) => s.instances);
  const select = useInstances((s) => s.select);
  const account = useAccounts(selectedAccount);
  const skin = useSkins((s) => accountSkin(s, account?.id));
  const defaultMem = useApp((s) => s.settings?.defaultMemoryMb ?? 4096);
  const setView = useApp((s) => s.setView);
  const setWizard = useApp((s) => s.setWizard);
  const openConsole = useApp((s) => s.openConsole);
  const task = useTasks((s) => activeTaskFor(s.tasks, inst?.id ?? null));

  if (!inst) {
    return (
      <div className="flex flex-col gap-5">
        <section className="relative overflow-hidden rounded-lg border border-line bg-surface-1/80 p-10 text-center backdrop-blur">
          <div className="pointer-events-none absolute -top-24 -right-24 h-72 w-72 rounded-full bg-accent/10 blur-3xl" />
          <h1 className="font-display text-4xl font-bold tracking-wide">
            MehburMC <span className="text-accent neon-text">Launcher</span>
          </h1>
          <p className="mx-auto mt-3 max-w-md text-sm text-muted">{t("home.welcome")}</p>
          <Button variant="primary" className="mt-6" onClick={() => setWizard(true)}>
            <Plus size={16} />
            {t("instances.create")}
          </Button>
        </section>
        <NewsFeed />
      </div>
    );
  }

  const memoryMb = inst.memoryMb ?? defaultMem;
  const preparing = task?.status === "preparing";
  const playing = task?.status === "playing";

  return (
    <div className="flex flex-col gap-5">
      <section
        key={inst.id}
        className="rise-in relative overflow-hidden rounded-lg border border-line bg-surface-1/80 p-7 backdrop-blur"
      >
        <div className="pointer-events-none absolute -top-24 -right-24 h-72 w-72 rounded-full bg-accent/10 blur-3xl" />
        <div className="relative flex items-end justify-between gap-8">
          <div className="flex min-w-0 items-center gap-5">
            <InstanceIcon icon={inst.icon} size={84} />
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-1.5">
                <Badge tone="accent">{inst.mcVersion}</Badge>
                <Badge>{loaderLabel(inst.loader.kind)}</Badge>
                {playing && <Badge tone="success">{t("home.running")}</Badge>}
              </div>
              <h1
                className="mt-2 truncate font-display text-4xl font-bold tracking-wide"
                title={inst.name}
              >
                {inst.name}
              </h1>
              <div className="mt-2 flex flex-wrap gap-4 text-xs text-muted">
                <span className="flex items-center gap-1.5">
                  <Clock size={13} />
                  {inst.lastPlayed
                    ? formatDate(inst.lastPlayed, i18n.language)
                    : t("instance.never")}
                </span>
                <span className="flex items-center gap-1.5">
                  <Timer size={13} />
                  {formatDuration(inst.playTimeSecs, units)}
                </span>
                <span className="flex items-center gap-1.5">
                  <MemoryStick size={13} />
                  {(memoryMb / 1024).toFixed(1)} GB
                </span>
              </div>
            </div>
          </div>

          <div className="flex w-72 shrink-0 flex-col items-stretch gap-3">
            <button
              type="button"
              onClick={() => setView("accounts")}
              className="flex items-center justify-end gap-2 text-xs text-muted hover:text-accent"
            >
              <SkinThumb
                src={skin?.dataUri ?? null}
                variant="head"
                unit={2}
                className="h-4 w-4 rounded-sm"
              />
              {account ? account.name : t("home.noAccount")}
            </button>

            {playing ? (
              <Button
                variant="danger"
                className="py-4 font-brand text-lg tracking-[0.15em]"
                onClick={() => void stop(inst.id)}
              >
                <Square size={18} fill="currentColor" />
                {t("home.stop")}
              </Button>
            ) : preparing && task ? (
              <div className="rounded-lg border border-accent/40 bg-accent/5 p-3">
                <div className="mb-2 flex items-center justify-between text-xs">
                  <span className="font-semibold text-accent">
                    {task.stage ? t(`stage.${task.stage}`) : t("home.preparing")}
                  </span>
                  <span className="flex items-center gap-1 text-muted tabular-nums">
                    {task.bytesTotal > 0
                      ? `${formatBytes(task.bytesDone)} / ${formatBytes(task.bytesTotal)}`
                      : `${Math.round(fraction(task) * 100)}%`}
                    <IconButton
                      label={t("common.cancel")}
                      className="h-6 w-6 hover:text-danger"
                      onClick={() => void ipc.cancelTask(task.id, false)}
                    >
                      <X size={14} />
                    </IconButton>
                  </span>
                </div>
                <ProgressBar value={fraction(task)} />
              </div>
            ) : (
              <button
                type="button"
                disabled={!account}
                title={account ? undefined : t("home.noAccount")}
                onClick={() => void play(inst.id)}
                className="play-pulse flex items-center justify-center gap-3 rounded-lg bg-accent px-10 py-4 font-brand text-xl font-bold tracking-[0.2em] text-on-accent transition-transform enabled:hover:scale-[1.03] disabled:animate-none disabled:cursor-not-allowed disabled:opacity-50"
              >
                <Play size={22} fill="currentColor" />
                {t("home.play")}
              </button>
            )}
            {!account && (
              <Button size="sm" variant="ghost" onClick={() => setView("accounts")}>
                {t("home.addAccount")}
              </Button>
            )}
            {task && (
              <Button size="sm" variant="ghost" onClick={() => openConsole(inst.id)}>
                <SquareTerminal size={14} />
                {t("home.showConsole")}
              </Button>
            )}
          </div>
        </div>
      </section>

      {instances.length > 1 && (
        <section>
          <h2 className="mb-2 text-xs font-semibold tracking-[0.15em] text-muted uppercase">
            {t("home.switch")}
          </h2>
          <div className="flex gap-2 overflow-x-auto pb-1">
            {instances.map((i) => (
              <button
                key={i.id}
                type="button"
                onClick={() => void select(i.id)}
                className={`flex shrink-0 items-center gap-2 rounded-md border px-3 py-2 text-sm transition-colors ${
                  i.id === inst.id
                    ? "border-accent/60 bg-accent/10 text-accent"
                    : "border-line bg-surface-1/70 hover:border-accent/40"
                }`}
              >
                <InstanceIcon icon={i.icon} size={26} />
                <span className="max-w-40 truncate">{i.name}</span>
                <span className="text-xs text-muted">{i.mcVersion}</span>
              </button>
            ))}
          </div>
        </section>
      )}

      <NewsFeed />
    </div>
  );
}
