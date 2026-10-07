import { Check, Download, Loader2, RefreshCw, Search } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { ProjectIcon } from "../../components/ProjectIcon";
import { Badge, Button, ConfirmDialog, EmptyState, TextInput, Toggle } from "../../components/ui";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { ErrorPayload } from "../../lib/ipc/bindings/ErrorPayload";
import type { ImportResult } from "../../lib/ipc/bindings/ImportResult";
import type { Instance } from "../../lib/ipc/bindings/Instance";
import type { InstalledItem } from "../../lib/ipc/bindings/InstalledItem";
import type { ProjectType } from "../../lib/ipc/bindings/ProjectType";
import type { SearchHit } from "../../lib/ipc/bindings/SearchHit";
import type { SearchSort } from "../../lib/ipc/bindings/SearchSort";
import { useApp } from "../../stores/app";
import { useInstances } from "../../stores/instances";
import { activeTaskFor, useTasks } from "../../stores/tasks";
import { ImportResultDialog } from "./ImportResultDialog";
import { InstalledMenu } from "./InstalledMenu";
import { LibraryHub } from "./LibraryTextures";
import { ProjectDialog } from "./ProjectDialog";
import {
  compactNumber,
  folderFor,
  installedByProject,
  installModpack,
  modrinthLoader,
} from "./content";

const TYPES: ProjectType[] = ["mod", "modpack", "resourcepack", "shader"];
const SORTS: SearchSort[] = ["relevance", "downloads", "updated", "newest"];

type InstallState = "busy" | "done" | ErrorPayload;

