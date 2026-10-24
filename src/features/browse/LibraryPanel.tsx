import {
  Boxes,
  Check,
  Download,
  Flag,
  Library,
  Loader2,
  Palette,
  RefreshCw,
  Search,
  ShieldAlert,
  ShieldCheck,
  ShieldX,
  Sparkles,
  Undo2,
  Upload,
  type LucideIcon,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
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
import { formatBytes } from "../../lib/format";
import type { ErrorPayload } from "../../lib/ipc/bindings/ErrorPayload";
import type { Finding } from "../../lib/ipc/bindings/Finding";
import type { Instance } from "../../lib/ipc/bindings/Instance";
import type { LibraryKind } from "../../lib/ipc/bindings/LibraryKind";
import type { LibraryMod } from "../../lib/ipc/bindings/LibraryMod";
import type { LibraryReportReason } from "../../lib/ipc/bindings/LibraryReportReason";
import type { LibraryScan } from "../../lib/ipc/bindings/LibraryScan";
import type { LibraryStatus } from "../../lib/ipc/bindings/LibraryStatus";
import type { LoaderKind } from "../../lib/ipc/bindings/LoaderKind";
import type { ModLoader } from "../../lib/ipc/bindings/ModLoader";
import type { PickPurpose } from "../../lib/ipc/bindings/PickPurpose";
import { selectedAccount, useAccounts } from "../../stores/accounts";
import { useInstances } from "../../stores/instances";
import { loaderLabel } from "../instances/loaderLabels";

type InstallState = "busy" | "done" | ErrorPayload;

/** What the library holds (K88), in display order. */
export const LIBRARY_KINDS: LibraryKind[] = ["mod", "modpack", "resourcepack", "shader"];

const PICK: Record<LibraryKind, PickPurpose> = {
  mod: "libraryMod",
  modpack: "libraryModpack",
  resourcepack: "libraryResourcePack",
  shader: "libraryShader",
};

const KIND_ICON: Record<LibraryKind, LucideIcon> = {
  mod: Library,
  modpack: Boxes,
  resourcepack: Palette,
  shader: Sparkles,
};

export function KindBadge({ kind }: { kind: LibraryKind }) {
  const { t } = useTranslation();
  const Icon = KIND_ICON[kind];
  return (
    <Badge tone="accent">
      <Icon size={10} />
      {t(`library.kinds.${kind}`)}
    </Badge>
  );
}

/** Same rule as `friends::library::fits` in Rust. */
export function fitsLoader(loaders: ModLoader[], kind: LoaderKind): boolean {
  const want: Partial<Record<LoaderKind, ModLoader>> = {
    fabric: "fabric",
    legacyFabric: "fabric",
    quilt: "quilt",
    forge: "forge",
    neoForge: "neoForge",
  };
  const w = want[kind];
  return w !== undefined && loaders.includes(w);
}

function ErrorLine({ error }: { error: ErrorPayload | null }) {
  const { t } = useTranslation();
  if (!error) return null;
  return (
    <p role="alert" className="text-sm text-danger">
      {t(`errors.${error.code}`, { ...error.params, defaultValue: error.detail })}
    </p>
  );
}

/** `kind` picks pack wording for the codes that differ (noDescriptor…). */
export function FindingList({
  findings,
  kind = "mod",
}: {
  findings: Finding[];
  kind?: LibraryKind;
}) {
  const { t } = useTranslation();
  if (findings.length === 0) return null;
  return (
    <ul className="flex flex-col gap-1.5">
      {findings.map((f) => (
        <li
          key={`${f.severity}-${f.code}`}
          className={`rounded-md border px-3 py-2 text-xs ${
            f.severity === "block"
              ? "border-danger/40 bg-danger/10 text-danger"
              : "border-warn/40 bg-warn/10 text-warn"
          }`}
        >
          <p className="font-medium">
            {t([`library.kindFindings.${kind}.${f.code}`, `library.findings.${f.code}`])}
          </p>
          {f.examples.length > 0 && (
            <p
              className="mt-0.5 truncate font-mono text-[10px] opacity-80"
              title={f.examples.join("\n")}
            >
              {f.examples.join(", ")}
            </p>
          )}
        </li>
      ))}
    </ul>
  );
}

function StatusBadge({ status }: { status: LibraryStatus }) {
  const { t } = useTranslation();
  const tone = status === "approved" ? "success" : status === "rejected" ? "danger" : "warn";
  return <Badge tone={tone}>{t(`library.status.${status}`)}</Badge>;
}

export function LoaderBadges({
  loaders,
  gameVersions,
}: {
  loaders: ModLoader[];
  gameVersions: string;
}) {
  const { t } = useTranslation();
  return (
    <div className="mt-1 flex flex-wrap gap-1.5">
      {loaders.map((l) => (
        <Badge key={l}>{t(`library.loaders.${l}`)}</Badge>
      ))}
      {gameVersions && <Badge>MC {gameVersions}</Badge>}
    </div>
  );
}

function UploadDialog({
  scan,
  onClose,
  onSubmitted,
}: {
  scan: LibraryScan | null;
  onClose: () => void;
  onSubmitted: () => void;
}) {
  const { t } = useTranslation();
  const account = useAccounts(selectedAccount);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ErrorPayload | null>(null);

  useEffect(() => {
    if (!scan) return;
    // Packs without a name of their own: the file name without extension.
    setName(scan.report.descriptor?.name || scan.fileName.replace(/\.[^.]+$/, ""));
    setDescription("");
    setError(null);
  }, [scan]);

  const report = scan?.report;
  const blocked = report?.verdict === "block";
  const d = report?.descriptor ?? null;
  const kind = report?.kind ?? "mod";

  const submit = async () => {
    setBusy(true);
    try {
      await ipc.librarySubmit(name.trim(), description);
      onSubmitted();
      onClose();
    } catch (e) {
      setError(toErrorPayload(e));
    } finally {
      setBusy(false);
    }
  };

  const Icon = blocked ? ShieldX : report?.verdict === "warn" ? ShieldAlert : ShieldCheck;
  const tone = blocked
    ? "border-danger/40 bg-danger/10 text-danger"
    : report?.verdict === "warn"
      ? "border-warn/40 bg-warn/10 text-warn"
      : "border-success/40 bg-success/10 text-success";

  return (
    <Modal
      open={scan !== null}
      onClose={onClose}
      wide
      title={t("library.upload.title", { file: scan?.fileName ?? "" })}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {blocked ? t("common.close") : t("common.cancel")}
          </Button>
          {!blocked && (
            <Button
              variant="primary"
              disabled={busy || !account || name.trim() === ""}
              onClick={() => void submit()}
            >
              {busy ? <Loader2 size={14} className="animate-spin" /> : <Upload size={14} />}
              {t("library.upload.submit")}
            </Button>
          )}
        </>
      }
    >
      {report && (
        <div className="flex flex-col gap-4">
          <div className={`flex items-start gap-2 rounded-md border px-3 py-2 text-sm ${tone}`}>
            <Icon size={18} className="mt-0.5 shrink-0" />
            <div className="min-w-0 flex-1">
              <p className="font-semibold">{t(`library.verdict.${report.verdict}`)}</p>
              <p className="text-xs opacity-90">
                {kind === "mod"
                  ? t("library.upload.stats", {
                      classes: report.classes,
                      size: formatBytes(report.size),
                    })
                  : formatBytes(report.size)}
              </p>
            </div>
            <KindBadge kind={kind} />
          </div>
          {d && (d.name || d.version || d.loaders.length > 0) && (
            <div className="text-sm">
              <p>
                <span className="font-semibold">{d.name}</span>{" "}
                <span className="text-muted">
                  {kind === "mod" ? `${d.modId} · ${d.version || "?"}` : d.version}
                </span>
              </p>
              <LoaderBadges loaders={d.loaders} gameVersions={d.gameVersions} />
            </div>
          )}
          {scan?.linked != null && (
            <p className="text-xs text-muted">
              {t("library.upload.packed", { linked: scan.linked, embedded: scan.embedded ?? 0 })}
            </p>
          )}
          <FindingList findings={report.findings} kind={kind} />
          {!blocked && (
            <>
              <Field label={t("library.upload.name")}>
                <TextInput value={name} maxLength={64} onChange={(e) => setName(e.target.value)} />
              </Field>
              <Field label={t("library.upload.description")}>
                <textarea
                  value={description}
                  maxLength={1000}
                  rows={3}
                  onChange={(e) => setDescription(e.target.value)}
                  className="w-full resize-none rounded-md border border-line bg-surface-2 px-3 py-2 text-sm focus:border-accent focus:outline-none"
                />
              </Field>
              <p className="text-xs text-muted">
                {account
                  ? t("library.upload.author", { name: account.name })
                  : t("library.upload.noAccount")}
              </p>
              <p className="rounded-md border border-warn/40 bg-warn/10 px-3 py-2 text-xs text-warn">
                {t("library.upload.rules")}
              </p>
            </>
          )}
          <ErrorLine error={error} />
        </div>
      )}
    </Modal>
  );
}

