import { Share2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Badge, Button } from "../../components/ui";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { ErrorPayload } from "../../lib/ipc/bindings/ErrorPayload";
import type { SharedList } from "../../lib/ipc/bindings/SharedList";
import { useInstances } from "../../stores/instances";
import { loaderLabel } from "../instances/loaderLabels";

/** The user's own instances: share / refresh / stop sharing their mod list. */
export function MyShares() {
  const { t } = useTranslation();
  const instances = useInstances((s) => s.instances);
  const [shares, setShares] = useState<SharedList[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<ErrorPayload | null>(null);

  useEffect(() => {
    ipc
      .myShares()
      .then(setShares)
      .catch((e: unknown) => setError(toErrorPayload(e)));
  }, []);

  const run = async (id: string, share: boolean) => {
    setBusy(id);
    setError(null);
    try {
      if (share) {
        const list = await ipc.shareInstance(id);
        setShares((s) => [...s.filter((x) => x.instanceId !== id), list]);
      } else {
        await ipc.unshareInstance(id);
        setShares((s) => s.filter((x) => x.instanceId !== id));
      }
    } catch (e) {
      setError(toErrorPayload(e));
    } finally {
      setBusy(null);
    }
  };

  if (instances.length === 0) return null;

  return (
    <section className="flex flex-col gap-2 rounded-lg border border-line bg-surface-1/85 p-4 backdrop-blur">
      <h2 className="flex items-center gap-2 text-sm font-semibold">
        <Share2 size={15} className="text-accent" />
        {t("friends.shares.title")}
      </h2>
      <p className="text-xs text-muted">{t("friends.shares.hint")}</p>
      <ul className="flex flex-col divide-y divide-line">
        {instances.map((i) => {
          const share = shares.find((s) => s.instanceId === i.id);
          return (
            <li key={i.id} className="flex flex-wrap items-center gap-2 py-2 text-sm">
              <span className="min-w-0 flex-1 truncate font-semibold">{i.name}</span>
              <span className="text-xs text-muted">
                {i.mcVersion} · {loaderLabel(i.loader.kind)}
              </span>
              {share && (
                <Badge tone="accent">
                  {t("friends.shares.shared", { count: share.items.length })}
                </Badge>
              )}
              <Button size="sm" disabled={busy !== null} onClick={() => void run(i.id, true)}>
                {busy === i.id
                  ? t("friends.shares.working")
                  : share
                    ? t("friends.shares.refresh")
                    : t("friends.shares.share")}
              </Button>
              {share && (
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={busy !== null}
                  onClick={() => void run(i.id, false)}
                >
                  {t("friends.shares.stop")}
                </Button>
              )}
            </li>
          );
        })}
      </ul>
      {error && (
        <p className="text-xs text-warn">
          {t(`errors.${error.code}`, { ...error.params, defaultValue: error.detail })}
        </p>
      )}
    </section>
  );
}
