import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import {
  Check,
  Download,
  FileUp,
  Pencil,
  Search,
  Shirt,
  Trash2,
  UserRound,
  Wind,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  Badge,
  Button,
  ConfirmDialog,
  EmptyState,
  IconButton,
  Modal,
  TextInput,
} from "../../components/ui";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { CapeItem } from "../../lib/ipc/bindings/CapeItem";
import type { ErrorPayload } from "../../lib/ipc/bindings/ErrorPayload";
import type { SkinItem } from "../../lib/ipc/bindings/SkinItem";
import type { TextureKind } from "../../lib/ipc/bindings/TextureKind";
import { useAccounts } from "../../stores/accounts";
import { useApp } from "../../stores/app";
import { accountCape, accountSkin, useSkins } from "../../stores/skins";
import { InGamePanel } from "./InGamePanel";
import { CapeThumb, SkinThumb } from "./SkinThumb";
import { SkinViewer3D, type BackItem, type Pose } from "./SkinViewer3D";

const PLAYER_RE = /^[A-Za-z0-9_]{1,16}$/;

type Item = { kind: "skin"; item: SkinItem } | { kind: "cape"; item: CapeItem };

export function SkinsPage() {
  const { t } = useTranslation();
  const setView = useApp((s) => s.setView);
  const accounts = useAccounts((s) => s.accounts);
  const selectedAccount = useAccounts((s) => s.selected);
  const store = useSkins();
  const { skins, capes, loaded, load, importFile, importPlayer, update, remove, assign } = store;

  const [accountId, setAccountId] = useState<string | null>(selectedAccount);
  const account = accounts.find((a) => a.id === accountId) ?? accounts[0] ?? null;
  const assignedSkin = accountSkin(store, account?.id);
  const assignedCape = accountCape(store, account?.id);

  const [tab, setTab] = useState<TextureKind>("skin");
  // Previewed (not yet applied) textures; null = show the account's own.
  const [previewSkin, setPreviewSkin] = useState<string | null>(null);
  const [previewCape, setPreviewCape] = useState<string | null>(null);
  const [pose, setPose] = useState<Pose>("walk");
  const [back, setBack] = useState<BackItem>("cape");
  const [player, setPlayer] = useState("");
  const [playerBusy, setPlayerBusy] = useState(false);
  const [playerError, setPlayerError] = useState<ErrorPayload | null>(null);
  const [renaming, setRenaming] = useState<Item | null>(null);
  const [newName, setNewName] = useState("");
  const [pendingDelete, setPendingDelete] = useState<Item | null>(null);

  useEffect(() => {
    if (!loaded) void load();
  }, [loaded, load]);

  // Switching account shows that account's own textures again.
  useEffect(() => {
    setPreviewSkin(null);
    setPreviewCape(null);
  }, [account?.id]);

  const shownSkin = skins.find((s) => s.id === previewSkin) ?? assignedSkin;
  const shownCape =
    previewCape === "none" ? null : (capes.find((c) => c.id === previewCape) ?? assignedCape);
  const previewing =
    (previewSkin !== null && previewSkin !== assignedSkin?.id) ||
    (previewCape !== null && previewCape !== (assignedCape?.id ?? "none"));

  const applyPreview = async () => {
    if (!account) return;
    if (previewSkin !== null) await assign(account.id, "skin", previewSkin);
    if (previewCape !== null)
      await assign(account.id, "cape", previewCape === "none" ? null : previewCape);
    setPreviewSkin(null);
    setPreviewCape(null);
  };

  const pickFile = async () => {
    const path = await openDialog({
      multiple: false,
      filters: [{ name: "PNG", extensions: ["png"] }],
    });
    if (typeof path !== "string") return;
    const id = await importFile(tab, path);
    if (!id) return;
    if (tab === "skin") setPreviewSkin(id);
    else setPreviewCape(id);
  };

  const fetchPlayer = async () => {
    setPlayerBusy(true);
    setPlayerError(null);
    const r = await importPlayer(player.trim());
    setPlayerBusy(false);
    if ("code" in r) {
      setPlayerError(r);
      return;
    }
    setPlayer("");
    if (r.skin) setPreviewSkin(r.skin.id);
    if (r.cape) setPreviewCape(r.cape.id);
    if (!r.skin && !r.cape) setPlayerError({ code: "skin.noTextures", params: {}, detail: "" });
  };

  const exportItem = async (it: Item) => {
    const dest = await saveDialog({
      defaultPath: `${it.item.name}.png`,
      filters: [{ name: "PNG", extensions: ["png"] }],
    });
    if (!dest) return;
    try {
      await ipc.exportSkin(it.item.id, dest);
    } catch (e) {
      useApp.setState({ notice: toErrorPayload(e) });
    }
  };

  if (accounts.length === 0) {
    return (
      <div className="mx-auto flex max-w-3xl flex-col gap-5">
        <h1 className="font-display text-3xl font-bold tracking-wide">{t("nav.skins")}</h1>
        <EmptyState icon={<UserRound size={30} />} title={t("skins.noAccount")}>
          <Button className="mt-2" onClick={() => setView("accounts")}>
            {t("nav.accounts")}
          </Button>
        </EmptyState>
      </div>
    );
  }

  const list: Item[] =
    tab === "skin"
      ? skins.map((item) => ({ kind: "skin", item }))
      : capes.map((item) => ({ kind: "cape", item }));
  const activeId = tab === "skin" ? shownSkin?.id : shownCape?.id;
  const appliedId = tab === "skin" ? assignedSkin?.id : assignedCape?.id;

  return (
    <div className="mx-auto flex max-w-6xl flex-col gap-5 pb-6">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <h1 className="font-display text-3xl font-bold tracking-wide">{t("nav.skins")}</h1>
        <div role="tablist" aria-label={t("skins.account")} className="flex flex-wrap gap-1.5">
          {accounts.map((a) => {
            const active = a.id === account?.id;
            const s = accountSkin(store, a.id);
            return (
              <button
                key={a.id}
                type="button"
                role="tab"
                aria-selected={active}
                onClick={() => setAccountId(a.id)}
                className={`flex items-center gap-2 rounded-md border px-2.5 py-1.5 text-sm transition-colors ${
                  active
                    ? "border-accent/60 bg-accent/10 text-accent neon-ring"
                    : "border-line bg-surface-1/85 text-muted hover:text-fg"
                }`}
              >
                <SkinThumb
                  src={s?.dataUri ?? null}
                  variant="head"
                  unit={2}
                  className="h-4 w-4 rounded-sm"
                />
                {a.name}
              </button>
            );
          })}
        </div>
      </div>

      <div className="grid gap-5 lg:grid-cols-[340px_1fr]">
        {/* 3D preview */}
        <section className="flex flex-col rounded-lg border border-line bg-surface-1/85 backdrop-blur">
          <div className="relative">
            <SkinViewer3D
              className="h-[420px] w-full"
              skin={shownSkin?.dataUri ?? null}
              model={shownSkin?.model ?? "classic"}
              cape={shownCape?.dataUri ?? null}
              back={back}
              pose={pose}
              name={account?.name ?? null}
            />
            {!shownSkin && (
              <div className="pointer-events-none absolute inset-x-0 bottom-4 text-center text-xs text-muted">
                {t("skins.noSkin")}
              </div>
            )}
          </div>
          <div className="flex flex-col gap-3 border-t border-line p-4">
            <div className="flex flex-wrap items-center gap-1.5">
              {(["idle", "walk", "run"] as const).map((p) => (
                <Button
                  key={p}
                  size="sm"
                  variant={pose === p ? "primary" : "ghost"}
                  onClick={() => setPose(p)}
                >
                  {t(`skins.pose.${p}`)}
                </Button>
              ))}
              <span className="mx-1 h-4 w-px bg-line" />
              <Button
                size="sm"
                variant="ghost"
                disabled={!shownCape}
                onClick={() => setBack(back === "cape" ? "elytra" : "cape")}
              >
                <Wind size={13} />
                {t(back === "cape" ? "skins.showElytra" : "skins.showCape")}
              </Button>
            </div>
            <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-sm">
              <dt className="text-muted">{t("skins.skin")}</dt>
              <dd className="flex min-w-0 items-center gap-2">
                <span className="truncate">{shownSkin?.name ?? t("skins.default")}</span>
                {shownSkin && <Badge>{t(`skins.model.${shownSkin.model}`)}</Badge>}
              </dd>
              <dt className="text-muted">{t("skins.cape")}</dt>
              <dd className="truncate">{shownCape?.name ?? t("skins.none")}</dd>
            </dl>
            {previewing ? (
              <div className="flex gap-2">
                <Button variant="primary" className="flex-1" onClick={() => void applyPreview()}>
                  <Check size={15} />
                  {t("skins.apply", { name: account?.name ?? "" })}
                </Button>
                <Button
                  variant="ghost"
                  onClick={() => {
                    setPreviewSkin(null);
                    setPreviewCape(null);
                  }}
                >
                  {t("common.cancel")}
                </Button>
              </div>
            ) : (
              <p className="text-xs text-muted">
                {t("skins.appliedTo", { name: account?.name ?? "" })}
              </p>
            )}
          </div>
        </section>

        {/* Library */}
        <section className="flex min-w-0 flex-col gap-3 rounded-lg border border-line bg-surface-1/85 p-4 backdrop-blur">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div role="tablist" className="flex gap-1">
              {(["skin", "cape"] as const).map((k) => (
                <Button
                  key={k}
                  role="tab"
                  aria-selected={tab === k}
                  size="sm"
                  variant={tab === k ? "secondary" : "ghost"}
                  onClick={() => setTab(k)}
                >
                  {t(k === "skin" ? "skins.tabSkins" : "skins.tabCapes", {
                    count: k === "skin" ? skins.length : capes.length,
                  })}
                </Button>
              ))}
            </div>
            <Button size="sm" onClick={() => void pickFile()}>
              <FileUp size={13} />
              {t(tab === "skin" ? "skins.importSkin" : "skins.importCape")}
            </Button>
          </div>

          <form
            className="flex gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              if (PLAYER_RE.test(player.trim()) && !playerBusy) void fetchPlayer();
            }}
          >
            <TextInput
              value={player}
              maxLength={16}
              spellCheck={false}
              placeholder={t("skins.playerPlaceholder")}
              onChange={(e) => {
                setPlayer(e.target.value);
                setPlayerError(null);
              }}
            />
            <Button
              type="submit"
              className="shrink-0 whitespace-nowrap"
              disabled={!PLAYER_RE.test(player.trim()) || playerBusy}
            >
              <Search size={14} />
              {t("skins.fetchPlayer")}
            </Button>
          </form>
          <p className={`-mt-1 text-xs ${playerError ? "text-warn" : "text-muted"}`}>
            {playerError
              ? t(`errors.${playerError.code}`, {
                  ...playerError.params,
                  defaultValue: playerError.detail,
                })
              : t("skins.playerHint")}
          </p>

          {tab === "cape" && (
            <button
              type="button"
              onClick={() => setPreviewCape("none")}
              className={`flex items-center gap-2 self-start rounded-md border px-3 py-1.5 text-sm ${
                !shownCape ? "border-accent/60 text-accent" : "border-line text-muted hover:text-fg"
              }`}
            >
              <X size={14} />
              {t("skins.noCape")}
            </button>
          )}

          {list.length === 0 ? (
            <EmptyState
              icon={<Shirt size={30} />}
              title={t(tab === "skin" ? "skins.emptySkins" : "skins.emptyCapes")}
            >
              <p>{t(tab === "skin" ? "skins.emptySkinsHint" : "skins.emptyCapesHint")}</p>
            </EmptyState>
          ) : (
            <div className="grid grid-cols-[repeat(auto-fill,minmax(132px,1fr))] gap-3">
              {list.map((it) => {
                const active = it.item.id === activeId;
                const applied = it.item.id === appliedId;
                return (
                  <div
                    key={it.item.id}
                    className={`group relative flex flex-col overflow-hidden rounded-md border bg-surface-2/70 transition-colors ${
                      active ? "border-accent/70 neon-ring" : "border-line hover:border-accent/40"
                    }`}
                  >
                    <button
                      type="button"
                      onClick={() =>
                        it.kind === "skin" ? setPreviewSkin(it.item.id) : setPreviewCape(it.item.id)
                      }
                      className="grid h-36 place-items-center bg-surface-3/40"
                      aria-label={t("skins.preview", { name: it.item.name })}
                    >
                      {it.kind === "skin" ? (
                        <SkinThumb
                          src={it.item.dataUri}
                          model={it.item.model}
                          unit={4}
                          className="h-32 w-16"
                        />
                      ) : (
                        <CapeThumb src={it.item.dataUri} unit={7} className="h-28 w-[70px]" />
                      )}
                    </button>
                    <div className="flex items-center gap-1 px-2 py-1.5">
                      <span
                        className="min-w-0 flex-1 truncate text-xs font-semibold"
                        title={it.item.name}
                      >
                        {it.item.name}
                      </span>
                      {applied && (
                        <Check
                          size={13}
                          className="shrink-0 text-success"
                          aria-label={t("skins.applied")}
                        />
                      )}
                    </div>
                    <div className="flex items-center justify-between border-t border-line px-1 py-0.5">
                      {it.kind === "skin" ? (
                        <button
                          type="button"
                          title={t("skins.toggleModel")}
                          onClick={() =>
                            void update(
                              "skin",
                              it.item.id,
                              undefined,
                              it.item.model === "slim" ? "classic" : "slim",
                            )
                          }
                          className="rounded px-1.5 py-0.5 text-[11px] text-muted hover:bg-surface-3 hover:text-accent"
                        >
                          {t(`skins.model.${it.item.model}`)}
                        </button>
                      ) : (
                        <span />
                      )}
                      <div className="flex">
                        <IconButton
                          label={t("skins.rename")}
                          className="h-7 w-7"
                          onClick={() => {
                            setRenaming(it);
                            setNewName(it.item.name);
                          }}
                        >
                          <Pencil size={13} />
                        </IconButton>
                        <IconButton
                          label={t("skins.export")}
                          className="h-7 w-7"
                          onClick={() => void exportItem(it)}
                        >
                          <Download size={13} />
                        </IconButton>
                        <IconButton
                          label={t("common.delete")}
                          className="h-7 w-7 hover:text-danger"
                          onClick={() => setPendingDelete(it)}
                        >
                          <Trash2 size={13} />
                        </IconButton>
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </section>
      </div>

      <InGamePanel />

      <Modal
        open={renaming !== null}
        onClose={() => setRenaming(null)}
        title={t("skins.rename")}
        footer={
          <>
            <Button variant="ghost" onClick={() => setRenaming(null)}>
              {t("common.cancel")}
            </Button>
            <Button
              variant="primary"
              disabled={newName.trim().length === 0}
              onClick={() => {
                if (renaming) void update(renaming.kind, renaming.item.id, newName.trim());
                setRenaming(null);
              }}
            >
              {t("common.save")}
            </Button>
          </>
        }
      >
        <TextInput
          autoFocus
          value={newName}
          maxLength={48}
          onChange={(e) => setNewName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && renaming && newName.trim()) {
              void update(renaming.kind, renaming.item.id, newName.trim());
              setRenaming(null);
            }
          }}
        />
      </Modal>

      <ConfirmDialog
        open={pendingDelete !== null}
        title={t("skins.deleteTitle")}
        message={t("skins.deleteBody", { name: pendingDelete?.item.name ?? "" })}
        confirmLabel={t("common.delete")}
        danger
        onConfirm={() => {
          if (!pendingDelete) return;
          void remove(pendingDelete.kind, pendingDelete.item.id);
          if (pendingDelete.item.id === previewSkin) setPreviewSkin(null);
          if (pendingDelete.item.id === previewCape) setPreviewCape(null);
        }}
        onClose={() => setPendingDelete(null)}
      />
    </div>
  );
}

export default SkinsPage;
