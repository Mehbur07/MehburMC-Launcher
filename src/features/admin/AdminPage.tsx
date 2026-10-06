import {
  Ban,
  Check,
  EyeOff,
  Flag,
  Loader2,
  RefreshCw,
  ScanSearch,
  Search,
  ShieldCheck,
  ShieldMinus,
  ShieldPlus,
  Send,
  Trash2,
  Undo2,
  Upload,
  Users,
  X,
} from "lucide-react";
import { useCallback, useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import {
  Badge,
  Button,
  ConfirmDialog,
  EmptyState,
  Field,
  Modal,
  TextInput,
} from "../../components/ui";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { AdminBan } from "../../lib/ipc/bindings/AdminBan";
import type { AdminEntry } from "../../lib/ipc/bindings/AdminEntry";
import type { AdminMod } from "../../lib/ipc/bindings/AdminMod";
import type { AdminModStatus } from "../../lib/ipc/bindings/AdminModStatus";
import type { AdminReport } from "../../lib/ipc/bindings/AdminReport";
import type { AdminUser } from "../../lib/ipc/bindings/AdminUser";
import type { ErrorPayload } from "../../lib/ipc/bindings/ErrorPayload";
import type { PrivateTexture } from "../../lib/ipc/bindings/PrivateTexture";
import type { SkinModel } from "../../lib/ipc/bindings/SkinModel";
import type { TextureGrant } from "../../lib/ipc/bindings/TextureGrant";
import type { TextureKind } from "../../lib/ipc/bindings/TextureKind";
import type { ScanReport } from "../../lib/ipc/bindings/ScanReport";
import { formatBytes, formatIso } from "../../lib/format";
import { useAuth } from "../../stores/auth";
import { FindingList, LoaderBadges } from "../browse/LibraryPanel";

type Tab = "queue" | "published" | "reports" | "bans" | "admins" | "private";

/** A user to act on: id plus a name to show. */
interface Target {
  userId: string;
  label: string;
}

const DURATIONS: { key: string; hours: number | null }[] = [
  { key: "day", hours: 24 },
  { key: "week", hours: 24 * 7 },
  { key: "month", hours: 24 * 30 },
  { key: "permanent", hours: null },
];

function ErrorLine({ error }: { error: ErrorPayload | null }) {
  const { t } = useTranslation();
  if (!error) return null;
  return (
    <p role="alert" className="text-sm text-danger">
      {t(`errors.${error.code}`, { ...error.params, defaultValue: error.detail })}
    </p>
  );
}

/** Loads `fetch()` on mount and on `reload()`. */
function useList<T>(fetch: () => Promise<T[]>) {
  const [items, setItems] = useState<T[] | null>(null);
  const [error, setError] = useState<ErrorPayload | null>(null);
  const [n, setN] = useState(0);
  useEffect(() => {
    let cancelled = false;
    setError(null);
    fetch()
      .then((v) => !cancelled && setItems(v))
      .catch((e) => !cancelled && setError(toErrorPayload(e)));
    return () => {
      cancelled = true;
    };
  }, [fetch, n]);
  return { items, error, reload: useCallback(() => setN((x) => x + 1), []) };
}

function ListState<T>({
  items,
  error,
  empty,
  children,
}: {
  items: T[] | null;
  error: ErrorPayload | null;
  empty: string;
  children: (items: T[]) => ReactNode;
}) {
  const { t } = useTranslation();
  if (error) return <ErrorLine error={error} />;
  if (items === null)
    return (
      <div className="flex justify-center py-6">
        <Loader2 className="animate-spin text-accent" aria-label={t("common.loading")} />
      </div>
    );
  if (items.length === 0) return <EmptyState icon={<ShieldCheck size={28} />} title={empty} />;
  return <ul className="flex flex-col gap-2">{children(items)}</ul>;
}

function Row({ children }: { children: ReactNode }) {
  return (
    <li className="flex flex-wrap items-start gap-3 rounded-lg border border-line bg-surface-1/85 px-3 py-2.5">
      {children}
    </li>
  );
}

/** Asks for a note (reject / remove) and runs `onConfirm(note)`. */
function NoteDialog({
  open,
  title,
  confirmLabel,
  onClose,
  onConfirm,
}: {
  open: boolean;
  title: string;
  confirmLabel: string;
  onClose: () => void;
  onConfirm: (note: string) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ErrorPayload | null>(null);
  useEffect(() => {
    if (open) {
      setNote("");
      setError(null);
    }
  }, [open]);
  return (
    <Modal
      open={open}
      onClose={onClose}
      title={title}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button
            variant="danger"
            disabled={busy}
            onClick={() => {
              setBusy(true);
              onConfirm(note)
                .then(onClose)
                .catch((e) => setError(toErrorPayload(e)))
                .finally(() => setBusy(false));
            }}
          >
            {confirmLabel}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-3">
        <Field label={t("admin.note")}>
          <textarea
            value={note}
            maxLength={500}
            rows={3}
            onChange={(e) => setNote(e.target.value)}
            className="w-full resize-none rounded-md border border-line bg-surface-2 px-3 py-2 text-sm focus:border-accent focus:outline-none"
          />
        </Field>
        <p className="text-xs text-muted">{t("admin.noteHint")}</p>
        <ErrorLine error={error} />
      </div>
    </Modal>
  );
}

function BanDialog({
  target,
  onClose,
  onDone,
}: {
  target: Target | null;
  onClose: () => void;
  onDone: () => void;
}) {
  const { t } = useTranslation();
  const [duration, setDuration] = useState("week");
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ErrorPayload | null>(null);
  useEffect(() => {
    if (target) {
      setDuration("week");
      setReason("");
      setError(null);
    }
  }, [target]);
  const submit = async () => {
    if (!target) return;
    setBusy(true);
    try {
      const hours = DURATIONS.find((d) => d.key === duration)?.hours ?? null;
      await ipc.adminBan(target.userId, hours, reason);
      onDone();
      onClose();
    } catch (e) {
      setError(toErrorPayload(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Modal
      open={target !== null}
      onClose={onClose}
      title={t("admin.bans.title", { name: target?.label ?? "" })}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button variant="danger" disabled={busy} onClick={() => void submit()}>
            <Ban size={14} />
            {t("admin.bans.submit")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-3">
        <fieldset className="flex flex-col gap-1.5">
          <legend className="mb-1.5 text-xs font-semibold tracking-wide text-muted uppercase">
            {t("admin.bans.duration")}
          </legend>
          {DURATIONS.map((d) => (
            <label key={d.key} className="flex items-center gap-2 text-sm">
              <input
                type="radio"
                name="ban-duration"
                checked={duration === d.key}
                onChange={() => setDuration(d.key)}
                className="accent-[rgb(var(--mc-accent-rgb))]"
              />
              {t(`admin.bans.durations.${d.key}`)}
            </label>
          ))}
        </fieldset>
        <Field label={t("admin.bans.reason")}>
          <TextInput value={reason} maxLength={500} onChange={(e) => setReason(e.target.value)} />
        </Field>
        <p className="text-xs text-muted">{t("admin.bans.hint")}</p>
        <ErrorLine error={error} />
      </div>
    </Modal>
  );
}

/** Finds accounts by name or friend code; `action` renders per-user buttons. */
function UserSearch({ action }: { action: (u: AdminUser) => ReactNode }) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [users, setUsers] = useState<AdminUser[] | null>(null);
  const [error, setError] = useState<ErrorPayload | null>(null);
  const search = async () => {
    setError(null);
    try {
      setUsers(await ipc.adminFindUsers(query));
    } catch (e) {
      setError(toErrorPayload(e));
    }
  };
  return (
    <div className="flex flex-col gap-2">
      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          void search();
        }}
      >
        <TextInput
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t("admin.users.search")}
          aria-label={t("admin.users.search")}
        />
        <Button type="submit" disabled={query.trim().length < 2}>
          <Search size={14} />
          {t("admin.users.find")}
        </Button>
      </form>
      <ErrorLine error={error} />
      {users && users.length === 0 && <p className="text-sm text-muted">{t("admin.users.none")}</p>}
      {users && users.length > 0 && (
        <ul className="flex flex-col gap-1.5">
          {users.map((u) => (
            <li
              key={u.userId}
              className="flex items-center gap-2 rounded-md border border-line bg-surface-2/60 px-3 py-2 text-sm"
            >
              <span className="flex-1 truncate font-medium">{u.names.join(", ") || u.userId}</span>
              {u.rank > 0 && <Badge tone="accent">{t(`admin.rank.${u.rank}`)}</Badge>}
              {u.banned && <Badge tone="danger">{t("admin.bans.active")}</Badge>}
              {action(u)}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function ModInfo({ m }: { m: AdminMod }) {
  const { t } = useTranslation();
  return (
    <div className="min-w-0 flex-1">
      <div className="flex flex-wrap items-center gap-2">
        <span className="font-semibold">{m.mod.name}</span>
        <span className="text-xs text-muted">
          {m.mod.author} · {m.mod.modId} {m.mod.version} · {formatBytes(m.mod.size)}
        </span>
        {m.reports > 0 && (
          <Badge tone="danger">
            <Flag size={10} />
            {m.reports}
          </Badge>
        )}
      </div>
      {m.mod.description && <p className="text-xs text-muted">{m.mod.description}</p>}
      <LoaderBadges loaders={m.mod.loaders} gameVersions={m.mod.gameVersions} />
      {m.mod.findings.length > 0 && (
        <details className="mt-1 text-xs">
          <summary className="cursor-pointer text-muted">
            {t("admin.queue.uploaderScan", { count: m.mod.findings.length })}
          </summary>
          <div className="mt-1">
            <FindingList findings={m.mod.findings} />
          </div>
        </details>
      )}
    </div>
  );
}

function QueueTab({ status }: { status: AdminModStatus }) {
  const { t } = useTranslation();
  const fetch = useCallback(() => ipc.adminLibrary(status), [status]);
  const { items, error, reload } = useList(fetch);
  const [scans, setScans] = useState<Record<number, ScanReport | "busy" | ErrorPayload>>({});
  const [rejecting, setRejecting] = useState<AdminMod | null>(null);
  const [notice, setNotice] = useState<ErrorPayload | null>(null);

  const scan = async (m: AdminMod) => {
    setScans((s) => ({ ...s, [m.mod.id]: "busy" }));
    try {
      const r = await ipc.adminScanMod(m.mod.id, status);
      setScans((s) => ({ ...s, [m.mod.id]: r }));
    } catch (e) {
      setScans((s) => ({ ...s, [m.mod.id]: toErrorPayload(e) }));
    }
  };
  const approve = async (m: AdminMod) => {
    setNotice(null);
    try {
      await ipc.adminReviewMod(m.mod.id, status, true, "");
      reload();
    } catch (e) {
      setNotice(toErrorPayload(e));
    }
  };

  return (
    <>
      <p className="mb-3 text-xs text-muted">
        {status === "pending" ? t("admin.queue.hint") : t("admin.published.hint")}
      </p>
      <ErrorLine error={notice} />
      <ListState
        items={items}
        error={error}
        empty={status === "pending" ? t("admin.queue.empty") : t("admin.published.empty")}
      >
        {(list) =>
          list.map((m) => {
            const s = scans[m.mod.id];
            const report = s && s !== "busy" && "verdict" in s ? s : null;
            return (
              <Row key={m.mod.id}>
                <ModInfo m={m} />
                <div className="flex shrink-0 flex-col items-end gap-1.5">
                  {status === "pending" ? (
                    <div className="flex gap-1.5">
                      <Button size="sm" disabled={s === "busy"} onClick={() => void scan(m)}>
                        {s === "busy" ? (
                          <Loader2 size={13} className="animate-spin" />
                        ) : (
                          <ScanSearch size={13} />
                        )}
                        {t("admin.queue.scan")}
                      </Button>
                      <Button
                        size="sm"
                        variant="primary"
                        disabled={!report || report.verdict === "block"}
                        title={!report ? t("admin.queue.scanFirst") : undefined}
                        onClick={() => void approve(m)}
                      >
                        <Check size={13} />
                        {t("admin.queue.approve")}
                      </Button>
                      <Button size="sm" variant="danger" onClick={() => setRejecting(m)}>
                        <X size={13} />
                        {t("admin.queue.reject")}
                      </Button>
                    </div>
                  ) : (
                    <Button size="sm" variant="danger" onClick={() => setRejecting(m)}>
                      <Trash2 size={13} />
                      {t("admin.published.remove")}
                    </Button>
                  )}
                </div>
                {report && (
                  <div className="w-full">
                    <p
                      className={`mb-1 text-xs font-semibold ${
                        report.verdict === "block"
                          ? "text-danger"
                          : report.verdict === "warn"
                            ? "text-warn"
                            : "text-success"
                      }`}
                    >
                      {t("admin.queue.myScan")}: {t(`library.verdict.${report.verdict}`)}
                    </p>
                    <FindingList findings={report.findings} />
                  </div>
                )}
                {s && s !== "busy" && !("verdict" in s) && (
                  <div className="w-full">
                    <ErrorLine error={s} />
                  </div>
                )}
              </Row>
            );
          })
        }
      </ListState>
      <NoteDialog
        open={rejecting !== null}
        title={
          status === "pending"
            ? t("admin.queue.rejectTitle", { name: rejecting?.mod.name ?? "" })
            : t("admin.published.removeTitle", { name: rejecting?.mod.name ?? "" })
        }
        confirmLabel={status === "pending" ? t("admin.queue.reject") : t("admin.published.remove")}
        onClose={() => setRejecting(null)}
        onConfirm={async (note) => {
          if (!rejecting) return;
          if (status === "pending") await ipc.adminReviewMod(rejecting.mod.id, status, false, note);
          else await ipc.adminRemoveMod(rejecting.mod.id, note);
          reload();
        }}
      />
    </>
  );
}

function ReportsTab({ rank, onBan }: { rank: number; onBan: (t: Target) => void }) {
  const { t, i18n } = useTranslation();
  const { items, error, reload } = useList(ipc.adminReports);
  const [removing, setRemoving] = useState<AdminReport | null>(null);
  const [notice, setNotice] = useState<ErrorPayload | null>(null);
  const act = async (op: () => Promise<void>) => {
    setNotice(null);
    try {
      await op();
      reload();
    } catch (e) {
      setNotice(toErrorPayload(e));
    }
  };
  const reason = (r: AdminReport, code: string) =>
    r.kind === "mod" ? t(`library.report.reasons.${code}`) : t(`skins.report.reasons.${code}`);

  return (
    <>
      <p className="mb-3 text-xs text-muted">
        {rank === 1 ? t("admin.reports.hint") : t("admin.reports.hintRank2")}
      </p>
      <ErrorLine error={notice} />
      <ListState items={items} error={error} empty={t("admin.reports.empty")}>
        {(list) =>
          list.map((r) => (
            <Row key={`${r.kind}-${r.itemId}`}>
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2">
                  <Badge tone={r.kind === "mod" ? "accent" : "neutral"}>
                    {t(`admin.reports.kind.${r.kind}`)}
                  </Badge>
                  <span className="font-semibold">{r.name}</span>
                  <span className="text-xs text-muted">{r.author}</span>
                  <Badge tone="danger">
                    <Flag size={10} />
                    {r.reports}
                  </Badge>
                </div>
                <p className="text-xs text-muted">
                  {r.reasons.map((c) => reason(r, c)).join(" · ")} ·{" "}
                  {formatIso(r.lastReport, i18n.language)}
                </p>
                {r.notes.length > 0 && (
                  <ul className="mt-1 list-disc pl-4 text-xs">
                    {r.notes.slice(0, 5).map((n, i) => (
                      <li key={i}>{n}</li>
                    ))}
                  </ul>
                )}
              </div>
              <div className="flex shrink-0 flex-wrap justify-end gap-1.5">
                {r.kind === "mod" ? (
                  <Button size="sm" variant="danger" onClick={() => setRemoving(r)}>
                    <Trash2 size={13} />
                    {t("admin.published.remove")}
                  </Button>
                ) : (
                  <Button
                    size="sm"
                    variant="danger"
                    onClick={() => void act(() => ipc.adminHideTexture(r.itemId))}
                  >
                    <EyeOff size={13} />
                    {t("admin.reports.hide")}
                  </Button>
                )}
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => void act(() => ipc.adminDismissReports(r.kind, r.itemId))}
                >
                  <Check size={13} />
                  {t("admin.reports.dismiss")}
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => onBan({ userId: r.owner, label: r.author })}
                >
                  <Ban size={13} />
                  {t("admin.reports.banOwner")}
                </Button>
              </div>
            </Row>
          ))
        }
      </ListState>
      <NoteDialog
        open={removing !== null}
        title={t("admin.published.removeTitle", { name: removing?.name ?? "" })}
        confirmLabel={t("admin.published.remove")}
        onClose={() => setRemoving(null)}
        onConfirm={async (note) => {
          if (!removing) return;
          await ipc.adminRemoveMod(removing.itemId, note);
          reload();
        }}
      />
    </>
  );
}

function BansTab({ onBan, version }: { onBan: (t: Target) => void; version: number }) {
  const { t, i18n } = useTranslation();
  const fetch = useCallback(() => ipc.adminBans(), []);
  const { items, error, reload } = useList(fetch);
  const [notice, setNotice] = useState<ErrorPayload | null>(null);
  useEffect(() => {
    if (version > 0) reload();
  }, [version, reload]);
  const unban = async (b: AdminBan) => {
    setNotice(null);
    try {
      await ipc.adminUnban(b.userId);
      reload();
    } catch (e) {
      setNotice(toErrorPayload(e));
    }
  };
  return (
    <div className="flex flex-col gap-5">
      <section>
        <h2 className="mb-2 text-xs font-semibold tracking-wide text-muted uppercase">
          {t("admin.bans.new")}
        </h2>
        <UserSearch
          action={(u) =>
            u.rank === 0 && (
              <Button
                size="sm"
                variant="danger"
                onClick={() => onBan({ userId: u.userId, label: u.names[0] ?? u.userId })}
              >
                <Ban size={13} />
                {t("admin.bans.submit")}
              </Button>
            )
          }
        />
      </section>
      <section>
        <h2 className="mb-2 text-xs font-semibold tracking-wide text-muted uppercase">
          {t("admin.bans.list")}
        </h2>
        <ErrorLine error={notice} />
        <ListState items={items} error={error} empty={t("admin.bans.empty")}>
          {(list) =>
            list.map((b) => (
              <Row key={b.userId}>
                <div className="min-w-0 flex-1 text-sm">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="font-semibold">{b.names.join(", ") || b.userId}</span>
                    <Badge tone={b.active ? "danger" : "neutral"}>
                      {b.active ? t("admin.bans.active") : t("admin.bans.expired")}
                    </Badge>
                  </div>
                  <p className="text-xs text-muted">
                    {b.until
                      ? t("admin.bans.until", { date: formatIso(b.until, i18n.language) })
                      : t("admin.bans.durations.permanent")}
                    {b.reason && ` · ${b.reason}`}
                    {b.bannedBy.length > 0 && ` · ${t("admin.bans.by", { name: b.bannedBy[0] })}`}
                  </p>
                </div>
                {b.active && (
                  <Button size="sm" variant="ghost" onClick={() => void unban(b)}>
                    {t("admin.bans.unban")}
                  </Button>
                )}
              </Row>
            ))
          }
        </ListState>
      </section>
    </div>
  );
}

function AdminsTab() {
  const { t } = useTranslation();
  const fetch = useCallback(() => ipc.adminList(), []);
  const { items, error, reload } = useList<AdminEntry>(fetch);
  const [notice, setNotice] = useState<ErrorPayload | null>(null);
  const setRank = async (userId: string, admin: boolean) => {
    setNotice(null);
    try {
      await ipc.adminSetRank(userId, admin);
      reload();
    } catch (e) {
      setNotice(toErrorPayload(e));
    }
  };
  return (
    <div className="flex flex-col gap-5">
      <p className="text-xs text-muted">{t("admin.admins.hint")}</p>
      <ErrorLine error={notice} />
      <ListState items={items} error={error} empty="">
        {(list) =>
          list.map((a) => (
            <Row key={a.userId}>
              <span className="flex-1 font-semibold">{a.names.join(", ") || a.userId}</span>
              <Badge tone="accent">{t(`admin.rank.${a.rank}`)}</Badge>
              {a.rank === 2 && (
                <Button size="sm" variant="ghost" onClick={() => void setRank(a.userId, false)}>
                  <ShieldMinus size={13} />
                  {t("admin.admins.remove")}
                </Button>
              )}
            </Row>
          ))
        }
      </ListState>
      <section>
        <h2 className="mb-2 text-xs font-semibold tracking-wide text-muted uppercase">
          {t("admin.admins.add")}
        </h2>
        <UserSearch
          action={(u) =>
            u.rank === 0 &&
            !u.banned && (
              <Button size="sm" onClick={() => void setRank(u.userId, true)}>
                <ShieldPlus size={13} />
                {t("admin.admins.make")}
              </Button>
            )
          }
        />
      </section>
    </div>
  );
}

/** Uploads the PNG picked with `pickPath("privateTexture")`. */
function PrivateUploadDialog({
  file,
  onClose,
  onDone,
}: {
  file: string | null;
  onClose: () => void;
  onDone: () => void;
}) {
  const { t } = useTranslation();
  const [kind, setKind] = useState<TextureKind>("skin");
  const [model, setModel] = useState<SkinModel>("classic");
  const [name, setName] = useState("MehburMC");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ErrorPayload | null>(null);
  useEffect(() => {
    if (file) {
      setKind("skin");
      setModel("classic");
      setName("MehburMC");
      setError(null);
    }
  }, [file]);
  const submit = async () => {
    setBusy(true);
    try {
      await ipc.adminUploadPrivateTexture(kind, model, name);
      onDone();
      onClose();
    } catch (e) {
      setError(toErrorPayload(e));
    } finally {
      setBusy(false);
    }
  };
  const radio = <T extends string>(
    group: string,
    value: T,
    current: T,
    set: (v: T) => void,
    label: string,
  ) => (
    <label className="flex items-center gap-2 text-sm">
      <input
        type="radio"
        name={group}
        checked={current === value}
        onChange={() => set(value)}
        className="accent-[rgb(var(--mc-accent-rgb))]"
      />
      {label}
    </label>
  );
  return (
    <Modal
      open={file !== null}
      onClose={onClose}
      title={t("admin.private.uploadTitle")}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button
            variant="primary"
            disabled={busy || name.trim() === ""}
            onClick={() => void submit()}
          >
            {busy ? <Loader2 size={14} className="animate-spin" /> : <Upload size={14} />}
            {t("admin.private.upload")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-3">
        <p className="truncate font-mono text-xs text-muted" title={file ?? ""}>
          {file}
        </p>
        <fieldset className="flex gap-4">
          {radio("pt-kind", "skin" as TextureKind, kind, setKind, t("skins.skin"))}
          {radio("pt-kind", "cape" as TextureKind, kind, setKind, t("skins.cape"))}
        </fieldset>
        {kind === "skin" && (
          <fieldset className="flex gap-4">
            {radio("pt-model", "classic" as SkinModel, model, setModel, t("skins.model.classic"))}
            {radio("pt-model", "slim" as SkinModel, model, setModel, t("skins.model.slim"))}
          </fieldset>
        )}
        <Field label={t("admin.private.name")}>
          <TextInput value={name} maxLength={48} onChange={(e) => setName(e.target.value)} />
        </Field>
        <p className="text-xs text-muted">{t("admin.private.uploadHint")}</p>
        <ErrorLine error={error} />
      </div>
    </Modal>
  );
}

/** Who has a private texture; give or take it back. */
function GrantsPanel({ texture, onChange }: { texture: PrivateTexture; onChange: () => void }) {
  const { t } = useTranslation();
  const fetch = useCallback(() => ipc.adminTextureGrants(texture.id), [texture.id]);
  const { items, error, reload } = useList<TextureGrant>(fetch);
  const [notice, setNotice] = useState<ErrorPayload | null>(null);
  const set = async (userId: string, grant: boolean) => {
    setNotice(null);
    try {
      await ipc.adminSetTextureGrant(texture.id, userId, grant);
      reload();
      onChange();
    } catch (e) {
      setNotice(toErrorPayload(e));
    }
  };
  const has = (userId: string) => items?.some((g) => g.userId === userId) ?? false;
  return (
    <div className="mt-2 flex w-full flex-col gap-3 border-t border-line pt-3">
      <ErrorLine error={notice} />
      <ListState items={items} error={error} empty={t("admin.private.noGrants")}>
        {(list) =>
          list.map((g) => (
            <Row key={g.userId}>
              <span className="flex-1 text-sm font-medium">{g.names.join(", ") || g.userId}</span>
              <Button size="sm" variant="ghost" onClick={() => void set(g.userId, false)}>
                <Undo2 size={13} />
                {t("admin.private.revoke")}
              </Button>
            </Row>
          ))
        }
      </ListState>
      <UserSearch
        action={(u) =>
          !has(u.userId) &&
          !u.banned && (
            <Button size="sm" onClick={() => void set(u.userId, true)}>
              <Send size={13} />
              {t("admin.private.give")}
            </Button>
          )
        }
      />
    </div>
  );
}

function PrivateTab() {
  const { t } = useTranslation();
  const fetch = useCallback(() => ipc.adminPrivateTextures(), []);
  const { items, error, reload } = useList<PrivateTexture>(fetch);
  const [file, setFile] = useState<string | null>(null);
  const [open, setOpen] = useState<number | null>(null);
  const [deleting, setDeleting] = useState<PrivateTexture | null>(null);
  const [notice, setNotice] = useState<ErrorPayload | null>(null);
  const pick = async () => {
    const path = await ipc.pickPath("privateTexture", undefined, t("admin.private.pick"));
    if (path) setFile(path);
  };
  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-start justify-between gap-3">
        <p className="text-xs text-muted">{t("admin.private.hint")}</p>
        <Button variant="primary" onClick={() => void pick()}>
          <Upload size={14} />
          {t("admin.private.upload")}
        </Button>
      </div>
      <ErrorLine error={notice} />
      <ListState items={items} error={error} empty={t("admin.private.empty")}>
        {(list) =>
          list.map((p) => (
            <Row key={p.id}>
              <div className="grid h-16 w-16 shrink-0 place-items-center rounded-md bg-surface-3/40">
                {p.dataUri ? (
                  <img
                    src={p.dataUri}
                    alt={p.name}
                    className="max-h-16 max-w-16 [image-rendering:pixelated]"
                  />
                ) : (
                  <X size={18} className="text-danger" />
                )}
              </div>
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="font-semibold">{p.name}</span>
                  <Badge>{p.kind === "skin" ? t("skins.skin") : t("skins.cape")}</Badge>
                  {p.kind === "skin" && <Badge>{t(`skins.model.${p.model}`)}</Badge>}
                </div>
                <p className="text-xs text-muted">
                  {t("admin.private.grants", { count: p.grants })}
                </p>
              </div>
              <div className="flex shrink-0 gap-1.5">
                <Button size="sm" onClick={() => setOpen(open === p.id ? null : p.id)}>
                  <Users size={13} />
                  {t("admin.private.people")}
                </Button>
                <Button size="sm" variant="danger" onClick={() => setDeleting(p)}>
                  <Trash2 size={13} />
                </Button>
              </div>
              {open === p.id && <GrantsPanel texture={p} onChange={reload} />}
            </Row>
          ))
        }
      </ListState>
      <PrivateUploadDialog file={file} onClose={() => setFile(null)} onDone={reload} />
      <ConfirmDialog
        open={deleting !== null}
        danger
        title={t("admin.private.deleteTitle", { name: deleting?.name ?? "" })}
        message={t("admin.private.deleteBody")}
        confirmLabel={t("common.delete")}
        onClose={() => setDeleting(null)}
        onConfirm={() => {
          if (!deleting) return;
          setNotice(null);
          ipc
            .adminDeletePrivateTexture(deleting.id)
            .then(reload)
            .catch((e) => setNotice(toErrorPayload(e)));
        }}
      />
    </div>
  );
}

/** Admin notifications and tools; only reachable with an admin rank (K73). */
export function AdminPage() {
  const { t } = useTranslation();
  const rank = useAuth((s) => s.status?.rank ?? 0);
  const tabs: Tab[] =
    rank === 1
      ? ["queue", "published", "reports", "bans", "admins", "private"]
      : ["published", "reports", "bans"];
  const [tab, setTab] = useState<Tab>(rank === 1 ? "queue" : "published");
  const [banTarget, setBanTarget] = useState<Target | null>(null);
  const [bansVersion, setBansVersion] = useState(0);
  const [key, setKey] = useState(0);

  if (rank === 0) return null;

  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <div className="flex items-end justify-between gap-3">
        <h1 className="font-display text-3xl font-bold tracking-wide">{t("nav.admin")}</h1>
        <div className="flex items-center gap-2">
          <Badge tone="accent">{t(`admin.rank.${rank}`)}</Badge>
          <Button size="sm" variant="ghost" onClick={() => setKey((k) => k + 1)}>
            <RefreshCw size={13} />
            {t("common.refresh")}
          </Button>
        </div>
      </div>
      <div role="tablist" className="flex gap-1 border-b border-line">
        {tabs.map((k) => (
          <button
            key={k}
            type="button"
            role="tab"
            aria-selected={tab === k}
            onClick={() => setTab(k)}
            className={`-mb-px border-b-2 px-3 py-2 text-sm font-medium transition-colors ${
              tab === k
                ? "border-accent text-accent"
                : "border-transparent text-muted hover:text-fg"
            }`}
          >
            {t(`admin.tabs.${k}`)}
          </button>
        ))}
      </div>
      <div key={`${tab}-${key}`} className="min-h-0 flex-1 overflow-y-auto pr-1 pb-4">
        {tab === "queue" && <QueueTab status="pending" />}
        {tab === "published" && <QueueTab status="approved" />}
        {tab === "reports" && <ReportsTab rank={rank} onBan={setBanTarget} />}
        {tab === "bans" && <BansTab onBan={setBanTarget} version={bansVersion} />}
        {tab === "admins" && <AdminsTab />}
        {tab === "private" && <PrivateTab />}
      </div>
      <BanDialog
        target={banTarget}
        onClose={() => setBanTarget(null)}
        onDone={() => setBansVersion((v) => v + 1)}
      />
    </div>
  );
}
