import { ExternalLink, Newspaper } from "lucide-react";
import { motion } from "motion/react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { create } from "zustand";

import { ipc, toErrorPayload } from "../../lib/ipc";
import type { NewsItem } from "../../lib/ipc/bindings/NewsItem";
import { useApp } from "../../stores/app";

/** Loaded once per session; the backend caches the feed for an hour. */
const useNews = create<{ items: NewsItem[] | null; load: () => void }>((set, get) => ({
  items: null,
  load: () => {
    if (get().items) return;
    set({ items: [] });
    void ipc
      .listNews()
      .then((items) => set({ items }))
      .catch(() => set({ items: [] }));
  },
}));

function NewsImage({ url }: { url?: string }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    if (!url) return;
    let alive = true;
    // Remote images are fetched by Rust (CSP) and returned as data URIs.
    void ipc
      .contentIcon(url)
      .then((s) => alive && setSrc(s))
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [url]);
  return (
    <div className="aspect-[16/9] w-full overflow-hidden bg-gradient-to-br from-accent/15 via-surface-2 to-surface-3">
      {src && (
        <motion.img
          src={src}
          alt=""
          initial={{ opacity: 0, scale: 1.04 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ duration: 0.35 }}
          className="h-full w-full object-cover transition-transform duration-500 group-hover:scale-105"
        />
      )}
    </div>
  );
}

export function NewsFeed({ limit = 6 }: { limit?: number }) {
  const { t, i18n } = useTranslation();
  const items = useNews((s) => s.items);
  const load = useNews((s) => s.load);
  useEffect(load, [load]);

  if (!items || items.length === 0) return null;

  const open = (link?: string) => {
    if (!link) return;
    ipc.openExternal(link).catch((e) => useApp.setState({ notice: toErrorPayload(e) }));
  };

  return (
    <section className="flex flex-col gap-3">
      <h2 className="flex items-center gap-2 font-display text-xs font-semibold tracking-[0.2em] text-muted uppercase">
        <Newspaper size={14} />
        {t("home.news")}
      </h2>
      <div className="grid grid-cols-[repeat(auto-fill,minmax(230px,1fr))] gap-3">
        {items.slice(0, limit).map((n, i) => (
          <motion.button
            key={n.id}
            type="button"
            onClick={() => open(n.link)}
            disabled={!n.link}
            initial={{ opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: 0.04 * i, duration: 0.22, ease: "easeOut" }}
            className="group flex flex-col overflow-hidden rounded-lg border border-line bg-surface-1/85 text-left backdrop-blur transition-colors hover:border-accent/50 disabled:cursor-default"
          >
            <NewsImage url={n.image} />
            <div className="flex flex-1 flex-col gap-1 p-3">
              <div className="flex items-center justify-between text-[11px] text-muted">
                <span>
                  {n.date
                    ? new Date(n.date).toLocaleDateString(i18n.language, { dateStyle: "medium" })
                    : ""}
                </span>
                {n.link && (
                  <ExternalLink
                    size={12}
                    className="opacity-0 transition-opacity group-hover:opacity-100"
                  />
                )}
              </div>
              <div className="line-clamp-2 font-display text-[15px] leading-snug font-semibold group-hover:text-accent">
                {n.title}
              </div>
              {n.text && <p className="line-clamp-2 text-xs text-muted">{n.text}</p>}
            </div>
          </motion.button>
        ))}
      </div>
    </section>
  );
}
