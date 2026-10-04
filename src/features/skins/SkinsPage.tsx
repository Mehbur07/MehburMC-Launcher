import {
  Check,
  Download,
  FileUp,
  Library,
  Paintbrush,
  Pencil,
  Shirt,
  Sparkles,
  Trash2,
  UserRound,
  Wind,
  X,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
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
import type { SkinItem } from "../../lib/ipc/bindings/SkinItem";
import type { SkinModel } from "../../lib/ipc/bindings/SkinModel";
import type { TextureKind } from "../../lib/ipc/bindings/TextureKind";
import { useAccounts } from "../../stores/accounts";
import { useApp } from "../../stores/app";
import { accountCape, accountSkin, useSkins } from "../../stores/skins";
import { type EditorSeed, SkinEditor } from "./editor/SkinEditor";
import { InGamePanel } from "./InGamePanel";
import { type LooseTexture, PresetsPanel } from "./PresetsPanel";
import { CapeThumb, SkinThumb } from "./SkinThumb";
import { SkinViewer3D, type BackItem, type Pose } from "./SkinViewer3D";

type Item = { kind: "skin"; item: SkinItem } | { kind: "cape"; item: CapeItem };
type Mode = "library" | "presets" | "editor";

/** What the 3D preview shows. */
interface Shown {
  name: string;
  model: SkinModel;
  dataUri: string;
}

let seedCounter = 0;
function newSeed(kind: TextureKind, from?: Partial<Omit<EditorSeed, "key" | "kind">>): EditorSeed {
  seedCounter += 1;
  return {
    key: `seed-${seedCounter}`,
    kind,
    name: from?.name ?? "",
    model: from?.model ?? "classic",
    dataUri: from?.dataUri ?? null,
  };
}

export function SkinsPage() {
  const { t } = useTranslation();
  const setView = useApp((s) => s.setView);
  const accounts = useAccounts((s) => s.accounts);
  const selectedAccount = useAccounts((s) => s.selected);
  const store = useSkins();
  const { skins, capes, loaded, load, importFile, addTexture, update, remove, assign } = store;

  const [accountId, setAccountId] = useState<string | null>(selectedAccount);
  const account = accounts.find((a) => a.id === accountId) ?? accounts[0] ?? null;
  const assignedSkin = accountSkin(store, account?.id);
  const assignedCape = accountCape(store, account?.id);

  const [mode, setMode] = useState<Mode>("library");
  const [tab, setTab] = useState<TextureKind>("skin");
  // Previewed (not yet applied) library textures; null = show the account's own.
  const [previewSkin, setPreviewSkin] = useState<string | null>(null);
  const [previewCape, setPreviewCape] = useState<string | null>(null);
  // Previewed textures that are not in the library (presets, the editor).
  const [looseSkin, setLooseSkin] = useState<LooseTexture | null>(null);
  const [looseCape, setLooseCape] = useState<LooseTexture | null>(null);
  const [seed, setSeed] = useState<EditorSeed>(() => newSeed("skin"));
  const [outerLayer, setOuterLayer] = useState(true);
  const [pose, setPose] = useState<Pose>("walk");
  const [back, setBack] = useState<BackItem>("cape");
  const [renaming, setRenaming] = useState<Item | null>(null);
  const [newName, setNewName] = useState("");
  const [pendingDelete, setPendingDelete] = useState<Item | null>(null);

  useEffect(() => {
    if (!loaded) void load();
  }, [loaded, load]);

  const resetPreview = () => {
    setPreviewSkin(null);
    setPreviewCape(null);
    setLooseSkin(null);
    setLooseCape(null);
  };

  // Switching account shows that account's own textures again.
  useEffect(() => {
    setPreviewSkin(null);
    setPreviewCape(null);
    setLooseSkin(null);
    setLooseCape(null);
  }, [account?.id]);

  const librarySkin = skins.find((s) => s.id === previewSkin) ?? assignedSkin;
  const libraryCape =
    previewCape === "none" ? null : (capes.find((c) => c.id === previewCape) ?? assignedCape);
  const shownSkin: Shown | null = looseSkin ?? librarySkin;
  const shownCape: Shown | null =
    looseCape ?? (libraryCape ? { ...libraryCape, model: "classic" } : null);
  const previewing =
    looseSkin !== null ||
    looseCape !== null ||
    (previewSkin !== null && previewSkin !== assignedSkin?.id) ||
    (previewCape !== null && previewCape !== (assignedCape?.id ?? "none"));

  const applyPreview = async () => {
    if (!account) return;
    let skinId = previewSkin;
    let capeId = previewCape;
    if (looseSkin)
      skinId = await addTexture("skin", looseSkin.name, looseSkin.dataUri, looseSkin.model);
    if (looseCape) capeId = await addTexture("cape", looseCape.name, looseCape.dataUri);
    if (skinId !== null) await assign(account.id, "skin", skinId);
    if (capeId !== null) await assign(account.id, "cape", capeId === "none" ? null : capeId);
    resetPreview();
  };

  const pickFile = async () => {
    const path = await ipc.pickPath(tab).catch(() => null);
    if (!path) return;
    const id = await importFile(tab);
    if (!id) return;
    previewLibrary(tab, id);
  };

  const previewLibrary = (kind: TextureKind, id: string) => {
    if (kind === "skin") {
      setLooseSkin(null);
      setPreviewSkin(id);
    } else {
      setLooseCape(null);
      setPreviewCape(id);
    }
  };

  const previewLoose = (it: LooseTexture) =>
    it.kind === "skin" ? setLooseSkin(it) : setLooseCape(it);

  const addLoose = async (it: LooseTexture) => {
    const id = await addTexture(it.kind, it.name, it.dataUri, it.model);
    if (id) previewLibrary(it.kind, id);
  };

  const openEditor = (kind: TextureKind, from?: Partial<Omit<EditorSeed, "key" | "kind">>) => {
    setTab(kind);
    setSeed(newSeed(kind, from));
    setMode("editor");
  };

  const switchMode = (m: Mode) => {
    if (m === "editor" && seed.kind !== tab) setSeed(newSeed(tab));
    if (mode === "editor" && m !== "editor") {
      setLooseSkin(null);
      setLooseCape(null);
    }
    setMode(m);
  };

  const switchKind = (k: TextureKind) => {
    setTab(k);
    if (mode === "editor" && seed.kind !== k) {
      // Drop the unsaved drawing of the other kind from the preview.
      if (seed.kind === "skin") setLooseSkin(null);
      else setLooseCape(null);
      setSeed(newSeed(k));
    }
  };

  const onLive = useCallback(
    (dataUri: string, model: SkinModel) => {
      const it: LooseTexture = {
        key: "editor",
        kind: seed.kind,
        name: t("skins.editor.title"),
        model,
        dataUri,
      };
      if (seed.kind === "skin") setLooseSkin(it);
      else setLooseCape(it);
    },
    [seed.kind, t],
  );

  const onSave = async (name: string, dataUri: string, model: SkinModel) => {
    const id = await addTexture(seed.kind, name, dataUri, model);
    return id !== null;
  };

  const exportItem = async (it: Item) => {
    try {
      const dest = await ipc.pickPath("skinExport", `${it.item.name}.png`);
      if (!dest) return;
      await ipc.exportSkin(it.item.id);
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
  const activeId = tab === "skin" ? librarySkin?.id : libraryCape?.id;
  const appliedId = tab === "skin" ? assignedSkin?.id : assignedCape?.id;
  const looseKey = (tab === "skin" ? looseSkin : looseCape)?.key ?? null;
  const modes: { id: Mode; icon: React.ReactNode }[] = [
    { id: "library", icon: <Library size={14} /> },
    { id: "presets", icon: <Sparkles size={14} /> },
    { id: "editor", icon: <Paintbrush size={14} /> },
  ];

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
        <section className="flex flex-col self-start rounded-lg border border-line bg-surface-1/85 backdrop-blur lg:sticky lg:top-0">
          <div className="relative">
            <SkinViewer3D
              className="h-[420px] w-full"
              skin={shownSkin?.dataUri ?? null}
              model={shownSkin?.model ?? "classic"}
              cape={shownCape?.dataUri ?? null}
              back={back}
              pose={pose}
              name={account?.name ?? null}
              outerLayer={mode !== "editor" || outerLayer}
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
                {mode !== "editor" && (
                  <Button variant="ghost" onClick={resetPreview}>
                    {t("common.cancel")}
                  </Button>
                )}
              </div>
            ) : (
              <p className="text-xs text-muted">
                {t("skins.appliedTo", { name: account?.name ?? "" })}
              </p>
            )}
          </div>
        </section>

        <section className="flex min-w-0 flex-col gap-3 rounded-lg border border-line bg-surface-1/85 p-4 backdrop-blur">
          <div
            role="tablist"
            aria-label={t("skins.modes")}
            className="flex gap-1 border-b border-line pb-3"
          >
            {modes.map((m) => (
              <Button
                key={m.id}
                role="tab"
                aria-selected={mode === m.id}
                variant={mode === m.id ? "primary" : "ghost"}
                size="sm"
                onClick={() => switchMode(m.id)}
              >
                {m.icon}
                {t(`skins.mode.${m.id}`)}
              </Button>
            ))}
          </div>

          <div className="flex flex-wrap items-center justify-between gap-2">
            <div role="tablist" aria-label={t("skins.kind")} className="flex gap-1">
              {(["skin", "cape"] as const).map((k) => (
                <Button
                  key={k}
                  role="tab"
                  aria-selected={tab === k}
                  size="sm"
                  variant={tab === k ? "secondary" : "ghost"}
                  onClick={() => switchKind(k)}
                >
                  {mode === "library"
                    ? t(k === "skin" ? "skins.tabSkins" : "skins.tabCapes", {
                        count: k === "skin" ? skins.length : capes.length,
                      })
                    : t(k === "skin" ? "skins.skin" : "skins.cape")}
                </Button>
              ))}
            </div>
            {mode === "library" && (
              <Button size="sm" onClick={() => void pickFile()}>
                <FileUp size={13} />
                {t(tab === "skin" ? "skins.importSkin" : "skins.importCape")}
              </Button>
            )}
          </div>

          {mode === "presets" && (
            <PresetsPanel
              kind={tab}
              activeKey={looseKey}
              onPreview={previewLoose}
              onAdd={(it) => void addLoose(it)}
              onEdit={(it) =>
                openEditor(it.kind, { name: it.name, model: it.model, dataUri: it.dataUri })
              }
            />
          )}

          {mode === "editor" && (
            <SkinEditor
              key={seed.key}
              seed={seed}
              onLive={onLive}
              onSave={onSave}
              outerLayer={outerLayer}
              onOuterLayer={setOuterLayer}
            />
          )}

          {mode === "library" && (
            <>
              {tab === "cape" && (
                <button
                  type="button"
                  onClick={() => {
                    setLooseCape(null);
                    setPreviewCape("none");
                  }}
                  className={`flex items-center gap-2 self-start rounded-md border px-3 py-1.5 text-sm ${
                    !shownCape
                      ? "border-accent/60 text-accent"
                      : "border-line text-muted hover:text-fg"
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
                  <div className="mt-3 flex justify-center gap-2">
                    <Button size="sm" onClick={() => switchMode("presets")}>
                      <Sparkles size={13} />
                      {t("skins.mode.presets")}
                    </Button>
                    <Button size="sm" onClick={() => switchMode("editor")}>
                      <Paintbrush size={13} />
                      {t("skins.mode.editor")}
                    </Button>
                  </div>
                </EmptyState>
              ) : (
                <div className="grid grid-cols-[repeat(auto-fill,minmax(132px,1fr))] gap-3">
                  {list.map((it) => {
                    const active =
                      it.item.id === activeId && !(tab === "skin" ? looseSkin : looseCape);
                    const applied = it.item.id === appliedId;
                    return (
                      <div
                        key={it.item.id}
                        className={`group relative flex flex-col overflow-hidden rounded-md border bg-surface-2/70 transition-colors ${
                          active
                            ? "border-accent/70 neon-ring"
                            : "border-line hover:border-accent/40"
                        }`}
                      >
                        <button
                          type="button"
                          onClick={() => previewLibrary(it.kind, it.item.id)}
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
                              label={t("skins.editInEditor")}
                              className="h-7 w-7"
                              onClick={() =>
                                openEditor(it.kind, {
                                  name: it.item.name,
                                  model: it.kind === "skin" ? it.item.model : "classic",
                                  dataUri: it.item.dataUri,
                                })
                              }
                            >
                              <Paintbrush size={13} />
                            </IconButton>
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
            </>
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
