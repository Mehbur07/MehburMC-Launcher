import {
  Clock,
  FolderOpen,
  Play,
  ShieldCheck,
  Sparkles,
  Square,
  SquareTerminal,
  Timer,
} from "lucide-react";
import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "../../components/ui";
import { openFolder, play, repair, stop } from "../../lib/actions";
import { formatDate, formatDuration } from "../../lib/format";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { Instance } from "../../lib/ipc/bindings/Instance";
import { useApp } from "../../stores/app";
import { activeTaskFor, useTasks } from "../../stores/tasks";
import { loaderLabel, usePlayTimeUnits } from "../instances/InstancesPage";
import { SHADER_LOADERS } from "../instances/LoaderPicker";
import { forgeLabel } from "../instances/loaderLabels";

export function GeneralTab({ inst }: { inst: Instance }) {
  const { t, i18n } = useTranslation();
  const units = usePlayTimeUnits();
  const task = useTasks((s) => activeTaskFor(s.tasks, inst.id));
  const openConsole = useApp((s) => s.openConsole);
  const defaultMem = useApp((s) => s.settings?.defaultMemoryMb ?? 4096);
  const [shaderBusy, setShaderBusy] = useState(false);
  const [shaderResult, setShaderResult] = useState<string | null>(null);

  const installShaders = async () => {
    setShaderBusy(true);
    setShaderResult(null);
    try {
      const r = await ipc.installShaderSupport(inst.id);
      setShaderResult(
        r.installed.length
          ? t("shaders.installed", { files: r.installed.join(", ") })
          : t("shaders.nothing", { mods: r.alreadyPresent.join(", ") }),
      );
    } catch (e) {
      useApp.setState({ notice: toErrorPayload(e) });
    } finally {
      setShaderBusy(false);
    }
  };

  const loaderText =
    loaderLabel(inst.loader.kind) +
    (inst.loader.version ? ` ${forgeLabel(inst.mcVersion, inst.loader.version)}` : "");

  return (
    <div className="flex flex-col gap-5">
      <div className="flex flex-wrap gap-2">
        {task ? (
          <Button variant="danger" onClick={() => void stop(inst.id)}>
            <Square size={15} />
            {t("home.stop")}
          </Button>
        ) : (
          <Button variant="primary" onClick={() => void play(inst.id)}>
            <Play size={15} fill="currentColor" />
            {t("home.play")}
          </Button>
        )}
        <Button onClick={() => void openFolder(inst.id)}>
          <FolderOpen size={15} />
          {t("instances.openFolder")}
        </Button>
        <Button onClick={() => openConsole(inst.id)}>
          <SquareTerminal size={15} />
          {t("nav.console")}
        </Button>
        <Button
          disabled={!!task}
          onClick={() => void repair(inst.id)}
          title={t("instance.repairHint")}
        >
          <ShieldCheck size={15} />
          {t("instance.repair")}
        </Button>
      </div>

      {SHADER_LOADERS.includes(inst.loader.kind) && (
        <div className="flex flex-wrap items-center gap-3 rounded-md border border-line bg-surface-1/80 px-4 py-3">
          <div className="min-w-0 flex-1">
            <div className="text-sm font-semibold">{t("shaders.title")}</div>
            <div className="text-xs text-muted">
              {inst.loader.kind === "forge" ? t("shaders.bodyForge") : t("shaders.body")}
            </div>
            {shaderResult && <div className="mt-1 text-xs text-success">{shaderResult}</div>}
          </div>
          <Button disabled={!!task || shaderBusy} onClick={() => void installShaders()}>
            <Sparkles size={15} />
            {shaderBusy ? t("shaders.installing") : t("shaders.install")}
          </Button>
        </div>
      )}

      <dl className="grid grid-cols-2 gap-3 md:grid-cols-3">
        <Stat label={t("instance.version")} value={inst.mcVersion} />
        <Stat label={t("instance.loader")} value={loaderText} />
        <Stat
          label={t("options.memory")}
          value={`${((inst.memoryMb ?? defaultMem) / 1024).toFixed(1)} GB${inst.memoryMb ? "" : ` (${t("options.default")})`}`}
        />
        <Stat
          icon={<Timer size={14} />}
          label={t("instances.playTime")}
          value={formatDuration(inst.playTimeSecs, units)}
        />
        <Stat
          icon={<Clock size={14} />}
          label={t("instance.lastPlayed")}
          value={inst.lastPlayed ? formatDate(inst.lastPlayed, i18n.language) : t("instance.never")}
        />
        <Stat label={t("instance.created")} value={formatDate(inst.createdAt, i18n.language)} />
        <Stat label={t("options.java")} value={inst.javaPath ?? t("options.javaAuto")} wide />
      </dl>
    </div>
  );
}

function Stat({
  label,
  value,
  icon,
  wide,
}: {
  label: string;
  value: string;
  icon?: ReactNode;
  wide?: boolean;
}) {
  return (
    <div
      className={`rounded-md border border-line bg-surface-1/80 px-4 py-3 ${wide ? "col-span-2 md:col-span-3" : ""}`}
    >
      <dt className="flex items-center gap-1.5 text-xs text-muted">
        {icon}
        {label}
      </dt>
      <dd data-selectable className="mt-1 truncate text-sm font-semibold" title={value}>
        {value}
      </dd>
    </div>
  );
}
