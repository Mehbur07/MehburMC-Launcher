import { convertFileSrc } from "@tauri-apps/api/core";
import { FolderOpen, Image as ImageIcon, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button, ConfirmDialog, EmptyState, IconButton, Modal } from "../../components/ui";
import { openFolder } from "../../lib/actions";
import { formatDate } from "../../lib/format";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { FileEntry } from "../../lib/ipc/bindings/FileEntry";
import type { Instance } from "../../lib/ipc/bindings/Instance";
import { useApp } from "../../stores/app";

export function ScreenshotsTab({ inst }: { inst: Instance }) {
  const { t, i18n } = useTranslation();
  const [dir, setDir] = useState<string | null>(null);
  const [shots, setShots] = useState<FileEntry[] | null>(null);
  const [viewing, setViewing] = useState<FileEntry | null>(null);
  const [pendingDelete, setPendingDelete] = useState<FileEntry | null>(null);

  const refresh = useCallback(() => {
    // Listing first also authorises the asset protocol for this folder.
    ipc
      .listInstanceFiles(inst.id, "screenshots")
      .then(async (list) => {
        setDir(await ipc.instanceFolderPath(inst.id, "screenshots"));
        setShots(list.filter((f) => /\.(png|jpe?g)$/i.test(f.name)));
      })
      .catch((e) => {
        setShots([]);
        useApp.setState({ notice: toErrorPayload(e) });
      });
  }, [inst.id]);
  useEffect(refresh, [refresh]);

  const src = (f: FileEntry) => (dir ? convertFileSrc(`${dir}\\${f.name}`) : "");

  return (
    <div className="flex flex-col gap-3">
      <div className="flex justify-end">
        <Button size="sm" onClick={() => void openFolder(inst.id, "screenshots")}>
          <FolderOpen size={14} />
          {t("instances.openFolder")}
        </Button>
      </div>
      {shots?.length === 0 && (
        <EmptyState icon={<ImageIcon size={30} />} title={t("files.empty.screenshots")}>
          <p>{t("files.screenshotsHint")}</p>
        </EmptyState>
      )}
      <div className="grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-3">
        {shots?.map((f) => (
          <figure
            key={f.name}
            className="group relative overflow-hidden rounded-md border border-line bg-surface-1"
          >
            <button type="button" onClick={() => setViewing(f)} className="block w-full">
              <img
                src={src(f)}
                alt={f.name}
                loading="lazy"
                className="aspect-video w-full object-cover"
              />
            </button>
            <figcaption className="flex items-center justify-between px-2 py-1.5 text-xs text-muted">
              <span className="truncate">{formatDate(f.modified, i18n.language)}</span>
              <IconButton
                label={t("common.delete")}
                onClick={() => setPendingDelete(f)}
                className="h-6 w-6 hover:text-danger"
              >
                <Trash2 size={13} />
              </IconButton>
            </figcaption>
          </figure>
        ))}
      </div>

      <Modal
        open={viewing !== null}
        onClose={() => setViewing(null)}
        title={viewing?.name ?? ""}
        wide
      >
        {viewing && <img src={src(viewing)} alt={viewing.name} className="w-full rounded-md" />}
      </Modal>
      <ConfirmDialog
        open={pendingDelete !== null}
        title={t("files.deleteTitle")}
        message={t("files.deleteBody", { name: pendingDelete?.name ?? "" })}
        confirmLabel={t("common.delete")}
        danger
        onConfirm={() =>
          pendingDelete &&
          void ipc
            .deleteInstanceFile(inst.id, "screenshots", pendingDelete.name)
            .then(refresh)
            .catch((e) => useApp.setState({ notice: toErrorPayload(e) }))
        }
        onClose={() => setPendingDelete(null)}
      />
    </div>
  );
}
