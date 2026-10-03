import { useVirtualizer } from "@tanstack/react-virtual";
import { Check, RefreshCw, Search } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { Badge, Button, TextInput } from "../../components/ui";
import { toErrorPayload } from "../../lib/ipc";
import type { ManifestEntry } from "../../lib/ipc/bindings/ManifestEntry";
import { useInstances } from "../../stores/instances";

const TYPES = ["release", "snapshot", "old_beta", "old_alpha"] as const;
type VType = (typeof TYPES)[number];

export function filterVersions(all: ManifestEntry[], types: Set<string>, query: string) {
  const q = query.trim().toLowerCase();
  return all.filter((v) => types.has(v.type) && (!q || v.id.toLowerCase().includes(q)));
}

export function VersionPicker({
  value,
  onChange,
}: {
  value: string | null;
  onChange: (id: string) => void;
}) {
  const { t, i18n } = useTranslation();
  const loadVersions = useInstances((s) => s.loadVersions);
  const [all, setAll] = useState<ManifestEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [types, setTypes] = useState<Set<VType>>(new Set(["release"]));
  const [query, setQuery] = useState("");
  const scroller = useRef<HTMLDivElement>(null);

  const load = () => {
    setError(null);
    loadVersions()
      .then((v) => {
        setAll(v);
        if (!value) {
          const latest = v.find((x) => x.type === "release");
          if (latest) onChange(latest.id);
        }
      })
      .catch((e) => setError(toErrorPayload(e).code));
  };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(load, []);

  const list = useMemo(() => filterVersions(all ?? [], types, query), [all, types, query]);
  const virtual = useVirtualizer({
    count: list.length,
    getScrollElement: () => scroller.current,
    estimateSize: () => 38,
    overscan: 12,
  });

  const toggle = (k: VType) =>
    setTypes((prev) => {
      const next = new Set(prev);
      if (next.has(k)) next.delete(k);
      else next.add(k);
      return next.size ? next : prev;
    });

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="relative">
        <Search
          size={15}
          className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-muted"
        />
        <TextInput
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t("wizard.searchVersion")}
          className="pl-9"
          aria-label={t("wizard.searchVersion")}
        />
      </div>
      <div className="flex flex-wrap gap-1.5">
        {TYPES.map((k) => (
          <button
            key={k}
            type="button"
            aria-pressed={types.has(k)}
            onClick={() => toggle(k)}
            className={`rounded-full border px-2.5 py-1 text-xs transition-colors ${
              types.has(k)
                ? "border-accent/60 bg-accent/15 text-accent"
                : "border-line text-muted hover:text-fg"
            }`}
          >
            {t(`versionType.${k}`)}
          </button>
        ))}
      </div>

      <div
        ref={scroller}
        className="min-h-0 flex-1 overflow-y-auto rounded-md border border-line bg-bg/40"
      >
        {error && (
          <div className="flex flex-col items-center gap-3 p-6 text-center text-sm text-muted">
            {t(`errors.${error}`, { defaultValue: t("errors.unknown") })}
            <Button size="sm" onClick={load}>
              <RefreshCw size={14} />
              {t("common.retry")}
            </Button>
          </div>
        )}
        {!all && !error && (
          <div className="p-6 text-center text-sm text-muted">{t("common.loading")}</div>
        )}
        <div style={{ height: virtual.getTotalSize(), position: "relative" }}>
          {virtual.getVirtualItems().map((row) => {
            const v = list[row.index]!;
            const active = v.id === value;
            return (
              <button
                key={v.id}
                type="button"
                onClick={() => onChange(v.id)}
                aria-selected={active}
                className={`absolute inset-x-0 flex items-center justify-between px-3 text-left text-sm transition-colors ${
                  active ? "bg-accent/15 text-accent" : "hover:bg-surface-2"
                }`}
                style={{ top: row.start, height: row.size }}
              >
                <span className="flex items-center gap-2 font-medium">
                  {active && <Check size={14} />}
                  {v.id}
                </span>
                <span className="flex items-center gap-2 text-xs text-muted">
                  {v.type !== "release" && <Badge tone="warn">{t(`versionType.${v.type}`)}</Badge>}
                  {new Date(v.releaseTime).toLocaleDateString(i18n.language)}
                </span>
              </button>
            );
          })}
        </div>
      </div>
      <div className="text-right text-xs text-muted">
        {t("wizard.versionCount", { count: list.length })}
      </div>
    </div>
  );
}
