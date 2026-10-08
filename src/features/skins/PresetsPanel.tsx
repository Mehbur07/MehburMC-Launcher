import { Check, Flag, Paintbrush, Plus, RefreshCw, ShieldX, Undo2 } from "lucide-react";
import { type ReactNode, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { Badge, Button, ConfirmDialog, IconButton, TextInput } from "../../components/ui";
import { ipc } from "../../lib/ipc";
import type { DefaultSkin } from "../../lib/ipc/bindings/DefaultSkin";
import type { SharedTexture } from "../../lib/ipc/bindings/SharedTexture";
import type { SkinModel } from "../../lib/ipc/bindings/SkinModel";
import type { TextureKind } from "../../lib/ipc/bindings/TextureKind";
import { toDataUri } from "./editor/canvas";
import type { Pixels } from "./editor/ops";
import { CAPE_PRESETS, SKIN_PRESETS } from "./presets";
import { CATALOG_CAPES, CATALOG_SKINS } from "./presets/catalog";
import { LIBRARY_CAPES, LIBRARY_SKINS } from "./presets/library";
import { useApp } from "../../stores/app";
import { useAuth } from "../../stores/auth";
import { useCommunity } from "../../stores/community";
import { ReportDialog, VisibilityIcon } from "./ShareDialogs";
import { CapeThumb, SkinThumb } from "./SkinThumb";

/** Collection items shown before "Show all". */
const FIRST_ITEMS = 24;

interface Drawable {
  key: string;
  kind: TextureKind;
  name: string;
  model: SkinModel;
  draw: () => Pixels;
}

// Drawn textures are kept for the session; switching tabs does not redraw.
const drawnCache = new Map<string, string>();
function drawn(d: Drawable): string {
  let uri = drawnCache.get(d.key);
  if (!uri) {
    uri = toDataUri(d.draw());
    drawnCache.set(d.key, uri);
  }
  return uri;
}

/** A texture that is not (yet) in the library. */
export interface LooseTexture {
  key: string;
  kind: TextureKind;
  name: string;
  model: SkinModel;
  dataUri: string;
  /** Set for community shares. */
  share?: SharedTexture;
}

// Defaults are read from a jar once per session.
let defaultsCache: Promise<DefaultSkin[]> | null = null;
function loadDefaults(): Promise<DefaultSkin[]> {
  defaultsCache ??= ipc.listDefaultSkins().catch(() => {
    defaultsCache = null;
    return [];
  });
  return defaultsCache;
}

export function PresetsPanel({
  kind,
  activeKey,
  onPreview,
  onAdd,
  onEdit,
}: {
  kind: TextureKind;
  activeKey: string | null;
  onPreview: (t: LooseTexture) => void;
  onAdd: (t: LooseTexture) => void;
  onEdit: (t: LooseTexture) => void;
}) {
  const { t } = useTranslation();
  const [defaults, setDefaults] = useState<DefaultSkin[] | null>(null);
  const shares = useCommunity((s) => s.items);
  const communityLoading = useCommunity((s) => s.loading);
  const communityError = useCommunity((s) => s.error);
  const loadCommunity = useCommunity((s) => s.load);
  const unshare = useCommunity((s) => s.unshare);
  const adminRemove = useCommunity((s) => s.adminRemove);
  const isFounder = useAuth((s) => s.status?.rank === 1);
  const [removing, setRemoving] = useState<SharedTexture | null>(null);
  const [reporting, setReporting] = useState<SharedTexture | null>(null);
  const [withdrawing, setWithdrawing] = useState<SharedTexture | null>(null);
  const [reported, setReported] = useState(false);

  useEffect(() => {
    let alive = true;
    void loadDefaults().then((d) => alive && setDefaults(d));
    return () => {
      alive = false;
    };
  }, []);

  // The original presets, then the phase 22 collection (K75) and the
  // catalogue (K77). Over 100 items per kind: only the visible ones are drawn.
  const [query, setQuery] = useState("");
  const [showAll, setShowAll] = useState(false);
  const collection = useMemo<Drawable[]>(() => {
    const entry = (
      prefix: string,
      p: { id: string; model?: SkinModel; draw: () => Pixels },
    ): Drawable => ({
      key: `${prefix}:${p.id}`,
      kind,
      name: t(`skins.presets.items.${p.id}`),
      model: p.model ?? "classic",
      draw: p.draw,
    });
    return kind === "skin"
      ? [
          ...SKIN_PRESETS.map((p) => entry("preset", p)),
          ...LIBRARY_SKINS.map((p) => entry("library", p)),
          ...CATALOG_SKINS.map((p) => entry("catalog", p)),
        ]
      : [
          ...CAPE_PRESETS.map((p) => entry("preset", p)),
          ...LIBRARY_CAPES.map((p) => entry("library", p)),
          ...CATALOG_CAPES.map((p) => entry("catalog", p)),
        ];
  }, [kind, t]);
  const q = query.trim().toLocaleLowerCase();
  const matching = q
    ? collection.filter((d) => d.name.toLocaleLowerCase().includes(q))
    : collection;
  const ours: LooseTexture[] = (q || showAll ? matching : matching.slice(0, FIRST_ITEMS)).map(
    (d) => ({ key: d.key, kind: d.kind, name: d.name, model: d.model, dataUri: drawn(d) }),
  );

  const game: LooseTexture[] =
    kind === "skin"
      ? (defaults ?? []).map((d) => ({
          key: `default:${d.name}:${d.model}`,
          kind: "skin",
          name: `${d.name} (${t(`skins.model.${d.model}`)})`,
          model: d.model,
          dataUri: d.dataUri,
        }))
      : [];

  const community: LooseTexture[] = (shares ?? [])
    .filter((s) => s.kind === kind)
    .map((s) => ({
      key: `community:${s.id}`,
      kind: s.kind,
      name: s.name,
      model: s.model,
      dataUri: s.dataUri,
      share: s,
    }));

  const shareActions = (it: LooseTexture): ReactNode => {
    const s = it.share;
    if (!s) return null;
    return s.mine ? (
      <IconButton
        label={t("skins.share.withdraw")}
        className="h-7 w-7 hover:text-danger"
        onClick={() => setWithdrawing(s)}
      >
        <Undo2 size={13} />
      </IconButton>
    ) : (
      <>
        {isFounder && (
          <IconButton
            label={t("skins.community.adminRemove")}
            className="h-7 w-7 hover:text-danger"
            onClick={() => setRemoving(s)}
          >
            <ShieldX size={13} />
          </IconButton>
        )}
        <IconButton
          label={t("skins.report.button")}
          className="h-7 w-7 hover:text-danger"
          onClick={() => setReporting(s)}
        >
          <Flag size={13} />
        </IconButton>
      </>
    );
  };

  const grid = (items: LooseTexture[]) => (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(112px,1fr))] gap-3">
      {items.map((it) => {
        const active = it.key === activeKey;
        return (
          <div
            key={it.key}
            className={`flex flex-col overflow-hidden rounded-md border bg-surface-2/70 transition-colors ${
              active ? "border-accent/70 neon-ring" : "border-line hover:border-accent/40"
            }`}
          >
            <button
              type="button"
              onClick={() => onPreview(it)}
              className="grid h-32 place-items-center bg-surface-3/40"
              aria-label={t("skins.preview", { name: it.name })}
            >
              {it.kind === "skin" ? (
                <SkinThumb src={it.dataUri} model={it.model} unit={3} className="h-24 w-12" />
              ) : (
                <CapeThumb src={it.dataUri} unit={6} className="h-24 w-[60px]" />
              )}
            </button>
            <div className="flex items-center gap-1 px-2 pt-1.5">
              <span className="min-w-0 flex-1 truncate text-xs font-semibold" title={it.name}>
                {it.name}
              </span>
              {it.share && <VisibilityIcon visibility={it.share.visibility} />}
            </div>
            {it.share && (
              <div className="truncate px-2 text-[11px] text-muted" title={it.share.author}>
                {it.share.mine
                  ? t("skins.community.mine")
                  : t("skins.community.by", { name: it.share.author })}
              </div>
            )}
            <div className="flex items-center justify-between px-1 pb-0.5">
              <span className="px-1 text-[11px] text-muted">
                {it.kind === "skin" && !it.key.startsWith("default:")
                  ? t(`skins.model.${it.model}`)
                  : ""}
              </span>
              <div className="flex">
                {shareActions(it)}
                <IconButton
                  label={t("skins.presets.edit", { name: it.name })}
                  className="h-7 w-7"
                  onClick={() => onEdit(it)}
                >
                  <Paintbrush size={13} />
                </IconButton>
                <IconButton
                  label={t("skins.presets.add", { name: it.name })}
                  className="h-7 w-7"
                  onClick={() => onAdd(it)}
                >
                  {active ? <Check size={13} /> : <Plus size={13} />}
                </IconButton>
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );

  return (
    <div className="flex flex-col gap-4">
      <section className="flex flex-col gap-2">
        <div className="flex items-center gap-2">
          <h3 className="text-sm font-semibold">{t("skins.community.title")}</h3>
          <Badge tone="accent">{community.length}</Badge>
          <IconButton
            label={t("common.refresh")}
            className="ml-auto h-7 w-7"
            disabled={communityLoading}
            onClick={() => void loadCommunity()}
          >
            <RefreshCw size={13} className={communityLoading ? "animate-spin" : ""} />
          </IconButton>
        </div>
        <p className="text-xs text-muted">{t("skins.community.hint")}</p>
        {reported && (
          <p role="status" className="text-xs text-success">
            {t("skins.report.thanks")}
          </p>
        )}
        {communityError && shares === null ? (
          <div className="flex items-center gap-2 text-xs text-danger">
            {communityError.code === "friends.server"
              ? t("skins.community.unavailable")
              : t(`errors.${communityError.code}`, {
                  ...communityError.params,
                  defaultValue: t("skins.community.unavailable"),
                })}
            <Button size="sm" onClick={() => void loadCommunity()}>
              {t("common.retry")}
            </Button>
          </div>
        ) : shares === null ? (
          <p className="text-xs text-muted">{t("common.loading")}</p>
        ) : community.length === 0 ? (
          <p className="text-xs text-muted">{t("skins.community.empty")}</p>
        ) : (
          grid(community)
        )}
      </section>
      {kind === "skin" && (
        <section className="flex flex-col gap-2">
          <h3 className="text-sm font-semibold">{t("skins.presets.gameDefaults")}</h3>
          {defaults === null ? (
            <p className="text-xs text-muted">{t("common.loading")}</p>
          ) : game.length === 0 ? (
            <p className="text-xs text-muted">{t("skins.presets.noDefaults")}</p>
          ) : (
            <>
              <p className="text-xs text-muted">
                {t("skins.presets.gameDefaultsHint", { version: defaults[0]?.source ?? "" })}
              </p>
              {grid(game)}
            </>
          )}
        </section>
      )}
      <section className="flex flex-col gap-2">
        <h3 className="text-sm font-semibold">{t("skins.presets.collection")}</h3>
        <p className="text-xs text-muted">{t("skins.presets.collectionHint")}</p>
        <TextInput
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t("skins.presets.search")}
          aria-label={t("skins.presets.search")}
          className="max-w-72"
        />
        {ours.length === 0 ? (
          <p className="text-xs text-muted">{t("skins.presets.noMatch")}</p>
        ) : (
          grid(ours)
        )}
        {!q && !showAll && matching.length > FIRST_ITEMS && (
          <Button size="sm" variant="ghost" className="self-start" onClick={() => setShowAll(true)}>
            {t("skins.presets.showAll", { count: matching.length })}
          </Button>
        )}
      </section>

      <ReportDialog
        item={reporting}
        onClose={() => setReporting(null)}
        onReported={() => setReported(true)}
      />
      <ConfirmDialog
        open={removing !== null}
        danger
        title={t("skins.community.adminRemoveTitle")}
        message={t("skins.community.adminRemoveBody", {
          name: removing?.name ?? "",
          author: removing?.author ?? "",
        })}
        confirmLabel={t("skins.community.adminRemove")}
        onClose={() => setRemoving(null)}
        onConfirm={() => {
          if (!removing) return;
          void adminRemove(removing.id).then((e) => e && useApp.setState({ notice: e }));
        }}
      />
      <ConfirmDialog
        open={withdrawing !== null}
        danger
        title={t("skins.share.withdrawTitle")}
        message={t("skins.share.withdrawBody", { name: withdrawing?.name ?? "" })}
        confirmLabel={t("skins.share.withdraw")}
        onClose={() => setWithdrawing(null)}
        onConfirm={() => {
          if (!withdrawing) return;
          void unshare(withdrawing.id).then((e) => e && useApp.setState({ notice: e }));
        }}
      />
    </div>
  );
}
