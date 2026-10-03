import {
  ArrowUpCircle,
  FileArchive,
  FolderOpen,
  Loader2,
  Plus,
  RefreshCw,
  Trash2,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { ProjectIcon } from "../../components/ProjectIcon";
import { Badge, Button, ConfirmDialog, EmptyState, IconButton, Toggle } from "../../components/ui";
import { openFolder } from "../../lib/actions";
import { formatBytes } from "../../lib/format";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { Folder } from "../../lib/ipc/bindings/Folder";
import type { InstalledItem } from "../../lib/ipc/bindings/InstalledItem";
import type { Instance } from "../../lib/ipc/bindings/Instance";
import type { ProjectType } from "../../lib/ipc/bindings/ProjectType";
import { useApp } from "../../stores/app";
import { activeTaskFor, useTasks } from "../../stores/tasks";

const notify = (e: unknown) => useApp.setState({ notice: toErrorPayload(e) });

const TYPE: Partial<Record<Folder, ProjectType>> = {
  mods: "mod",
  resourcePacks: "resourcepack",
  shaderPacks: "shader",
};

/** Mods / resource packs / shader packs with Modrinth metadata and updates. */
export function ContentTab({ inst, folder }: { inst: Instance; folder: Folder }) {
  const { t } = useTranslation();
  const openBrowse = useApp((s) => s.openBrowse);
  const busyInstance = useTasks((s) => !!activeTaskFor(s.tasks, inst.id));
  const [items, setItems] = useState<InstalledItem[] | null>(null);
  const [checking, setChecking] = useState(false);
  const [checked, setChecked] = useState(false);
  const [updating, setUpdating] = useState<Set<string>>(new Set());
  const [pendingDelete, setPendingDelete] = useState<InstalledItem | null>(null);

  const load = useCallback(
    async (checkUpdates: boolean) => {
      if (checkUpdates) setChecking(true);
      try {
        setItems(await ipc.scanContent(inst.id, folder, checkUpdates));
        if (checkUpdates) setChecked(true);
      } catch (e) {
        setItems((prev) => prev ?? []);
        notify(e);
      } finally {
        setChecking(false);
      }
    },
    [inst.id, folder],
  );

  useEffect(() => {
    setItems(null);
    setChecked(false);
    void load(false);
  }, [load]);

  const toggle = async (item: InstalledItem) => {
    try {
      await ipc.toggleInstanceFile(inst.id, folder, item.fileName);
      await load(checked);
    } catch (e) {
      notify(e);
    }
  };

  const remove = async (item: InstalledItem) => {
    try {
      await ipc.deleteInstanceFile(inst.id, folder, item.fileName);
      await load(checked);
    } catch (e) {
      notify(e);
    }
  };

  const update = async (files: string[]) => {
    setUpdating(new Set(files));
    try {
      await ipc.updateContent(inst.id, folder, files);
      await load(true);
    } catch (e) {
      notify(e);
    } finally {
      setUpdating(new Set());
    }
  };

  const outdated = items?.filter((i) => i.update) ?? [];
  const displayName = (n: string) => n.replace(/\.disabled$/, "");

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <span className="text-sm text-muted">
          {t("files.count", { count: items?.length ?? 0 })}
        </span>
        <div className="flex flex-wrap gap-2">
          <Button size="sm" variant="primary" onClick={() => openBrowse(inst.id, TYPE[folder])}>
            <Plus size={14} />
            {t("content.add")}
          </Button>
          <Button size="sm" disabled={checking || !items?.length} onClick={() => void load(true)}>
            {checking ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}
            {t("content.checkUpdates")}
          </Button>
          {outdated.length > 0 && (
            <Button
              size="sm"
              variant="primary"
              disabled={busyInstance || updating.size > 0}
              onClick={() => void update(outdated.map((i) => i.fileName))}
            >
              <ArrowUpCircle size={14} />
              {t("content.updateAll", { count: outdated.length })}
            </Button>
          )}
          <Button size="sm" onClick={() => void openFolder(inst.id, folder)}>
            <FolderOpen size={14} />
            {t("instances.openFolder")}
          </Button>
        </div>
      </div>

      {checked && outdated.length === 0 && items && items.length > 0 && (
        <p className="text-xs text-success">{t("content.upToDate")}</p>
      )}

      {items === null && (
        <div className="flex justify-center py-8">
          <Loader2 className="animate-spin text-accent" aria-label={t("common.loading")} />
        </div>
      )}

      {items?.length === 0 && (
        <EmptyState icon={<FileArchive size={30} />} title={t(`files.empty.${folder}`)}>
          <p>{t("content.emptyHint")}</p>
        </EmptyState>
      )}

      {items && items.length > 0 && (
        <ul className="divide-y divide-line overflow-hidden rounded-md border border-line bg-surface-1/80">
          {items.map((i) => (
            <li key={i.fileName} className="flex items-center gap-3 px-3 py-2">
              <ProjectIcon url={i.iconUrl} size={36} />
              <div className="min-w-0 flex-1">
                <div
                  className={`flex items-center gap-2 truncate text-sm font-medium ${i.enabled ? "" : "text-muted line-through"}`}
                  title={i.fileName}
                >
                  {i.title ?? displayName(i.fileName)}
                  {i.update && (
                    <Badge tone="accent">
                      {t("content.updateTo", { version: i.update.versionNumber })}
                    </Badge>
                  )}
                </div>
                <div className="truncate text-xs text-muted">
                  {i.versionNumber ? `${i.versionNumber} · ` : ""}
                  {i.title ? `${displayName(i.fileName)} · ` : ""}
                  {formatBytes(i.size)}
                </div>
              </div>
              {i.update && (
                <IconButton
                  label={t("content.update")}
                  disabled={busyInstance || updating.size > 0}
                  onClick={() => void update([i.fileName])}
                >
                  {updating.has(i.fileName) ? (
                    <Loader2 size={15} className="animate-spin" />
                  ) : (
                    <ArrowUpCircle size={15} />
                  )}
                </IconButton>
              )}
              <Toggle
                checked={i.enabled}
                onChange={() => void toggle(i)}
                label={t("files.enabled")}
              />
              <IconButton
                label={t("common.delete")}
                onClick={() => setPendingDelete(i)}
                className="hover:text-danger"
              >
                <Trash2 size={15} />
              </IconButton>
            </li>
          ))}
        </ul>
      )}

      <ConfirmDialog
        open={pendingDelete !== null}
        title={t("files.deleteTitle")}
        message={t("files.deleteBody", {
          name: pendingDelete ? (pendingDelete.title ?? displayName(pendingDelete.fileName)) : "",
        })}
        confirmLabel={t("common.delete")}
        danger
        onConfirm={() => pendingDelete && void remove(pendingDelete)}
        onClose={() => setPendingDelete(null)}
      />
    </div>
  );
}
