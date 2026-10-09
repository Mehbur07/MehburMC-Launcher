import { Loader2, RefreshCw, Search, X } from "lucide-react";
import { type ReactNode, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button, IconButton, TextInput } from "../../components/ui";
import type { SkinMcSort } from "../../lib/ipc/bindings/SkinMcSort";
import { useSkinMc } from "../../stores/skinmc";
import type { LooseTexture } from "./PresetsPanel";

const SORTS: SkinMcSort[] = ["latest", "today", "week", "month", "random"];

/** Skins from skinmc.net, one page at a time (K78). */
export function SkinMcSection({ grid }: { grid: (items: LooseTexture[]) => ReactNode }) {
  const { t } = useTranslation();
  const { sort, tag, items, next, loading, error, open, more } = useSkinMc();
  const [draft, setDraft] = useState(tag);

  // First visit: the newest skins.
  useEffect(() => {
    const s = useSkinMc.getState();
    if (s.items.length === 0 && !s.loading && !s.error) void s.open("latest", "");
  }, []);

  const textures: LooseTexture[] = items.map((s) => ({
    key: `skinmc:${s.id}`,
    kind: "skin",
    name: s.author ? t("skins.skinmc.by", { name: s.author }) : t("skins.skinmc.unnamed"),
    model: s.model,
    dataUri: s.dataUri,
  }));

  const search = () => void open(sort, draft);

  return (
    <section className="flex flex-col gap-2">
      <p className="text-xs text-muted">{t("skins.skinmc.hint")}</p>
      <div className="flex flex-wrap items-center gap-1.5">
        {SORTS.map((s) => (
          <Button
            key={s}
            size="sm"
            variant={!tag && sort === s ? "primary" : "ghost"}
            aria-pressed={!tag && sort === s}
            onClick={() => {
              setDraft("");
              void open(s, "");
            }}
          >
            {t(`skins.skinmc.sort.${s}`)}
          </Button>
        ))}
        <IconButton
          label={t("common.refresh")}
          className="h-7 w-7"
          disabled={loading}
          onClick={() => void open(sort, tag)}
        >
          <RefreshCw size={13} className={loading ? "animate-spin" : ""} />
        </IconButton>
      </div>
      <form
        className="flex items-center gap-1.5"
        onSubmit={(e) => {
          e.preventDefault();
          search();
        }}
      >
        <TextInput
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          placeholder={t("skins.skinmc.tagPlaceholder")}
          aria-label={t("skins.skinmc.tag")}
          className="max-w-64"
        />
        <Button size="sm" type="submit" disabled={!draft.trim()}>
          <Search size={13} />
          {t("skins.skinmc.search")}
        </Button>
        {tag && (
          <IconButton
            label={t("skins.skinmc.clearTag")}
            className="h-7 w-7"
            onClick={() => {
              setDraft("");
              void open(sort, "");
            }}
          >
            <X size={13} />
          </IconButton>
        )}
      </form>
      {tag && <p className="text-xs text-muted">{t("skins.skinmc.tagged", { tag })}</p>}
      {error && (
        <div role="alert" className="flex items-center gap-2 text-xs text-danger">
          {t(`errors.${error.code}`, {
            ...error.params,
            defaultValue: t("skins.skinmc.unavailable"),
          })}
          <Button size="sm" onClick={() => void (items.length ? more() : open(sort, tag))}>
            {t("common.retry")}
          </Button>
        </div>
      )}
      {items.length > 0 && grid(textures)}
      {!loading && !error && items.length === 0 && (
        <p className="text-xs text-muted">{t("skins.skinmc.empty")}</p>
      )}
      {loading && (
        <div className="flex justify-center py-3">
          <Loader2 className="animate-spin text-accent" aria-label={t("common.loading")} />
        </div>
      )}
      {!loading && next && items.length > 0 && (
        <Button size="sm" variant="ghost" className="self-start" onClick={() => void more()}>
          {t("skins.skinmc.more")}
        </Button>
      )}
    </section>
  );
}
