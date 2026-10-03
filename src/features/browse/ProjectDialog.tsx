import { Download, ExternalLink, Loader2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { ProjectIcon } from "../../components/ProjectIcon";
import { Badge, Button, Modal } from "../../components/ui";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { Instance } from "../../lib/ipc/bindings/Instance";
import type { ProjectType } from "../../lib/ipc/bindings/ProjectType";
import type { SearchHit } from "../../lib/ipc/bindings/SearchHit";
import type { VersionSummary } from "../../lib/ipc/bindings/VersionSummary";

const MAX_SHOWN = 60;

/** Project details with its versions (compatible ones when an instance is set). */
export function ProjectDialog({
  hit,
  type,
  instance,
  canInstall,
  onClose,
  onInstall,
}: {
  hit: SearchHit | null;
  type: ProjectType;
  instance: Instance | null;
  canInstall: boolean;
  onClose: () => void;
  onInstall: (hit: SearchHit, versionId: string) => void;
}) {
  const { t, i18n } = useTranslation();
  const [versions, setVersions] = useState<VersionSummary[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!hit) return;
    let alive = true;
    setVersions(null);
    setError(null);
    ipc
      .projectVersions(hit.projectId, type, instance?.id)
      .then((v) => alive && setVersions(v))
      .catch((e) => alive && setError(toErrorPayload(e).code));
    return () => {
      alive = false;
    };
  }, [hit, type, instance?.id]);

  if (!hit) return null;
  const page = `https://modrinth.com/${type}/${hit.slug}`;

  return (
    <Modal
      open
      onClose={onClose}
      wide
      title={hit.title}
      footer={
        <>
          <Button variant="ghost" onClick={() => void ipc.openExternal(page)}>
            <ExternalLink size={14} />
            {t("browse.openModrinth")}
          </Button>
          <Button onClick={onClose}>{t("common.close")}</Button>
        </>
      }
    >
      <div className="flex flex-col gap-4" style={{ maxHeight: "min(60vh, 520px)" }}>
        <div className="flex items-start gap-4">
          <ProjectIcon url={hit.iconUrl} size={64} />
          <div className="min-w-0">
            <p className="text-sm">{hit.description}</p>
            <div className="mt-2 flex flex-wrap gap-1.5">
              {hit.categories.slice(0, 6).map((c) => (
                <Badge key={c}>{c}</Badge>
              ))}
            </div>
          </div>
        </div>

        <div className="flex items-center justify-between">
          <span className="text-xs text-muted">
            <span className="font-semibold tracking-wide uppercase">{t("browse.versions")}</span>
            {instance && ` · ${t("browse.compatibleWith", { name: instance.name })}`}
          </span>
          {versions && <span className="text-xs text-muted">{versions.length}</span>}
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto rounded-md border border-line">
          {!versions && !error && (
            <div className="flex justify-center p-6">
              <Loader2 className="animate-spin text-accent" aria-label={t("common.loading")} />
            </div>
          )}
          {error && (
            <p className="p-4 text-sm text-danger">
              {t(`errors.${error}`, { defaultValue: t("errors.unknown") })}
            </p>
          )}
          {versions?.length === 0 && (
            <p className="p-4 text-sm text-muted">{t("browse.noCompatible")}</p>
          )}
          <ul className="divide-y divide-line">
            {versions?.slice(0, MAX_SHOWN).map((v) => (
              <li key={v.id} className="flex items-center gap-3 px-3 py-2">
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="truncate text-sm font-medium">{v.versionNumber}</span>
                    {v.versionType !== "release" && <Badge tone="warn">{v.versionType}</Badge>}
                  </div>
                  <div className="truncate text-xs text-muted">
                    {v.gameVersions.slice(-3).join(", ")}
                    {v.loaders.length > 0 && ` · ${v.loaders.join(", ")}`}
                    {v.datePublished &&
                      ` · ${new Date(v.datePublished).toLocaleDateString(i18n.language)}`}
                  </div>
                </div>
                <Button size="sm" disabled={!canInstall} onClick={() => onInstall(hit, v.id)}>
                  <Download size={13} />
                  {type === "modpack" ? t("browse.installPack") : t("browse.install")}
                </Button>
              </li>
            ))}
          </ul>
        </div>
      </div>
    </Modal>
  );
}
