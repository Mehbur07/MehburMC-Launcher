import {
  FolderOpen,
  Loader2,
  Lock,
  PackageOpen,
  Plus,
  RefreshCw,
  Search,
  ToggleLeft,
  ToggleRight,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { ProjectIcon } from "../../components/ProjectIcon";
import { Badge, Button, EmptyState, IconButton, TextInput, Toggle } from "../../components/ui";
import { openFolder } from "../../lib/actions";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { InstalledItem } from "../../lib/ipc/bindings/InstalledItem";
import { useApp } from "../../stores/app";
import { selectedInstance, useInstances } from "../../stores/instances";
import { activeTaskFor, useTasks } from "../../stores/tasks";

type Filter = "all" | "on" | "off";
const FILTERS: Filter[] = ["all", "on", "off"];

const notify = (e: unknown) => useApp.setState({ notice: toErrorPayload(e) });
const baseName = (n: string) => n.replace(/\.disabled$/, "");
const modLabel = (i: InstalledItem) => i.title ?? baseName(i.fileName).replace(/\.jar$/i, "");

/** Every mod of a profile with an on/off switch. */
export function ModTogglePage() {
  const { t } = useTranslation();
  const instances = useInstances((s) => s.instances);
  const inst = useInstances(selectedInstance);
  const select = useInstances((s) => s.select);
  const openBrowse = useApp((s) => s.openBrowse);
  const locked = useTasks((s) => activeTaskFor(s.tasks, inst?.id ?? null) !== null);
  const [items, setItems] = useState<InstalledItem[] | null>(null);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [busy, setBusy] = useState<Set<string>>(new Set());
  const [bulk, setBulk] = useState(false);

  const instId = inst?.id ?? null;
  const load = useCallback(async () => {
    if (!instId) return;
    try {
      setItems(await ipc.scanContent(instId, "mods", false));
    } catch (e) {
      setItems((prev) => prev ?? []);
      notify(e);
    }
  }, [instId]);

  useEffect(() => {
    setItems(null);
    void load();
  }, [load]);

  const shown = useMemo(() => {
    const q = query.trim().toLocaleLowerCase();
    return (items ?? [])
      .filter((i) => filter === "all" || i.enabled === (filter === "on"))
      .filter(
        (i) =>
          !q ||
          modLabel(i).toLocaleLowerCase().includes(q) ||
          i.fileName.toLocaleLowerCase().includes(q),
      )
      .sort((a, b) => modLabel(a).localeCompare(modLabel(b)));
  }, [items, query, filter]);

  const enabledCount = items?.filter((i) => i.enabled).length ?? 0;
  const filtered = query.trim() !== "" || filter !== "all";

  const toggle = async (item: InstalledItem) => {
    if (!instId) return;
    setBusy((s) => new Set(s).add(item.fileName));
    try {
      await ipc.toggleInstanceFile(instId, "mods", item.fileName);
    } catch (e) {
      notify(e);
    } finally {
      setBusy((s) => {
        const next = new Set(s);
        next.delete(item.fileName);
        return next;
      });
      await load();
    }
  };

  const setAll = async (enabled: boolean) => {
    if (!instId) return;
    setBulk(true);
    try {
      await ipc.setContentEnabled(
        instId,
        "mods",
        shown.map((i) => i.fileName),
        enabled,
      );
    } catch (e) {
      notify(e);
    } finally {
      setBulk(false);
      await load();
    }
  };

  const noLoader = inst?.loader.kind === "vanilla" || inst?.loader.kind === "optifine";

  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h1 className="font-display text-3xl font-bold tracking-wide">{t("modToggle.title")}</h1>
          <p className="mt-1 text-sm text-muted">{t("modToggle.subtitle")}</p>
        </div>
        {instances.length > 0 && (
          <label className="flex items-center gap-2 text-sm">
            <span className="text-muted">{t("modToggle.profile")}</span>
            <select
              value={inst?.id ?? ""}
              onChange={(e) => void select(e.target.value)}
              className="rounded-md border border-line bg-surface-2 px-2 py-1.5 text-sm focus:border-accent focus:outline-none"
            >
              {instances.map((i) => (
                <option key={i.id} value={i.id}>
                  {i.name} ({i.mcVersion})
                </option>
              ))}
            </select>
          </label>
        )}
      </div>

      {!inst ? (
        <EmptyState icon={<PackageOpen size={30} />} title={t("modToggle.noInstances")} />
      ) : (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <div className="relative min-w-56 flex-1">
              <Search
                size={15}
                className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-muted"
              />
              <TextInput
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder={t("modToggle.search")}
                aria-label={t("modToggle.search")}
                className="pl-9"
              />
            </div>
            <div role="tablist" aria-label={t("modToggle.filter")} className="flex gap-1">
              {FILTERS.map((f) => (
                <Button
                  key={f}
                  role="tab"
                  size="sm"
                  aria-selected={filter === f}
                  variant={filter === f ? "secondary" : "ghost"}
                  onClick={() => setFilter(f)}
                >
                  {t(`modToggle.filters.${f}`)}
                </Button>
              ))}
            </div>
            <Button
              size="sm"
              disabled={locked || bulk || shown.length === 0}
              onClick={() => void setAll(true)}
            >
              <ToggleRight size={14} />
              {filtered ? t("modToggle.enableShown") : t("modToggle.enableAll")}
            </Button>
            <Button
              size="sm"
              disabled={locked || bulk || shown.length === 0}
              onClick={() => void setAll(false)}
            >
              <ToggleLeft size={14} />
              {filtered ? t("modToggle.disableShown") : t("modToggle.disableAll")}
            </Button>
            <IconButton label={t("common.refresh")} onClick={() => void load()}>
              <RefreshCw size={15} />
            </IconButton>
            <IconButton
              label={t("instances.openFolder")}
              onClick={() => void openFolder(inst.id, "mods")}
            >
              <FolderOpen size={15} />
            </IconButton>
          </div>

          <div className="flex flex-wrap items-center gap-2 text-sm">
            {items && (
              <Badge tone="accent">
                {t("modToggle.count", { on: enabledCount, total: items.length })}
              </Badge>
            )}
            {locked && (
              <Badge tone="warn">
                <Lock size={11} />
                {t("modToggle.locked")}
              </Badge>
            )}
            {noLoader && <Badge tone="warn">{t("modToggle.noLoader")}</Badge>}
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto pr-1 pb-4">
            {items === null ? (
              <div className="flex justify-center py-10">
                <Loader2 className="animate-spin text-accent" aria-label={t("common.loading")} />
              </div>
            ) : items.length === 0 ? (
              <EmptyState icon={<PackageOpen size={30} />} title={t("modToggle.empty")}>
                <Button size="sm" variant="primary" onClick={() => openBrowse(inst.id, "mod")}>
                  <Plus size={14} />
                  {t("content.add")}
                </Button>
              </EmptyState>
            ) : shown.length === 0 ? (
              <EmptyState icon={<Search size={28} />} title={t("modToggle.noMatch")} />
            ) : (
              <ul className="divide-y divide-line overflow-hidden rounded-lg border border-line bg-surface-1/85 backdrop-blur">
                {shown.map((i) => (
                  <li
                    key={i.fileName}
                    className={`flex items-center gap-3 px-3 py-2 transition-opacity ${
                      i.enabled ? "" : "opacity-60"
                    }`}
                  >
                    <ProjectIcon url={i.iconUrl} size={36} />
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-sm font-semibold" title={modLabel(i)}>
                        {modLabel(i)}
                      </div>
                      <div className="truncate text-xs text-muted" title={i.fileName}>
                        {i.versionNumber ? `${i.versionNumber} · ` : ""}
                        {baseName(i.fileName)}
                      </div>
                    </div>
                    {busy.has(i.fileName) && (
                      <Loader2 size={14} className="animate-spin text-accent" />
                    )}
                    <span
                      className={`w-12 text-right text-xs ${i.enabled ? "text-success" : "text-muted"}`}
                    >
                      {i.enabled ? t("modToggle.on") : t("modToggle.off")}
                    </span>
                    <span title={locked ? t("modToggle.locked") : undefined}>
                      <Toggle
                        checked={i.enabled}
                        disabled={locked || bulk || busy.has(i.fileName)}
                        onChange={() => void toggle(i)}
                        label={t("modToggle.switch", { name: modLabel(i) })}
                      />
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </>
      )}
    </div>
  );
}
