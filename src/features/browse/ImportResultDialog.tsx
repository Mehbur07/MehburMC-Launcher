import { ExternalLink, TriangleAlert } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button, Modal } from "../../components/ui";
import { ipc } from "../../lib/ipc";
import type { ImportResult } from "../../lib/ipc/bindings/ImportResult";
import { useApp } from "../../stores/app";

/** Shown after a modpack import; lists files that need a manual download. */
export function ImportResultDialog({
  result,
  onClose,
}: {
  result: ImportResult | null;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const openInstance = useApp((s) => s.openInstance);
  if (!result) return null;
  const { instance, blocked } = result;

  return (
    <Modal
      open
      onClose={onClose}
      title={t("modpack.importedTitle")}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t("common.close")}
          </Button>
          <Button
            variant="primary"
            onClick={() => {
              onClose();
              openInstance(instance.id);
            }}
          >
            {t("modpack.openInstance")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-3 text-sm">
        <p>{t("modpack.imported", { name: instance.name, mc: instance.mcVersion })}</p>
        {blocked.length > 0 && (
          <div className="rounded-md border border-warn/40 bg-warn/10 p-3">
            <p className="flex items-center gap-2 font-semibold text-warn">
              <TriangleAlert size={15} />
              {t("modpack.blockedTitle", { count: blocked.length })}
            </p>
            <p className="mt-1 text-xs text-muted">{t("modpack.blockedBody")}</p>
            <ul className="mt-2 flex max-h-56 flex-col gap-1 overflow-y-auto">
              {blocked.map((b) => (
                <li key={b.name} className="flex items-center justify-between gap-2">
                  <span data-selectable className="truncate font-mono text-xs" title={b.name}>
                    {b.folder}/{b.name}
                  </span>
                  <Button size="sm" onClick={() => void ipc.openExternal(b.url)}>
                    <ExternalLink size={12} />
                    {t("modpack.download")}
                  </Button>
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
    </Modal>
  );
}