const REASONS: LibraryReportReason[] = ["malware", "broken", "stolen", "other"];

function ReportDialog({
  item,
  onClose,
  onReported,
}: {
  item: LibraryMod | null;
  onClose: () => void;
  onReported: () => void;
}) {
  const { t } = useTranslation();
  const [reason, setReason] = useState<LibraryReportReason>("malware");
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ErrorPayload | null>(null);

  useEffect(() => {
    if (!item) return;
    setReason("malware");
    setNote("");
    setError(null);
  }, [item]);

  const submit = async () => {
    if (!item) return;
    setBusy(true);
    try {
      await ipc.libraryReport(item.id, reason, note);
      onReported();
      onClose();
    } catch (e) {
      setError(toErrorPayload(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      open={item !== null}
      onClose={onClose}
      title={t("library.report.title", { name: item?.name ?? "" })}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button variant="danger" disabled={busy} onClick={() => void submit()}>
            <Flag size={14} />
            {t("library.report.submit")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-3">
        <p className="text-sm text-muted">{t("library.report.hint")}</p>
        <fieldset className="flex flex-col gap-1.5">
          {REASONS.map((r) => (
            <label key={r} className="flex items-center gap-2 text-sm">
              <input
                type="radio"
                name="library-reason"
                checked={reason === r}
                onChange={() => setReason(r)}
                className="accent-[rgb(var(--mc-accent-rgb))]"
              />
              {t(`library.report.reasons.${r}`)}
            </label>
          ))}
        </fieldset>
        <Field label={t("library.report.note")}>
          <textarea
            value={note}
            maxLength={500}
            rows={3}
            onChange={(e) => setNote(e.target.value)}
            className="w-full resize-none rounded-md border border-line bg-surface-2 px-3 py-2 text-sm focus:border-accent focus:outline-none"
          />
        </Field>
        <ErrorLine error={error} />
      </div>
    </Modal>
  );
}

/**
 * First upload step: what is being uploaded (decides the file dialog). A
 * modpack is one of the user's profiles, packed by the launcher (K90).
 */
function KindPickDialog({
  open,
  onClose,
  onPick,
  onPickInstance,
}: {
  open: boolean;
  onClose: () => void;
  onPick: (kind: LibraryKind) => void;
  onPickInstance: (id: string) => void;
}) {
  const { t } = useTranslation();
  const instances = useInstances((s) => s.instances);
  const [profiles, setProfiles] = useState(false);
  useEffect(() => {
    if (open) setProfiles(false);
  }, [open]);

  if (profiles) {
    return (
      <Modal
        open={open}
        onClose={onClose}
        title={t("library.upload.profileTitle")}
        footer={
          <>
            <Button variant="ghost" onClick={() => setProfiles(false)}>
              {t("common.back")}
            </Button>
            <Button variant="ghost" onClick={() => onPick("modpack")}>
              {t("library.upload.packFile")}
            </Button>
          </>
        }
      >
        <p className="mb-3 text-xs text-muted">{t("library.upload.profileHint")}</p>
        {instances.length === 0 ? (
          <p className="text-sm text-muted">{t("library.upload.noProfiles")}</p>
        ) : (
          <ul className="flex max-h-80 flex-col gap-1.5 overflow-y-auto pr-1">
            {instances.map((i) => (
              <li key={i.id}>
                <button
                  type="button"
                  onClick={() => onPickInstance(i.id)}
                  className="flex w-full items-center gap-3 rounded-lg border border-line bg-surface-2 px-3 py-2 text-left hover:border-accent focus:border-accent focus:outline-none"
                >
                  <Boxes size={16} className="shrink-0 text-accent" />
                  <span className="min-w-0 flex-1 truncate font-semibold">{i.name}</span>
                  <span className="shrink-0 text-xs text-muted">
                    {i.mcVersion} · {loaderLabel(i.loader.kind)}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </Modal>
    );
  }

  return (
    <Modal open={open} onClose={onClose} title={t("library.upload.kindTitle")}>
      <div className="grid grid-cols-2 gap-2">
        {LIBRARY_KINDS.map((k) => {
          const Icon = KIND_ICON[k];
          return (
            <button
              key={k}
              type="button"
              onClick={() => (k === "modpack" ? setProfiles(true) : onPick(k))}
              className="flex flex-col items-start gap-1 rounded-lg border border-line bg-surface-2 px-3 py-2.5 text-left hover:border-accent focus:border-accent focus:outline-none"
            >
              <span className="flex items-center gap-2 font-semibold">
                <Icon size={16} className="text-accent" />
                {t(`library.kinds.${k}`)}
              </span>
              <span className="text-xs text-muted">{t(`library.upload.kindHint.${k}`)}</span>
            </button>
          );
        })}
      </div>
    </Modal>
  );
}

/** "MehburMC Library" tab of the Mod Browser (K72, K88). */
export function LibraryPanel({ inst, gameBusy }: { inst: Instance | null; gameBusy: boolean }) {
  const { t } = useTranslation();
  const [mods, setMods] = useState<LibraryMod[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ErrorPayload | null>(null);
  const [reload, setReload] = useState(0);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<LibraryKind | "all">("all");
  const [choosingKind, setChoosingKind] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [scan, setScan] = useState<LibraryScan | null>(null);
  const [notice, setNotice] = useState<ErrorPayload | null>(null);
  /** Name of the instance a modpack install just created. */
  const [created, setCreated] = useState<string | null>(null);
  const [installs, setInstalls] = useState<Record<number, InstallState>>({});
  const [confirm, setConfirm] = useState<{ mod: LibraryMod; findings: Finding[] } | null>(null);
  const [withdrawing, setWithdrawing] = useState<LibraryMod | null>(null);
  const [reporting, setReporting] = useState<LibraryMod | null>(null);

  const refresh = useCallback(() => setReload((n) => n + 1), []);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    ipc
      .libraryMods()
      .then((list) => !cancelled && setMods(list))
      .catch((e) => !cancelled && setError(toErrorPayload(e)))
      .finally(() => !cancelled && setLoading(false));
    return () => {
      cancelled = true;
    };
  }, [reload]);

  const q = query.trim().toLowerCase();
  const matches = useCallback(
    (m: LibraryMod) =>
      (filter === "all" || m.kind === filter) &&
      (q === "" ||
        m.name.toLowerCase().includes(q) ||
        m.modId.includes(q) ||
        m.author.toLowerCase().includes(q)),
    [q, filter],
  );
  const mine = useMemo(() => mods.filter((m) => m.mine && matches(m)), [mods, matches]);
  const approved = useMemo(
    () => mods.filter((m) => !m.mine && m.status === "approved" && matches(m)),
    [mods, matches],
  );

  const pick = async (kind: LibraryKind) => {
    setChoosingKind(false);
    setNotice(null);
    const path = await ipc.pickPath(PICK[kind], undefined, t(`library.upload.pick.${kind}`));
    if (!path) return;
    setScanning(true);
    try {
      setScan(await ipc.libraryScanPick(kind));
    } catch (e) {
      setNotice(toErrorPayload(e));
    } finally {
      setScanning(false);
    }
  };

  const pickInstance = async (id: string) => {
    setChoosingKind(false);
    setNotice(null);
    setScanning(true);
    try {
      setScan(await ipc.libraryScanInstance(id));
    } catch (e) {
      setNotice(toErrorPayload(e));
    } finally {
      setScanning(false);
    }
  };

  const install = async (m: LibraryMod, accept: boolean) => {
    // A modpack becomes a new instance; everything else goes into `inst`.
    const target = m.kind === "modpack" ? null : inst?.id;
    if (target === undefined) return;
    setInstalls((s) => ({ ...s, [m.id]: "busy" }));
    try {
      const r = await ipc.libraryInstall(m.id, target, accept);
      if (r.kind === "needsConfirm") {
        setInstalls((s) => {
          const next = { ...s };
          delete next[m.id];
          return next;
        });
        setConfirm({ mod: m, findings: r.findings });
      } else {
        setInstalls((s) => ({ ...s, [m.id]: "done" }));
        if (r.kind === "imported") {
          setCreated(r.name);
          await useInstances.getState().load();
        }
      }
    } catch (e) {
      setInstalls((s) => ({ ...s, [m.id]: toErrorPayload(e) }));
    }
  };

  const withdraw = async (m: LibraryMod) => {
    try {
      await ipc.libraryWithdraw(m.id);
    } catch (e) {
      setNotice(toErrorPayload(e));
    } finally {
      refresh();
    }
  };

  const installButton = (m: LibraryMod) => {
    const st = installs[m.id];
    const pack = m.kind === "modpack";
    // Why the button is off, if it is.
    const blocker = pack
      ? null
      : !inst
        ? t("library.noInstance")
        : m.kind === "mod" && !fitsLoader(m.loaders, inst.loader.kind)
          ? t("library.wrongLoader")
          : gameBusy
            ? t("library.locked")
            : null;
    return (
      <div className="flex shrink-0 flex-col items-end gap-1">
        <Button
          size="sm"
          variant={st === "done" ? "ghost" : "primary"}
          disabled={blocker !== null || st === "busy" || st === "done"}
          title={blocker ?? undefined}
          onClick={() => void install(m, false)}
        >
          {st === "busy" ? (
            <Loader2 size={13} className="animate-spin" />
          ) : st === "done" ? (
            <Check size={13} />
          ) : (
            <Download size={13} />
          )}
          {pack
            ? st === "done"
              ? t("library.packCreated")
              : t("library.installAsInstance")
            : st === "done"
              ? t("browse.installed")
              : t("browse.install")}
        </Button>
        {typeof st === "object" && (
          <span className="max-w-56 truncate text-[11px] text-danger" title={st.detail}>
            {t(`errors.${st.code}`, { ...st.params, defaultValue: st.detail })}
          </span>
        )}
      </div>
    );
  };

  const card = (m: LibraryMod, actions: ReactNode) => {
    const Icon = KIND_ICON[m.kind];
    return (
      <li
        key={m.id}
        className="flex items-center gap-3 rounded-lg border border-line bg-surface-1/85 px-3 py-2.5"
      >
        <div className="grid size-10 shrink-0 place-items-center rounded-md bg-surface-2 text-accent">
          <Icon size={18} />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate font-semibold">{m.name}</span>
            <span className="truncate text-xs text-muted">
              {m.author} · {m.version}
            </span>
            <KindBadge kind={m.kind} />
            {m.mine && <StatusBadge status={m.status} />}
            {m.findings.length > 0 && (
              <Badge tone="warn">
                <ShieldAlert size={10} />
                {t("library.warnings", { count: m.findings.length })}
              </Badge>
            )}
          </div>
          {m.description && (
            <p className="truncate text-xs text-muted" title={m.description}>
              {m.description}
            </p>
          )}
          {m.mine && m.status === "rejected" && m.reviewNote && (
            <p className="text-xs text-danger">{t("library.reviewNote", { note: m.reviewNote })}</p>
          )}
          <LoaderBadges loaders={m.loaders} gameVersions={m.gameVersions} />
          {m.kind === "shader" && (
            <p className="mt-1 text-[11px] text-muted">{t("library.shaderHint")}</p>
          )}
        </div>
        {actions}
      </li>
    );
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4">
      <div className="flex flex-wrap items-center gap-3">
        <div className="relative min-w-60 flex-1">
          <Search
            size={15}
            className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-muted"
          />
          <TextInput
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("library.search")}
            aria-label={t("library.search")}
            className="pl-9"
          />
        </div>
        <div
          className="flex overflow-hidden rounded-md border border-line"
          role="group"
          aria-label={t("library.filterLabel")}
        >
          {(["all", ...LIBRARY_KINDS] as const).map((f) => (
            <button
              key={f}
              type="button"
              aria-pressed={filter === f}
              onClick={() => setFilter(f)}
              className={`px-2.5 py-1.5 text-xs ${filter === f ? "bg-accent/15 text-accent" : "text-muted hover:text-fg"}`}
            >
              {t(f === "all" ? "library.filterAll" : `library.kindsPlural.${f}`)}
            </button>
          ))}
        </div>
        <Button variant="primary" disabled={scanning} onClick={() => setChoosingKind(true)}>
          {scanning ? <Loader2 size={14} className="animate-spin" /> : <Upload size={14} />}
          {scanning ? t("library.scanning") : t("library.upload.button")}
        </Button>
      </div>
      <p className="text-xs text-muted">{t("library.intro")}</p>
      <ErrorLine error={notice} />
      {created && (
        <p role="status" className="text-sm text-success">
          {t("library.packCreatedNotice", { name: created })}
        </p>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto pr-1 pb-4">
        {loading && (
          <div className="flex justify-center py-4">
            <Loader2 className="animate-spin text-accent" aria-label={t("common.loading")} />
          </div>
        )}
        {!loading && error && (
          <EmptyState
            icon={<RefreshCw size={28} />}
            title={t(`errors.${error.code}`, {
              ...error.params,
              defaultValue: t("errors.unknown"),
            })}
          >
            <Button size="sm" onClick={refresh}>
              {t("common.retry")}
            </Button>
          </EmptyState>
        )}
        {!loading && !error && (
          <div className="flex flex-col gap-5">
            {mine.length > 0 && (
              <section>
                <h2 className="mb-2 text-xs font-semibold tracking-wide text-muted uppercase">
                  {t("library.mine")}
                </h2>
                <ul className="flex flex-col gap-2">
                  {mine.map((m) =>
                    card(
                      m,
                      <div className="flex shrink-0 items-center gap-2">
                        {m.status === "approved" && installButton(m)}
                        <Button size="sm" variant="ghost" onClick={() => setWithdrawing(m)}>
                          <Undo2 size={13} />
                          {t("library.withdraw")}
                        </Button>
                      </div>,
                    ),
                  )}
                </ul>
              </section>
            )}
            <section>
              <h2 className="mb-2 text-xs font-semibold tracking-wide text-muted uppercase">
                {t("library.approved")}
              </h2>
              {approved.length === 0 ? (
                <EmptyState icon={<Library size={28} />} title={t("library.empty")}>
                  <p className="text-xs text-muted">{t("library.emptyHint")}</p>
                </EmptyState>
              ) : (
                <ul className="flex flex-col gap-2">
                  {approved.map((m) =>
                    card(
                      m,
                      <div className="flex shrink-0 items-center gap-2">
                        {installButton(m)}
                        <Button
                          size="sm"
                          variant="ghost"
                          aria-label={t("library.report.button")}
                          title={t("library.report.button")}
                          onClick={() => setReporting(m)}
                        >
                          <Flag size={13} />
                        </Button>
                      </div>,
                    ),
                  )}
                </ul>
              )}
            </section>
          </div>
        )}
      </div>

      <KindPickDialog
        open={choosingKind}
        onClose={() => setChoosingKind(false)}
        onPick={(k) => void pick(k)}
        onPickInstance={(id) => void pickInstance(id)}
      />
      <UploadDialog scan={scan} onClose={() => setScan(null)} onSubmitted={refresh} />
      <ReportDialog item={reporting} onClose={() => setReporting(null)} onReported={refresh} />
      <ConfirmDialog
        open={confirm !== null}
        title={t("library.confirmTitle", { name: confirm?.mod.name ?? "" })}
        message={
          confirm && (
            <div className="flex flex-col gap-2">
              <p>{t("library.confirmMessage")}</p>
              <FindingList findings={confirm.findings} kind={confirm.mod.kind} />
            </div>
          )
        }
        confirmLabel={t("library.confirmInstall")}
        onClose={() => setConfirm(null)}
        onConfirm={() => confirm && void install(confirm.mod, true)}
      />
      <ConfirmDialog
        open={withdrawing !== null}
        danger
        title={t("library.withdrawTitle")}
        message={t("library.withdrawMessage", { name: withdrawing?.name ?? "" })}
        confirmLabel={t("library.withdraw")}
        onClose={() => setWithdrawing(null)}
        onConfirm={() => withdrawing && void withdraw(withdrawing)}
      />
    </div>
  );
}
