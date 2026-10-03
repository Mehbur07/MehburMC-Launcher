import { FileArchive, FolderOpen, Globe, RefreshCw, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button, ConfirmDialog, EmptyState, IconButton, Toggle } from "../../components/ui";
import { openFolder } from "../../lib/actions";
import { formatBytes, formatDate } from "../../lib/format";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { FileEntry } from "../../lib/ipc/bindings/FileEntry";
import type { Folder } from "../../lib/ipc/bindings/Folder";
import type { Instance } from "../../lib/ipc/bindings/Instance";
import { useApp } from "../../stores/app";

const notify = (e: unknown) => useApp.setState({ notice: toErrorPayload(e) });

/** Lists one content folder; mods/packs can be enabled/disabled. */
export function FilesTab({
  inst,
  folder,
  toggleable = false,
}: {
  inst: Instance;
  folder: Folder;
  toggleable?: boolean;
}) {
  const { t, i18n } = useTranslation();
  const [entries, setEntries] = useState<FileEntry[] | null>(null);
  const [pendingDelete, setPendingDelete] = useState<FileEntry | null>(null);

  const refresh = useCallback(() => {
    ipc
      .listInstanceFiles(inst.id, folder)
      .then(setEntries)
      .catch((e) => {
        setEntries([]);
        notify(e);
      });
  }, [inst.id, folder]);
  useEffect(refresh, [refresh]);

  const toggle = async (e: FileEntry) => {
    try {
      const updated = await ipc.toggleInstanceFile(inst.id, folder, e.name);
      setEntries((list) => list?.map((x) => (x.name === e.name ? updated : x)) ?? null);
    } catch (err) {
      notify(err);
    }
  };

  const remove = async (e: FileEntry) => {
    try {
      await ipc.deleteInstanceFile(inst.id, folder, e.name);
      refresh();
    } catch (err) {
      notify(err);
    }
  };

  const displayName = (name: string) => name.replace(/\.disabled$/, "");

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <span className="text-sm text-muted">
          {t("files.count", { count: entries?.length ?? 0 })}
        </span>
        <div className="flex gap-2">
          <Button size="sm" variant="ghost" onClick={refresh}>
            <RefreshCw size={14} />
            {t("common.refresh")}
          </Button>
          <Button size="sm" onClick={() => void openFolder(inst.id, folder)}>
            <FolderOpen size={14} />
            {t("instances.openFolder")}
          </Button>
        </div>
      </div>

      {entries && entries.length === 0 && (
        <EmptyState
          icon={folder === "saves" ? <Globe size={30} /> : <FileArchive size={30} />}
          title={t(`files.empty.${folder}`)}
        >
          {folder === "mods" && <p>{t("files.modsHint")}</p>}
        </EmptyState>
      )}

      {entries && entries.length > 0 && (
        <ul className="divide-y divide-line overflow-hidden rounded-md border border-line bg-surface-1/80">
          {entries.map((e) => (
            <li key={e.name} className="flex items-center gap-3 px-4 py-2.5">
              <div className="min-w-0 flex-1">
                <div
                  className={`truncate text-sm font-medium ${e.enabled ? "" : "text-muted line-through"}`}
                  title={e.name}
                >
                  {displayName(e.name)}
                </div>
                <div className="text-xs text-muted">
                  {formatBytes(e.size)} · {formatDate(e.modified, i18n.language)}
                </div>
              </div>
              {toggleable && !e.isDir && (
                <Toggle
                  checked={e.enabled}
                  onChange={() => void toggle(e)}
                  label={t("files.enabled")}
                />
              )}
              <IconButton
                label={t("common.delete")}
                onClick={() => setPendingDelete(e)}
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
        message={t(folder === "saves" ? "files.deleteWorld" : "files.deleteBody", {
          name: pendingDelete ? displayName(pendingDelete.name) : "",
        })}
        confirmLabel={t("common.delete")}
        danger
        onConfirm={() => pendingDelete && void remove(pendingDelete)}
        onClose={() => setPendingDelete(null)}
      />
    </div>
  );
}
