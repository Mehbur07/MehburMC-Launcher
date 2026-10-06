import { Flag, Globe, Share2, Undo2, Users } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button, Field, Modal, TextInput } from "../../components/ui";
import type { ErrorPayload } from "../../lib/ipc/bindings/ErrorPayload";
import type { ReportReason } from "../../lib/ipc/bindings/ReportReason";
import type { SharedTexture } from "../../lib/ipc/bindings/SharedTexture";
import type { TextureKind } from "../../lib/ipc/bindings/TextureKind";
import type { Visibility } from "../../lib/ipc/bindings/Visibility";
import { selectedAccount, useAccounts } from "../../stores/accounts";
import { useCommunity } from "../../stores/community";
import { useFriends } from "../../stores/friends";

function ErrorLine({ error }: { error: ErrorPayload | null }) {
  const { t } = useTranslation();
  if (!error) return null;
  return (
    <p role="alert" className="text-sm text-danger">
      {t(`errors.${error.code}`, { ...error.params, defaultValue: error.detail })}
    </p>
  );
}

/** Small marker on a library card that is shared. */
export function VisibilityIcon({
  visibility,
  size = 12,
}: {
  visibility: Visibility;
  size?: number;
}) {
  const { t } = useTranslation();
  const label = t(`skins.share.visibility.${visibility}`);
  const Icon = visibility === "public" ? Globe : Users;
  return (
    <span role="img" aria-label={label} title={label} className="inline-flex shrink-0 text-accent">
      <Icon size={size} />
    </span>
  );
}

export interface ShareTarget {
  kind: TextureKind;
  id: string;
  name: string;
}

export function ShareDialog({
  target,
  existing,
  onClose,
}: {
  target: ShareTarget | null;
  existing: SharedTexture | null;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const account = useAccounts(selectedAccount);
  const friendsOn = useFriends((s) => s.enabled === true);
  const share = useCommunity((s) => s.share);
  const unshare = useCommunity((s) => s.unshare);
  const [name, setName] = useState("");
  const [visibility, setVisibility] = useState<Visibility>("public");
  const [error, setError] = useState<ErrorPayload | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!target) return;
    setName(existing?.name ?? target.name);
    setVisibility(existing?.visibility ?? "public");
    setError(null);
  }, [target, existing]);

  const run = async (op: () => Promise<ErrorPayload | null>) => {
    setBusy(true);
    const e = await op();
    setBusy(false);
    setError(e);
    if (!e) onClose();
  };

  const options: Visibility[] = ["public", "friends"];

  return (
    <Modal
      open={target !== null}
      onClose={onClose}
      title={existing ? t("skins.share.manageTitle") : t("skins.share.title")}
      footer={
        <>
          {existing && (
            <Button
              variant="danger"
              className="mr-auto"
              disabled={busy}
              onClick={() => void run(() => unshare(existing.id))}
            >
              <Undo2 size={14} />
              {t("skins.share.withdraw")}
            </Button>
          )}
          <Button variant="ghost" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button
            variant="primary"
            disabled={busy || !account || name.trim() === ""}
            onClick={() =>
              target && void run(() => share(target.kind, target.id, name.trim(), visibility))
            }
          >
            <Share2 size={14} />
            {existing ? t("common.save") : t("skins.share.submit")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-4">
        <Field label={t("skins.share.name")}>
          <TextInput value={name} maxLength={48} onChange={(e) => setName(e.target.value)} />
        </Field>
        <fieldset className="flex flex-col gap-2">
          <legend className="mb-1.5 text-xs font-semibold tracking-wide text-muted uppercase">
            {t("skins.share.who")}
          </legend>
          {options.map((v) => {
            const off = v === "friends" && !friendsOn;
            return (
              <label
                key={v}
                className={`flex items-start gap-2 text-sm ${off ? "opacity-50" : ""}`}
              >
                <input
                  type="radio"
                  name="visibility"
                  disabled={off}
                  checked={visibility === v}
                  onChange={() => setVisibility(v)}
                  className="mt-1 accent-[rgb(var(--mc-accent-rgb))]"
                />
                <span>
                  <span className="flex items-center gap-1.5 font-medium">
                    <VisibilityIcon visibility={v} size={13} />
                    {t(`skins.share.visibility.${v}`)}
                  </span>
                  <span className="block text-xs text-muted">
                    {off ? t("skins.share.friendsOff") : t(`skins.share.hint.${v}`)}
                  </span>
                </span>
              </label>
            );
          })}
        </fieldset>
        <p className="text-xs text-muted">
          {account ? t("skins.share.author", { name: account.name }) : t("skins.share.noAccount")}
        </p>
        <p className="rounded-md border border-warn/40 bg-warn/10 px-3 py-2 text-xs text-warn">
          {t("skins.share.rules")}
        </p>
        <ErrorLine error={error} />
      </div>
    </Modal>
  );
}

const REASONS: ReportReason[] = ["inappropriate", "stolen", "spam", "other"];

export function ReportDialog({
  item,
  onClose,
  onReported,
}: {
  item: SharedTexture | null;
  onClose: () => void;
  onReported: () => void;
}) {
  const { t } = useTranslation();
  const report = useCommunity((s) => s.report);
  const [reason, setReason] = useState<ReportReason>("inappropriate");
  const [note, setNote] = useState("");
  const [error, setError] = useState<ErrorPayload | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!item) return;
    setReason("inappropriate");
    setNote("");
    setError(null);
  }, [item]);

  const submit = async () => {
    if (!item) return;
    setBusy(true);
    const e = await report(item.id, reason, note);
    setBusy(false);
    setError(e);
    if (!e) {
      onReported();
      onClose();
    }
  };

  return (
    <Modal
      open={item !== null}
      onClose={onClose}
      title={t("skins.report.title", { name: item?.name ?? "" })}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button variant="danger" disabled={busy} onClick={() => void submit()}>
            <Flag size={14} />
            {t("skins.report.submit")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-3">
        <p className="text-sm text-muted">{t("skins.report.hint")}</p>
        <fieldset className="flex flex-col gap-1.5">
          {REASONS.map((r) => (
            <label key={r} className="flex items-center gap-2 text-sm">
              <input
                type="radio"
                name="reason"
                checked={reason === r}
                onChange={() => setReason(r)}
                className="accent-[rgb(var(--mc-accent-rgb))]"
              />
              {t(`skins.report.reasons.${r}`)}
            </label>
          ))}
        </fieldset>
        <Field label={t("skins.report.note")}>
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
