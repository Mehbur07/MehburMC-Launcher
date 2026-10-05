import { Check, ImageOff, Pencil, Plus, Shirt, Trash2, UserPlus, X } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Avatar } from "../../components/Avatar";
import { Badge, Button, ConfirmDialog, IconButton, TextInput } from "../../components/ui";
import type { Account } from "../../lib/ipc/bindings/Account";
import type { ErrorPayload } from "../../lib/ipc/bindings/ErrorPayload";
import { useAccounts } from "../../stores/accounts";
import { useApp } from "../../stores/app";
import { useFriends } from "../../stores/friends";
import { accountSkin, useSkins } from "../../stores/skins";

const NAME_RE = /^[A-Za-z0-9_]{3,16}$/;

export function AccountsPage() {
  const { t } = useTranslation();
  const accounts = useAccounts((s) => s.accounts);
  const selected = useAccounts((s) => s.selected);
  const addOffline = useAccounts((s) => s.addOffline);
  const select = useAccounts((s) => s.select);
  const remove = useAccounts((s) => s.remove);
  const rename = useAccounts((s) => s.rename);
  const avatars = useAccounts((s) => s.avatars);
  const pickPhoto = useAccounts((s) => s.pickPhoto);
  const resetPhoto = useAccounts((s) => s.resetPhoto);
  const defaultSkins = useAccounts((s) => s.defaultSkins);
  const load = useAccounts((s) => s.load);
  const online = useFriends((s) => s.enabled === true && s.online);
  const [name, setName] = useState("");
  const [error, setError] = useState<ErrorPayload | null>(null);
  const [pendingDelete, setPendingDelete] = useState<Account | null>(null);
  // Inline rename of one account.
  const [editing, setEditing] = useState<string | null>(null);
  const [newName, setNewName] = useState("");
  const [renameError, setRenameError] = useState<ErrorPayload | null>(null);
  const skins = useSkins();
  const setView = useApp((s) => s.setView);

  // Skin assignments may have changed on the Skins page.
  useEffect(() => {
    void load();
  }, [load]);

  const valid = NAME_RE.test(name);
  const add = async () => {
    const err = await addOffline(name);
    setError(err);
    if (!err) setName("");
  };

  const startRename = (a: Account) => {
    setEditing(a.id);
    setNewName(a.name);
    setRenameError(null);
  };
  const newValid = NAME_RE.test(newName.trim());
  const saveRename = async (id: string) => {
    const err = await rename(id, newName.trim());
    setRenameError(err);
    if (!err) setEditing(null);
  };

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-5 pb-6">
      <h1 className="font-display text-3xl font-bold tracking-wide">{t("nav.accounts")}</h1>

      <section className="rounded-lg border border-line bg-surface-1/85 p-5 backdrop-blur">
        <h2 className="flex items-center gap-2 font-display text-lg font-semibold">
          <UserPlus size={18} className="text-accent" />
          {t("accounts.createTitle")}
        </h2>
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
              <div className="relative shrink-0">
                <Avatar
                  photo={avatars[a.id]}
                  skin={accountSkin(skins, a.id)?.dataUri ?? defaultSkins[a.id] ?? null}
                  unit={7}
                  className="h-14 w-14 rounded-md bg-surface-3"
                  online={active && online}
                />
                <button
                  type="button"
                  aria-label={t("accounts.photoAdd", { name: a.name })}
                  title={t("accounts.photoAdd", { name: a.name })}
                  onClick={() => void pickPhoto(a.id, t("accounts.photoAdd", { name: a.name }))}
                  className="absolute -bottom-1.5 -left-1.5 grid h-6 w-6 place-items-center rounded-full border-2 border-surface-1 bg-accent text-on-accent shadow transition-transform hover:scale-110"
                >
                  <Plus size={13} strokeWidth={3} />
                </button>
              </div>
              {editing === a.id ? (
                <form
                  className="flex min-w-0 flex-1 flex-col gap-1"
                  onSubmit={(e) => {
                    e.preventDefault();
                    if (newValid) void saveRename(a.id);
                  }}
                >
                  <div className="flex gap-2">
                    <TextInput
                      autoFocus
                      value={newName}
                      maxLength={16}
                      spellCheck={false}
                      aria-label={t("accounts.rename")}
                      aria-invalid={!newValid}
                      onChange={(e) => {
                        setNewName(e.target.value);
                        setRenameError(null);
                      }}
                      onKeyDown={(e) => e.key === "Escape" && setEditing(null)}
                    />
                    <Button type="submit" size="sm" variant="primary" disabled={!newValid}>
                      {t("accounts.renameSave")}
                    </Button>
                    <IconButton label={t("common.cancel")} onClick={() => setEditing(null)}>
                      <X size={15} />
                    </IconButton>
                  </div>
                  <p className={`text-xs ${renameError || !newValid ? "text-warn" : "text-muted"}`}>
                    {renameError
                      ? t(`errors.${renameError.code}`, {
                          ...renameError.params,
                          defaultValue: renameError.detail,
                        })
                      : newValid
                        ? t("accounts.renameHint")
                        : t("accounts.nameRules")}
                  </p>
                </form>
              ) : (
                <div className="min-w-0 flex-1">
                  <div className="font-semibold">{a.name}</div>
                  <div data-selectable className="truncate font-mono text-xs text-muted">
                    {a.uuid}
                  </div>
                </div>
              )}
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
              {editing !== a.id && (
                <IconButton label={t("accounts.rename")} onClick={() => startRename(a)}>
                  <Pencil size={15} />
                </IconButton>
              )}
              {avatars[a.id] && (
                <IconButton label={t("accounts.photoReset")} onClick={() => void resetPhoto(a.id)}>
                  <ImageOff size={15} />
                </IconButton>
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
