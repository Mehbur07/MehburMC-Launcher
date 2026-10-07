import { Check, Loader2, Plus } from "lucide-react";
import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { Badge, Button, Modal } from "../../components/ui";
import type { Instance } from "../../lib/ipc/bindings/Instance";
import type { SkinModel } from "../../lib/ipc/bindings/SkinModel";
import type { TextureKind } from "../../lib/ipc/bindings/TextureKind";
import { useSkins } from "../../stores/skins";
import { toDataUri } from "../skins/editor/canvas";
import { LIBRARY_CAPES, LIBRARY_SKINS, mannequin } from "../skins/presets/library";
import { CapeThumb, SkinThumb } from "../skins/SkinThumb";
import { SkinViewer3D } from "../skins/SkinViewer3D";
import { LibraryPanel } from "./LibraryPanel";

type Section = "mods" | "skins" | "capes";

interface Item {
  id: string;
  kind: TextureKind;
  model: SkinModel;
  name: string;
  dataUri: string;
}

/** MehburMC Library: community mods plus the free skin/cape collection (K75). */
export function LibraryHub({ inst, gameBusy }: { inst: Instance | null; gameBusy: boolean }) {
  const { t } = useTranslation();
  const [section, setSection] = useState<Section>("mods");
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex gap-1.5" role="group" aria-label={t("library.tab")}>
        {(["mods", "skins", "capes"] as const).map((s) => (
          <Button
            key={s}
            size="sm"
            variant={section === s ? "primary" : "ghost"}
            aria-pressed={section === s}
            onClick={() => setSection(s)}
          >
            {t(`library.sections.${s}`)}
          </Button>
        ))}
      </div>
      {section === "mods" ? (
        <LibraryPanel inst={inst} gameBusy={gameBusy} />
      ) : (
        <LibraryTextures kind={section === "skins" ? "skin" : "cape"} />
      )}
    </div>
  );
}

function LibraryTextures({ kind }: { kind: TextureKind }) {
  const { t } = useTranslation();
  const addTexture = useSkins((s) => s.addTexture);
  const [preview, setPreview] = useState<Item | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [added, setAdded] = useState<Set<string>>(() => new Set());
  const figure = useMemo(() => (kind === "cape" ? toDataUri(mannequin()) : null), [kind]);

  // Drawn once per tab; ~35 small canvases.
  const items = useMemo<Item[]>(() => {
    const name = (id: string) => t(`library.textures.items.${id}`);
    return kind === "skin"
      ? LIBRARY_SKINS.map((p) => ({
          id: p.id,
          kind,
          model: p.model,
          name: name(p.id),
          dataUri: toDataUri(p.draw()),
        }))
      : LIBRARY_CAPES.map((p) => ({
          id: p.id,
          kind,
          model: "classic" as const,
          name: name(p.id),
          dataUri: toDataUri(p.draw()),
        }));
  }, [kind, t]);

  const add = async (it: Item) => {
    setBusy(it.id);
    const id = await addTexture(it.kind, it.name, it.dataUri, it.model);
    setBusy(null);
    if (id) setAdded((s) => new Set(s).add(it.id));
  };

  const addButton = (it: Item, size: "sm" | "md" = "sm") => {
    const done = added.has(it.id);
    return (
      <Button
        size={size}
        variant={done ? "ghost" : "primary"}
        disabled={done || busy === it.id}
        aria-label={t("library.textures.addNamed", { name: it.name })}
        onClick={() => void add(it)}
      >
        {busy === it.id ? (
          <Loader2 size={13} className="animate-spin" />
        ) : done ? (
          <Check size={13} />
        ) : (
          <Plus size={13} />
        )}
        {done ? t("library.textures.added") : t("library.textures.add")}
      </Button>
    );
  };

  return (
    <div className="min-h-0 flex-1 overflow-y-auto pr-1 pb-4">
      <p className="mb-3 text-xs text-muted">{t("library.textures.intro")}</p>
      <ul className="grid grid-cols-[repeat(auto-fill,minmax(128px,1fr))] gap-3">
        {items.map((it) => (
          <li
            key={it.id}
            className="flex flex-col overflow-hidden rounded-md border border-line bg-surface-2/70 transition-colors hover:border-accent/40"
          >
            <button
              type="button"
              onClick={() => setPreview(it)}
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
              {it.kind === "skin" && <Badge>{t(`skins.model.${it.model}`)}</Badge>}
            </div>
            <div className="p-2">{addButton(it)}</div>
          </li>
        ))}
      </ul>

      <Modal
        open={preview !== null}
        onClose={() => setPreview(null)}
        title={preview?.name ?? ""}
        footer={
          preview && (
            <>
              <Button variant="ghost" onClick={() => setPreview(null)}>
                {t("common.close")}
              </Button>
              {addButton(preview, "md")}
            </>
          )
        }
      >
        {preview && (
          <div className="h-80">
            <SkinViewer3D
              skin={preview.kind === "skin" ? preview.dataUri : figure}
              model={preview.model}
              cape={preview.kind === "cape" ? preview.dataUri : null}
              back="cape"
              pose="walk"
              name={null}
              className="h-full w-full"
            />
          </div>
        )}
      </Modal>
    </div>
  );
}
