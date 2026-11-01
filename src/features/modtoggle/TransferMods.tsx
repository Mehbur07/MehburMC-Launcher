import { CheckCircle2, Loader2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button, Field, Modal } from "../../components/ui";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { ErrorPayload } from "../../lib/ipc/bindings/ErrorPayload";
import type { Instance } from "../../lib/ipc/bindings/Instance";
import type { TransferResult } from "../../lib/ipc/bindings/TransferResult";
import { activeTaskFor, useTasks } from "../../stores/tasks";
import { loaderLabel } from "../instances/loaderLabels";

const holdsMods = (i: Instance) => i.loader.kind !== "vanilla" && i.loader.kind !== "optifine";

/** Same rule as the backend: these jars run on the target as they are. */
const runsUnchanged = (from: Instance, to: Instance) =>
  from.mcVersion === to.mcVersion &&
  (from.loader.kind === to.loader.kind ||
    (from.loader.kind === "fabric" && to.loader.kind === "quilt"));

const SECTIONS = [
  ["transferred", "text-success"],
  ["dependencies", "text-success"],
  ["alreadyPresent", "text-muted"],
  ["notFound", "text-warn"],
  ["missingDependencies", "text-warn"],
] as const;

function Result({ result }: { result: TransferResult }) {
  const { t } = useTranslation();
  const shown = SECTIONS.filter(([key]) => result[key].length > 0);
  if (shown.length === 0) return <p className="text-sm text-muted">{t("modTransfer.nothing")}</p>;
  return (
    <div className="flex max-h-80 flex-col gap-3 overflow-y-auto pr-1" role="status">
      {shown.map(([key, tone]) => (
        <section key={key}>
          <h3 className={`text-sm font-semibold ${tone}`}>
            {t(`modTransfer.sections.${key}`, { count: result[key].length })}
          </h3>
          <p className="mt-0.5 text-xs text-muted">{result[key].join(", ")}</p>
        </section>
      ))}
    </div>
  );
}

/**
 * Sends all mods of a profile to another one (K99): copies on the same game
 * version, otherwise the backend finds each mod's version on Modrinth.
 */
export function TransferMods({
  open,
  source,
  instances,
  count,
  onClose,
}: {
  open: boolean;
  source: Instance;
  instances: Instance[];
  count: number;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const others = instances.filter((i) => i.id !== source.id);
  const [targetId, setTargetId] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<TransferResult | null>(null);
  const [error, setError] = useState<ErrorPayload | null>(null);
  const target = others.find((i) => i.id === targetId);
  const targetBusy = useTasks((s) => activeTaskFor(s.tasks, targetId || null) !== null);

  useEffect(() => {
    if (!open) return;
    setResult(null);
    setError(null);
    setTargetId((cur) => {
      const usable = others.filter(holdsMods);
      return usable.some((i) => i.id === cur) ? cur : (usable[0]?.id ?? "");
    });
    // Only when the dialog opens; a game starting elsewhere must not reset it.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, source.id]);

  const start = async () => {
    if (!target) return;
    setBusy(true);
    setResult(null);
    setError(null);
    try {
      setResult(await ipc.transferMods(source.id, target.id));
    } catch (e) {
      setError(toErrorPayload(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      open={open}
      // The work belongs to this dialog: closing would lose its result.
      onClose={() => !busy && onClose()}
      title={t("modTransfer.title", { name: source.name })}
      footer={
        busy ? (
          <Button variant="ghost" onClick={() => target && void ipc.stopInstance(target.id)}>
            {t("common.cancel")}
          </Button>
        ) : (
          <>
            <Button variant="ghost" onClick={onClose}>
              {t("common.close")}
            </Button>
            {!result && (
              <Button
                variant="primary"
                disabled={!target || targetBusy || count === 0}
                onClick={() => void start()}
              >
                {t("modTransfer.start")}
              </Button>
            )}
          </>
        )
      }
    >
      <div className="flex flex-col gap-3">
        {result ? (
          <>
            <p className="flex items-center gap-2 text-sm font-semibold">
              <CheckCircle2 size={16} className="text-success" />
              {target && t("modTransfer.done", { name: target.name })}
            </p>
            <Result result={result} />
          </>
        ) : busy ? (
          <p className="flex items-center gap-2 text-sm" role="status">
            <Loader2 size={16} className="animate-spin text-accent" />
            {t("modTransfer.working")}
          </p>
        ) : (
          <>
            <Field label={t("modTransfer.target")}>
              <select
                value={targetId}
                onChange={(e) => setTargetId(e.target.value)}
                className="w-full rounded-md border border-line bg-surface-2 px-2 py-1.5 text-sm focus:border-accent focus:outline-none"
              >
                {others.map((i) => (
                  <option key={i.id} value={i.id} disabled={!holdsMods(i)}>
                    {i.name} ({i.mcVersion} · {loaderLabel(i.loader.kind)})
                    {!holdsMods(i) && ` · ${t("modTransfer.noLoader")}`}
                  </option>
                ))}
              </select>
            </Field>
            <p className="text-sm">{t("modTransfer.count", { count })}</p>
            {target && (
              <p className="text-xs text-muted">
                {runsUnchanged(source, target)
                  ? t("modTransfer.copyHint")
                  : t("modTransfer.searchHint", {
                      target: `${target.mcVersion} ${loaderLabel(target.loader.kind)}`,
                    })}
              </p>
            )}
            <p className="text-xs text-muted">{t("modTransfer.keepHint")}</p>
            {others.filter(holdsMods).length === 0 && (
              <p className="text-xs text-warn">{t("modTransfer.noTarget")}</p>
            )}
            {targetBusy && <p className="text-xs text-warn">{t("modTransfer.locked")}</p>}
            {error && (
              <p className="text-xs text-warn" role="alert">
                {t(`errors.${error.code}`, { ...error.params, defaultValue: error.detail })}
              </p>
            )}
          </>
        )}
      </div>
    </Modal>
  );
}
