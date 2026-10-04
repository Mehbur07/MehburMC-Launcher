import { Check, Info, Shirt, Trash2, UserPlus } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Badge, Button, ConfirmDialog, IconButton, TextInput } from "../../components/ui";
import type { Account } from "../../lib/ipc/bindings/Account";
import type { ErrorPayload } from "../../lib/ipc/bindings/ErrorPayload";
import { useAccounts } from "../../stores/accounts";
import { useApp } from "../../stores/app";
import { accountSkin, useSkins } from "../../stores/skins";
import { SkinThumb } from "../skins/SkinThumb";

const NAME_RE = /^[A-Za-z0-9_]{3,16}$/;

export function AccountsPage() {
  const { t } = useTranslation();
  const accounts = useAccounts((s) => s.accounts);
  const selected = useAccounts((s) => s.selected);
  const addOffline = useAccounts((s) => s.addOffline);
  const select = useAccounts((s) => s.select);
  const remove = useAccounts((s) => s.remove);
  const [name, setName] = useState("");
  const [error, setError] = useState<ErrorPayload | null>(null);
  const [pendingDelete, setPendingDelete] = useState<Account | null>(null);
  const skins = useSkins();
  const setView = useApp((s) => s.setView);

  const valid = NAME_RE.test(name);
  const add = async () => {
    const err = await addOffline(name);
    setError(err);
    if (!err) setName("");
  };

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-5 pb-6">
      <h1 className="font-display text-3xl font-bold tracking-wide">{t("nav.accounts")}</h1>

      <section className="rounded-lg border border-line bg-surface-1/85 p-5 backdrop-blur">
        <h2 className="flex items-center gap-2 font-display text-lg font-semibold">
          <UserPlus size={18} className="text-accent" />
          {t("accounts.offlineTitle")}
        </h2>
        <p className="mt-1 text-sm text-muted">{t("accounts.offlineBody")}</p>
        <form
          className="mt-4 flex gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            if (valid) void add();
          }}
        >
          <TextInput
            value={name}
            maxLength={16}
            onChange={(e) => {
              setName(e.target.value);
              setError(null);
            }}
            placeholder={t("accounts.namePlaceholder")}
            aria-invalid={name.length > 0 && !valid}
            spellCheck={false}
          />
          <Button type="submit" variant="primary" disabled={!valid}>
            {t("accounts.add")}
          </Button>
        </form>
        <p className={`mt-2 text-xs ${name.length > 0 && !valid ? "text-warn" : "text-muted"}`}>
          {error
            ? t(`errors.${error.code}`, { ...error.params, defaultValue: error.detail })
            : t("accounts.nameRules")}
        </p>
      </section>

      <section className="flex flex-col gap-2">
        {accounts.map((a, i) => {
          const active = a.id === selected;
          return (
            <div
              key={a.id}
              style={{ ["--i" as string]: i }}
              className={`rise-in flex items-center gap-3 rounded-lg border bg-surface-1/85 px-4 py-3 ${
                active ? "border-accent/60 neon-ring" : "border-line"
              }`}
            >
              <SkinThumb
                src={accountSkin(skins, a.id)?.dataUri ?? null}
                variant="head"
                unit={5}
                className="h-10 w-10 rounded-md bg-surface-3"
              />
              <div className="min-w-0 flex-1">
                <div className="font-semibold">{a.name}</div>
                <div data-selectable className="truncate font-mono text-xs text-muted">
                  {a.uuid}
                </div>
              </div>
              {active ? (
                <Badge tone="accent">
                  <Check size={12} />
                  {t("accounts.active")}
                </Badge>
              ) : (
                <Button size="sm" onClick={() => void select(a.id)}>
                  {t("accounts.use")}
                </Button>
              )}
              <IconButton label={t("accounts.editSkin")} onClick={() => setView("skins")}>
                <Shirt size={15} />
              </IconButton>
              <IconButton
                label={t("common.delete")}
                onClick={() => setPendingDelete(a)}
                className="hover:text-danger"
              >
                <Trash2 size={15} />
              </IconButton>
            </div>
          );
        })}
      </section>

      <p className="flex items-start gap-2 text-xs text-muted">
        <Info size={14} className="mt-0.5 shrink-0" />
        {t("accounts.offlineServers")}
      </p>

      <ConfirmDialog
        open={pendingDelete !== null}
        title={t("accounts.removeTitle")}
        message={t("accounts.removeBody", { name: pendingDelete?.name ?? "" })}
        confirmLabel={t("common.delete")}
        danger
        onConfirm={() => pendingDelete && void remove(pendingDelete.id)}
        onClose={() => setPendingDelete(null)}
      />
    </div>
  );
}