export function BrowsePage() {
  const { t } = useTranslation();
  const tab = useApp((s) => s.browseType);
  const isLibrary = tab === "library";
  const type: ProjectType = isLibrary ? "mod" : tab;
  const target = useApp((s) => s.browseTarget);
  const instances = useInstances((s) => s.instances);
  const selected = useInstances((s) => s.selected);
  const loadInstances = useInstances((s) => s.load);

  const targetId = target ?? selected;
  const inst: Instance | null = instances.find((i) => i.id === targetId) ?? null;
  const setTab = (t: ProjectType | "library") => useApp.setState({ browseType: t });
  const setTarget = (id: string | null) => useApp.setState({ browseTarget: id });

  const [query, setQuery] = useState("");
  const [debounced, setDebounced] = useState("");
  const [sort, setSort] = useState<SearchSort>("relevance");
  const [compatOnly, setCompatOnly] = useState(true);
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [reload, setReload] = useState(0);
  const [installs, setInstalls] = useState<Record<string, InstallState>>({});
  const [open, setOpen] = useState<SearchHit | null>(null);
  const [imported, setImported] = useState<ImportResult | null>(null);
  /** Files of the target instance per Modrinth project (current tab's folder). */
  const [installed, setInstalled] = useState<Record<string, InstalledItem[]>>({});
  const [rescan, setRescan] = useState(0);
  const [removing, setRemoving] = useState<{ hit: SearchHit; files: InstalledItem[] } | null>(null);
  const gameBusy = useTasks((s) => activeTaskFor(s.tasks, inst?.id ?? null) !== null);

  useEffect(() => {
    const h = setTimeout(() => setDebounced(query), 350);
    return () => clearTimeout(h);
  }, [query]);

  const isPack = type === "modpack";
  const folder = folderFor(type);
  const instId = inst?.id ?? null;

  // What the target instance already has (identified by SHA-1 on Modrinth).
  useEffect(() => {
    setInstalled({});
    if (!instId || !folder) return;
    let cancelled = false;
    ipc
      .scanContent(instId, folder, false)
      .then((items) => !cancelled && setInstalled(installedByProject(items)))
      .catch(() => {
        // Offline: results simply show no "Installed" state.
      });
    return () => {
      cancelled = true;
    };
  }, [instId, folder, rescan]);
  const loader = inst ? modrinthLoader(inst.loader.kind) : null;
  const filter = useMemo(
    () => ({
      gameVersion: !isPack && compatOnly && inst ? inst.mcVersion : undefined,
      loader: type === "mod" && compatOnly && loader ? loader : undefined,
    }),
    [isPack, compatOnly, inst, type, loader],
  );

  const fetchPage = async (offset: number) => {
    setLoading(true);
    setError(null);
    try {
      const page = await ipc.searchModrinth({
        query: debounced,
        projectType: type,
        sort,
        offset,
        ...filter,
      });
      setHits((prev) => (offset === 0 ? page.hits : [...prev, ...page.hits]));
      setTotal(page.totalHits);
    } catch (e) {
      setError(toErrorPayload(e).code);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (!isLibrary) void fetchPage(0);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [debounced, type, sort, filter, reload, isLibrary]);

  const install = async (hit: SearchHit, versionId?: string) => {
    setInstalls((s) => ({ ...s, [hit.projectId]: "busy" }));
    try {
      if (isPack) {
        const r = await installModpack(hit.projectId, versionId);
        await loadInstances();
        setImported(r);
      } else if (inst) {
        await ipc.installContent(inst.id, [
          { project: hit.projectId, projectType: type, versionId },
        ]);
        setRescan((n) => n + 1);
      }
      setInstalls((s) => ({ ...s, [hit.projectId]: "done" }));
    } catch (e) {
      setInstalls((s) => ({ ...s, [hit.projectId]: toErrorPayload(e) }));
    }
  };

  const remove = async (hit: SearchHit, files: InstalledItem[]) => {
    if (!inst || !folder) return;
    try {
      for (const f of files) await ipc.deleteInstanceFile(inst.id, folder, f.fileName);
      setInstalls((s) => {
        const next = { ...s };
        delete next[hit.projectId];
        return next;
      });
    } catch (e) {
      useApp.setState({ notice: toErrorPayload(e) });
    } finally {
      setRescan((n) => n + 1);
    }
  };

  const modsUnsupported = type === "mod" && inst !== null && !loader;
  const canInstall = isPack || (inst !== null && !modsUnsupported);

  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <h1 className="font-display text-3xl font-bold tracking-wide">{t("nav.browse")}</h1>
        {!isPack && (
          <label className="flex items-center gap-2 text-sm">
            <span className="text-muted">{t("browse.target")}</span>
            <select
              value={inst?.id ?? ""}
              onChange={(e) => setTarget(e.target.value || null)}
              className="rounded-md border border-line bg-surface-2 px-2 py-1.5 text-sm focus:border-accent focus:outline-none"
            >
              {instances.length === 0 && <option value="">{t("browse.noInstances")}</option>}
              {instances.map((i) => (
                <option key={i.id} value={i.id}>
                  {i.name} ({i.mcVersion})
                </option>
              ))}
            </select>
          </label>
        )}
      </div>

      <div role="tablist" className="flex gap-1 border-b border-line">
        {[...TYPES, "library" as const].map((k) => (
          <button
            key={k}
            type="button"
            role="tab"
            aria-selected={tab === k}
            onClick={() => setTab(k)}
            className={`-mb-px border-b-2 px-3 py-2 text-sm font-medium transition-colors ${
              tab === k
                ? "border-accent text-accent"
                : "border-transparent text-muted hover:text-fg"
            }`}
          >
            {k === "library" ? t("library.tab") : t(`browse.types.${k}`)}
          </button>
        ))}
      </div>

      {isLibrary ? (
        <LibraryHub inst={inst} gameBusy={gameBusy} />
      ) : (
        <>
          <div className="flex flex-wrap items-center gap-3">
            <div className="relative min-w-60 flex-1">
              <Search
                size={15}
                className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-muted"
              />
              <TextInput
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder={t("browse.search")}
                aria-label={t("browse.search")}
                className="pl-9"
              />
            </div>
            <select
              value={sort}
              aria-label={t("browse.sort")}
              onChange={(e) => setSort(e.target.value as SearchSort)}
              className="rounded-md border border-line bg-surface-2 px-2 py-2 text-sm focus:border-accent focus:outline-none"
            >
              {SORTS.map((s) => (
                <option key={s} value={s}>
                  {t(`browse.sorts.${s}`)}
                </option>
              ))}
            </select>
            {!isPack && inst && (
              <label className="flex items-center gap-2 text-sm text-muted">
                <Toggle
                  checked={compatOnly}
                  onChange={setCompatOnly}
                  label={t("browse.compatible")}
                />
                {t("browse.compatible")}
              </label>
            )}
          </div>

          {modsUnsupported && (
            <p className="rounded-md border border-warn/40 bg-warn/10 px-3 py-2 text-sm text-warn">
              {t("browse.needsLoader")}
            </p>
          )}

          <div className="min-h-0 flex-1 overflow-y-auto pr-1 pb-4">
            {error && (
              <EmptyState
                icon={<RefreshCw size={28} />}
                title={t(`errors.${error}`, { defaultValue: t("errors.unknown") })}
              >
                <Button size="sm" onClick={() => setReload((n) => n + 1)}>
                  {t("common.retry")}
                </Button>
              </EmptyState>
            )}
            {!error && !loading && hits.length === 0 && (
              <EmptyState icon={<Search size={28} />} title={t("browse.empty")} />
            )}
            <ul className="flex flex-col gap-2">
              {hits.map((h) => {
                const st = installs[h.projectId];
                const files = isPack ? undefined : installed[h.projectId];
                return (
                  <li
                    key={h.projectId}
                    className="flex items-center gap-3 rounded-lg border border-line bg-surface-1/85 px-3 py-2.5 transition-colors hover:border-accent/40"
                  >
                    <button
                      type="button"
                      onClick={() => setOpen(h)}
                      className="flex min-w-0 flex-1 items-center gap-3 text-left"
                    >
                      <ProjectIcon url={h.iconUrl} />
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                          <span className="truncate font-semibold">{h.title}</span>
                          <span className="truncate text-xs text-muted">{h.author}</span>
                        </div>
                        <p className="truncate text-xs text-muted">{h.description}</p>
                        <div className="mt-1 flex gap-1.5">
                          <Badge>
                            <Download size={10} />
                            {compactNumber(h.downloads)}
                          </Badge>
                        </div>
                      </div>
                    </button>
                    <div className="flex shrink-0 flex-col items-end gap-1">
                      {files && st !== "busy" ? (
                        <InstalledMenu
                          disabled={files.some((f) => !f.enabled)}
                          locked={gameBusy}
                          onDelete={() => setRemoving({ hit: h, files })}
                        />
                      ) : (
                        <Button
                          size="sm"
                          variant={st === "done" ? "ghost" : "primary"}
                          disabled={!canInstall || st === "busy" || st === "done"}
                          onClick={() => void install(h)}
                        >
                          {st === "busy" ? (
                            <Loader2 size={13} className="animate-spin" />
                          ) : st === "done" ? (
                            <Check size={13} />
                          ) : (
                            <Download size={13} />
                          )}
                          {st === "done"
                            ? t("browse.installed")
                            : isPack
                              ? t("browse.installPack")
                              : t("browse.install")}
                        </Button>
                      )}
                      {typeof st === "object" && (
                        <span
                          className="max-w-56 truncate text-[11px] text-danger"
                          title={st.detail}
                        >
                          {t(`errors.${st.code}`, { ...st.params, defaultValue: st.detail })}
                        </span>
                      )}
                    </div>
                  </li>
                );
              })}
            </ul>
            {loading && (
              <div className="flex justify-center py-4">
                <Loader2 className="animate-spin text-accent" aria-label={t("common.loading")} />
              </div>
            )}
            {!loading && hits.length < total && (
              <div className="flex justify-center py-3">
                <Button size="sm" onClick={() => void fetchPage(hits.length)}>
                  {t("browse.more", { shown: hits.length, total })}
                </Button>
              </div>
            )}
          </div>
        </>
      )}

      <ProjectDialog
        hit={open}
        type={type}
        instance={isPack ? null : inst}
        canInstall={canInstall}
        onClose={() => setOpen(null)}
        onInstall={(h, versionId) => {
          setOpen(null);
          void install(h, versionId);
        }}
      />
      <ImportResultDialog result={imported} onClose={() => setImported(null)} />
      <ConfirmDialog
        open={removing !== null}
        danger
        title={t("browse.removeTitle")}
        message={
          removing && (
            <>
              <p>
                {t("browse.removeMessage", {
                  name: removing.hit.title,
                  instance: inst?.name ?? "",
                })}
              </p>
              <ul className="mt-2 font-mono text-xs">
                {removing.files.map((f) => (
                  <li key={f.fileName}>{f.fileName}</li>
                ))}
              </ul>
            </>
          )
        }
        confirmLabel={t("browse.remove")}
        onClose={() => setRemoving(null)}
        onConfirm={() => removing && void remove(removing.hit, removing.files)}
      />
    </div>
  );
}
