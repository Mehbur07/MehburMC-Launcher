import { Check, Paintbrush, Plus } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { IconButton } from "../../components/ui";
import { ipc } from "../../lib/ipc";
import type { DefaultSkin } from "../../lib/ipc/bindings/DefaultSkin";
import type { SkinModel } from "../../lib/ipc/bindings/SkinModel";
import type { TextureKind } from "../../lib/ipc/bindings/TextureKind";
import { toDataUri } from "./editor/canvas";
import { CAPE_PRESETS, SKIN_PRESETS } from "./presets";
import { CapeThumb, SkinThumb } from "./SkinThumb";

/** A texture that is not (yet) in the library. */
export interface LooseTexture {
  key: string;
  kind: TextureKind;
  name: string;
  model: SkinModel;
  dataUri: string;
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

  useEffect(() => {
    let alive = true;
    void loadDefaults().then((d) => alive && setDefaults(d));
    return () => {
      alive = false;
    };
  }, []);

  const ours = useMemo<LooseTexture[]>(
    () =>
      kind === "skin"
        ? SKIN_PRESETS.map((p) => ({
            key: `preset:${p.id}`,
            kind: "skin",
            name: t(`skins.presets.items.${p.id}`),
            model: p.model,
            dataUri: toDataUri(p.draw()),
          }))
        : CAPE_PRESETS.map((p) => ({
            key: `preset:${p.id}`,
            kind: "cape",
            name: t(`skins.presets.items.${p.id}`),
            model: "classic",
            dataUri: toDataUri(p.draw()),
          })),
    [kind, t],
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
            <div className="truncate px-2 pt-1.5 text-xs font-semibold" title={it.name}>
              {it.name}
            </div>
            <div className="flex items-center justify-between px-1 pb-0.5">
              <span className="px-1 text-[11px] text-muted">
                {it.kind === "skin" && !it.key.startsWith("default:")
                  ? t(`skins.model.${it.model}`)
                  : ""}
              </span>
              <div className="flex">
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
        {grid(ours)}
      </section>
    </div>
  );
}
