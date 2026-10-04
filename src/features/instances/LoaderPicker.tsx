import { FileUp, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "../../components/ui";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { LoaderKind } from "../../lib/ipc/bindings/LoaderKind";
import type { LoaderSpec } from "../../lib/ipc/bindings/LoaderSpec";
import type { LoaderVersion } from "../../lib/ipc/bindings/LoaderVersion";
import { useApp } from "../../stores/app";

export const LOADERS: { kind: LoaderKind; label: string }[] = [
  { kind: "vanilla", label: "Vanilla" },
  { kind: "fabric", label: "Fabric" },
  { kind: "quilt", label: "Quilt" },
  { kind: "forge", label: "Forge" },
  { kind: "neoForge", label: "NeoForge" },
  { kind: "legacyFabric", label: "Legacy Fabric" },
  { kind: "optifine", label: "OptiFine" },
];

/** Loaders that get the one-click Iris/Sodium (Oculus/Embeddium) setup. */
export const SHADER_LOADERS: LoaderKind[] = ["fabric", "quilt", "neoForge", "forge"];

const HINTS: Partial<Record<LoaderKind, string>> = {
  forge: "loader.forgeHint",
  neoForge: "loader.forgeHint",
  legacyFabric: "loader.legacyFabricHint",
  optifine: "loader.optifineHint",
};

type Status = "loading" | "ready" | "empty" | "error";

// Version lists rarely change while the launcher is open.
const cache = new Map<string, LoaderVersion[]>();

export function LoaderPicker({
  mc,
  value,
  onChange,
  onReady,
  onMcVersion,
}: {
  mc: string | null;
  value: LoaderSpec;
  onChange: (v: LoaderSpec) => void;
  /** Whether the current choice can be installed for `mc`. */
  onReady?: (ok: boolean) => void;
  /** An imported OptiFine jar targets another Minecraft version. */
  onMcVersion?: (mc: string) => void;
}) {
  const { t } = useTranslation();
  const [versions, setVersions] = useState<LoaderVersion[]>([]);
  const [status, setStatus] = useState<Status>("ready");
  const [error, setError] = useState<string | null>(null);
  const [reload, setReload] = useState(0);
  const [importing, setImporting] = useState(false);
  const [mismatch, setMismatch] = useState<string | null>(null);
  const kind = value.kind;

  useEffect(() => {
    if (kind === "vanilla" || !mc) {
      setVersions([]);
      setStatus("ready");
      return;
    }
    const key = `${kind}|${mc}`;
    const hit = kind === "optifine" || reload > 0 ? undefined : cache.get(key);
    if (hit) {
      setVersions(hit);
      setStatus(hit.length ? "ready" : "empty");
      return;
    }
    let alive = true;
    setStatus("loading");
    setError(null);
    ipc
      .listLoaderVersions(kind, mc)
      .then((list) => {
        if (!alive) return;
        cache.set(key, list);
        setVersions(list);
        setStatus(list.length ? "ready" : "empty");
      })
      .catch((e) => {
        if (!alive) return;
        setError(toErrorPayload(e).code);
        setStatus("error");
      });
    return () => {
      alive = false;
    };
  }, [kind, mc, reload]);

  // OptiFine has no "recommended" fallback on the backend: pin an edition.
  useEffect(() => {
    if (kind === "optifine" && status === "ready" && !value.version && versions[0]) {
      onChange({ kind, version: versions[0].id });
    }
  }, [kind, status, value.version, versions, onChange]);

  const ready =
    kind === "vanilla" || (status === "ready" && (kind !== "optifine" || !!value.version));
  useEffect(() => onReady?.(ready), [ready, onReady]);

  const importOptifine = async () => {
    const picked = await ipc.pickPath("optifine").catch(() => null);
    if (!picked) return;
    setImporting(true);
    try {
      const info = await ipc.importOptifine();
      setMismatch(null);
      if (mc && info.mcVersion !== mc) {
        if (onMcVersion) onMcVersion(info.mcVersion);
        else setMismatch(info.mcVersion);
      }
      onChange({ kind: "optifine", version: info.edition });
      setReload((n) => n + 1);
    } catch (e) {
      useApp.setState({ notice: toErrorPayload(e) });
    } finally {
      setImporting(false);
    }
  };

  const recommended = versions.find((v) => v.recommended);
  const known = !value.version || versions.some((v) => v.id === value.version);

  return (
    <div className="flex flex-col gap-3">
      <div className="grid grid-cols-4 gap-2" role="radiogroup" aria-label={t("wizard.loader")}>
        {LOADERS.map((l) => (
          <button
            key={l.kind}
            type="button"
            role="radio"
            aria-checked={kind === l.kind}
            onClick={() => onChange({ kind: l.kind })}
            className={`rounded-md border px-3 py-2 text-left text-sm font-semibold transition-colors ${
              kind === l.kind
                ? "border-accent bg-accent/10 text-accent"
                : "border-line hover:border-accent/40"
            }`}
          >
            {l.label}
          </button>
        ))}
      </div>

      {kind !== "vanilla" && (
        <div className="flex flex-col gap-2 rounded-md border border-line bg-surface-1/60 p-3">
          <div className="flex items-center gap-2">
            <span className="text-xs font-semibold tracking-wide text-muted uppercase">
              {t("loader.version")}
            </span>
            {status === "loading" && (
              <span className="text-xs text-muted">{t("common.loading")}</span>
            )}
          </div>

          {status === "ready" && versions.length > 0 && (
            <select
              aria-label={t("loader.version")}
              value={value.version ?? ""}
              onChange={(e) => onChange({ kind, version: e.target.value || undefined })}
              className="w-full rounded-md border border-line bg-surface-2 px-3 py-2 text-sm focus:border-accent focus:outline-none"
            >
              {kind !== "optifine" && (
                <option value="">
                  {t("loader.recommended", { version: recommended?.label ?? "" })}
                </option>
              )}
              {!known && value.version && <option value={value.version}>{value.version}</option>}
              {versions.map((v) => (
                <option key={v.id} value={v.id}>
                  {v.label}
                  {v.stable ? "" : ` (${t("loader.unstable")})`}
                </option>
              ))}
            </select>
          )}

          {status === "empty" && (
            <p className="text-sm text-warn">
              {kind === "optifine"
                ? t("loader.optifineNone", { mc })
                : t("loader.unavailable", {
                    loader: LOADERS.find((l) => l.kind === kind)?.label,
                    mc,
                  })}
            </p>
          )}

          {status === "error" && (
            <div className="flex items-center gap-3 text-sm text-danger">
              {t(`errors.${error}`, { defaultValue: t("errors.unknown") })}
              <Button size="sm" onClick={() => setReload((n) => n + 1)}>
                <RefreshCw size={13} />
                {t("common.retry")}
              </Button>
            </div>
          )}

          {kind === "optifine" && (
            <div>
              <Button size="sm" disabled={importing} onClick={() => void importOptifine()}>
                <FileUp size={13} />
                {t("loader.optifineImport")}
              </Button>
            </div>
          )}
          {mismatch && (
            <p className="text-xs text-warn">
              {t("loader.optifineOtherVersion", { jar: mismatch, mc })}
            </p>
          )}

          {HINTS[kind] && <p className="text-xs text-muted">{t(HINTS[kind])}</p>}
        </div>
      )}
    </div>
  );
}
