import { AlertTriangle, BadgeCheck, Download, Package } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { Badge, Button, ConfirmDialog, EmptyState } from "../../components/ui";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { ErrorPayload } from "../../lib/ipc/bindings/ErrorPayload";
import type { Friend } from "../../lib/ipc/bindings/Friend";
import type { FriendInstallResult } from "../../lib/ipc/bindings/FriendInstallResult";
import type { SharedItem } from "../../lib/ipc/bindings/SharedItem";
import type { SharedList } from "../../lib/ipc/bindings/SharedList";
import { useInstances } from "../../stores/instances";
import { loaderLabel } from "../instances/loaderLabels";

const MB = 1024 * 1024;
const downloadable = (i: SharedItem) => i.source.kind !== "tooLarge";

/** A friend's shared instances and their mods, installable into mine. */
export function FriendMods({ friend }: { friend: Friend }) {
  const { t } = useTranslation();
  const instances = useInstances((s) => s.instances);
  const [lists, setLists] = useState<SharedList[] | null>(null);
  const [listId, setListId] = useState<number | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [target, setTarget] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ErrorPayload | null>(null);
  const [result, setResult] = useState<FriendInstallResult | null>(null);
  const [confirmUnverified, setConfirmUnverified] = useState(false);

  useEffect(() => {
    let alive = true;
    ipc
      .friendSharedLists(friend.id)
      .then((l) => {
        if (!alive) return;
        setLists(l);
        setListId(l[0]?.id ?? null);
      })
      .catch((e: unknown) => alive && setError(toErrorPayload(e)));
    return () => {
      alive = false;
    };
  }, [friend.id]);

  const list = lists?.find((l) => l.id === listId) ?? null;

  // New list: preselect everything downloadable; target = matching instance.
  useEffect(() => {
    if (!list) return;
    setSelected(new Set(list.items.filter(downloadable).map((i) => i.fileName)));
    setResult(null);
    const match = instances.find(
      (i) => i.mcVersion === list.mcVersion && i.loader.kind === list.loader,
    );
    setTarget((cur) => cur || match?.id || instances[0]?.id || "");
  }, [list, instances]);

  const targetInst = instances.find((i) => i.id === target);
  const mismatch =
    list && targetInst
      ? targetInst.mcVersion !== list.mcVersion || targetInst.loader.kind !== list.loader
      : false;
  const chosen = useMemo(
    () => (list ? list.items.filter((i) => selected.has(i.fileName) && downloadable(i)) : []),
    [list, selected],
  );
  const unverified = chosen.filter((i) => i.source.kind === "upload").length;

  const install = async () => {
    if (!list || !target || chosen.length === 0) return;
    setBusy(true);
    setError(null);
    try {
      setResult(
        await ipc.installFriendMods(
          list.id,
          target,
          chosen.map((i) => i.fileName),
        ),
      );
    } catch (e) {
      setError(toErrorPayload(e));
    } finally {
      setBusy(false);
    }
  };

  const toggle = (name: string) =>
    setSelected((s) => {
      const n = new Set(s);
      if (n.has(name)) n.delete(name);
      else n.add(name);
      return n;
    });

  if (lists === null) {
    return (
      <p className="m-auto p-6 text-xs text-muted">
        {error
          ? t(`errors.${error.code}`, { ...error.params, defaultValue: error.detail })
          : t("common.loading")}
      </p>
    );
  }
  if (lists.length === 0) {
    return (
      <EmptyState
        icon={<Package size={30} />}
        title={t("friends.mods.none", { name: friend.displayName })}
      >
        <p>{t("friends.mods.noneHint")}</p>
      </EmptyState>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3 p-3">
      <div className="flex flex-wrap gap-1.5">
        {lists.map((l) => (
          <Button
            key={l.id}
            size="sm"
            variant={l.id === listId ? "secondary" : "ghost"}
            onClick={() => setListId(l.id)}
          >
            {l.instanceName}
            <span className="text-muted">
              {l.mcVersion} · {loaderLabel(l.loader as never)}
            </span>
          </Button>
        ))}
      </div>

      {list && (
        <>
          <div className="flex items-center gap-2 text-xs text-muted">
            <label className="flex items-center gap-1.5">
              <input
                type="checkbox"
                checked={chosen.length === list.items.filter(downloadable).length}
                onChange={(e) =>
                  setSelected(
                    e.target.checked
                      ? new Set(list.items.filter(downloadable).map((i) => i.fileName))
                      : new Set(),
                  )
                }
              />
              {t("friends.mods.all", { count: list.items.length })}
            </label>
          </div>
          <ul className="flex max-h-[340px] flex-col divide-y divide-line overflow-y-auto rounded-md border border-line">
            {list.items.map((i) => (
              <li key={i.fileName} className="flex items-center gap-2 px-3 py-1.5 text-sm">
                <input
                  type="checkbox"
                  aria-label={i.title ?? i.fileName}
                  disabled={!downloadable(i)}
                  checked={selected.has(i.fileName) && downloadable(i)}
                  onChange={() => toggle(i.fileName)}
                />
                <span className="min-w-0 flex-1 truncate" title={i.fileName}>
                  {i.title ?? i.fileName.replace(/\.disabled$/, "")}
                  {i.versionNumber && (
                    <span className="ml-1.5 text-xs text-muted">{i.versionNumber}</span>
                  )}
                  {!i.enabled && (
                    <span className="ml-1.5 text-xs text-muted">
                      ({t("friends.mods.disabled")})
                    </span>
                  )}
                </span>
                <span className="text-[11px] text-muted">{(i.size / MB).toFixed(1)} MB</span>
                {i.source.kind === "modrinth" ? (
                  <Badge tone="success">
                    <BadgeCheck size={11} />
                    Modrinth
                  </Badge>
                ) : i.source.kind === "upload" ? (
                  <Badge tone="warn">
                    <AlertTriangle size={11} />
                    {t("friends.mods.unverified")}
                  </Badge>
                ) : (
                  <Badge>{t("friends.mods.tooLarge")}</Badge>
                )}
              </li>
            ))}
          </ul>

          <div className="flex flex-wrap items-center gap-2">
            <label className="flex items-center gap-2 text-sm">
              {t("friends.mods.target")}
              <select
                value={target}
                onChange={(e) => setTarget(e.target.value)}
                className="rounded-md border border-line bg-surface-2 px-2 py-1 text-sm"
              >
                {instances.map((i) => (
                  <option key={i.id} value={i.id}>
                    {i.name} ({i.mcVersion} · {loaderLabel(i.loader.kind)})
                  </option>
                ))}
              </select>
            </label>
            <Button
              variant="primary"
              className="ml-auto"
              disabled={busy || !target || chosen.length === 0}
              onClick={() => (unverified > 0 ? setConfirmUnverified(true) : void install())}
            >
              <Download size={14} />
              {busy
                ? t("friends.mods.installing")
                : t("friends.mods.install", { count: chosen.length })}
            </Button>
          </div>
          {instances.length === 0 && (
            <p className="text-xs text-warn">{t("friends.mods.noInstance")}</p>
          )}
          {mismatch && (
            <p className="text-xs text-warn">
              {t("friends.mods.mismatch", {
                theirs: `${list.mcVersion} ${loaderLabel(list.loader as never)}`,
              })}
            </p>
          )}
          {error && (
            <p className="text-xs text-warn">
              {t(`errors.${error.code}`, { ...error.params, defaultValue: error.detail })}
            </p>
          )}
          {result && (
            <p className="text-xs text-success" role="status">
              {t("friends.mods.result", {
                installed: result.installed.length,
                present: result.alreadyPresent.length,
              })}
              {result.conflicts.length > 0 &&
                ` ${t("friends.mods.conflicts", { files: result.conflicts.join(", ") })}`}
              {result.skipped.length > 0 &&
                ` ${t("friends.mods.skipped", { files: result.skipped.join(", ") })}`}
            </p>
          )}
        </>
      )}

      <ConfirmDialog
        open={confirmUnverified}
        title={t("friends.mods.unverifiedTitle")}
        message={t("friends.mods.unverifiedBody", { count: unverified, name: friend.displayName })}
        confirmLabel={t("friends.mods.installAnyway")}
        danger
        onConfirm={() => void install()}
        onClose={() => setConfirmUnverified(false)}
      />
    </div>
  );
}
