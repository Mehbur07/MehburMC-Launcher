import { CheckCircle2, Download, Gamepad2, Info, Loader2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Badge, Button } from "../../components/ui";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { ErrorPayload } from "../../lib/ipc/bindings/ErrorPayload";
import type { Instance } from "../../lib/ipc/bindings/Instance";
import type { LoaderKind } from "../../lib/ipc/bindings/LoaderKind";
import { useInstances } from "../../stores/instances";
import { LOADERS } from "../instances/LoaderPicker";

/** CustomSkinLoader on Modrinth (see launcher-core skin::csl). */
export const CSL_PROJECT = "idMHQ4n2";
/** Loaders CustomSkinLoader ships builds for. */
const CSL_LOADERS: LoaderKind[] = ["fabric", "quilt", "forge", "neoForge"];

type Status = "checking" | "missing" | "installed" | "busy" | ErrorPayload;

const isCsl = (name: string) => /^customskinloader.*\.jar$/i.test(name);

/** Per-instance CustomSkinLoader status with one-click install. */
export function InGamePanel() {
  const { t } = useTranslation();
  const instances = useInstances((s) => s.instances);
  const modded = instances.filter((i) => CSL_LOADERS.includes(i.loader.kind));
  const vanillaCount = instances.length - modded.length;
  const [status, setStatus] = useState<Record<string, Status>>({});

  useEffect(() => {
    let alive = true;
    for (const inst of modded) {
      void ipc
        .listInstanceFiles(inst.id, "mods")
        .then((files) => {
          if (alive)
            setStatus((s) => ({
              ...s,
              [inst.id]: files.some((f) => f.enabled && isCsl(f.name)) ? "installed" : "missing",
            }));
        })
        .catch(() => alive && setStatus((s) => ({ ...s, [inst.id]: "missing" })));
    }
    return () => {
      alive = false;
    };
    // Re-check when the set of instances changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [modded.map((i) => i.id).join()]);

  const install = async (inst: Instance) => {
    setStatus((s) => ({ ...s, [inst.id]: "busy" }));
    try {
      await ipc.installContent(inst.id, [{ project: CSL_PROJECT, projectType: "mod" }]);
      setStatus((s) => ({ ...s, [inst.id]: "installed" }));
    } catch (e) {
      setStatus((s) => ({ ...s, [inst.id]: toErrorPayload(e) }));
    }
  };

  const loaderLabel = (k: LoaderKind) => LOADERS.find((l) => l.kind === k)?.label ?? k;

  return (
    <section className="rounded-lg border border-line bg-surface-1/85 p-5 backdrop-blur">
      <h2 className="flex items-center gap-2 font-display text-lg font-semibold">
        <Gamepad2 size={18} className="text-accent" />
        {t("skins.inGame.title")}
      </h2>
      <p className="mt-1 text-sm text-muted">{t("skins.inGame.body")}</p>

      {modded.length === 0 ? (
        <p className="mt-4 text-sm text-muted">{t("skins.inGame.noModded")}</p>
      ) : (
        <ul className="mt-4 flex flex-col divide-y divide-line rounded-md border border-line">
          {modded.map((inst) => {
            const st = status[inst.id] ?? "checking";
            return (
              <li key={inst.id} className="flex items-center gap-3 px-3 py-2">
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm font-semibold">{inst.name}</div>
                  <div className="text-xs text-muted">
                    {loaderLabel(inst.loader.kind)} · {inst.mcVersion}
                  </div>
                  {typeof st === "object" && (
                    <div className="mt-1 text-xs text-warn">
                      {t(`errors.${st.code}`, { ...st.params, defaultValue: st.detail })}
                    </div>
                  )}
                </div>
                {st === "installed" && (
                  <Badge tone="success">
                    <CheckCircle2 size={12} />
                    {t("skins.inGame.installed")}
                  </Badge>
                )}
                {(st === "checking" || st === "busy") && (
                  <Loader2 size={16} className="animate-spin text-accent" />
                )}
                {(st === "missing" || typeof st === "object") && (
                  <Button size="sm" onClick={() => void install(inst)}>
                    <Download size={13} />
                    {t("skins.inGame.install")}
                  </Button>
                )}
              </li>
            );
          })}
        </ul>
      )}

      <p className="mt-3 flex items-start gap-2 text-xs text-muted">
        <Info size={14} className="mt-0.5 shrink-0" />
        {vanillaCount > 0 ? t("skins.inGame.vanillaNote") : t("skins.inGame.syncNote")}
      </p>
    </section>
  );
}
