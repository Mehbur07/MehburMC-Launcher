import { ChevronDown, Sparkles } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import changelogRaw from "../../../CHANGELOG.md?raw";
import { useApp } from "../../stores/app";
import { type Block, type Release, parseChangelog, parseInline } from "./changelog";

function InlineText({ text }: { text: string }) {
  return (
    <>
      {parseInline(text).map((p, i) =>
        p.kind === "bold" ? (
          <strong key={i} className="font-semibold text-fg">
            {p.text}
          </strong>
        ) : p.kind === "code" ? (
          <code key={i} className="rounded bg-surface-3 px-1 py-0.5 font-mono text-[12px]">
            {p.text}
          </code>
        ) : (
          <span key={i}>{p.text}</span>
        ),
      )}
    </>
  );
}

function Blocks({ blocks }: { blocks: Block[] }) {
  // Group consecutive bullets into one list.
  const groups: (Block | Block[])[] = [];
  for (const b of blocks) {
    const last = groups[groups.length - 1];
    if (b.kind === "bullet" && Array.isArray(last)) last.push(b);
    else groups.push(b.kind === "bullet" ? [b] : b);
  }
  return (
    <div className="flex flex-col gap-3 text-sm leading-relaxed text-muted">
      {groups.map((g, i) =>
        Array.isArray(g) ? (
          <ul key={i} className="flex flex-col gap-2">
            {g.map((b, j) => (
              <li key={j} className="flex gap-2">
                <span className="mt-2 h-1.5 w-1.5 shrink-0 rounded-full bg-accent" />
                <span>
                  <InlineText text={b.text} />
                </span>
              </li>
            ))}
          </ul>
        ) : g.kind === "heading" ? (
          <h3 key={i} className="mt-1 font-display text-base font-semibold text-accent">
            <InlineText text={g.text} />
          </h3>
        ) : (
          <p key={i}>
            <InlineText text={g.text} />
          </p>
        ),
      )}
    </div>
  );
}

function ReleaseCard({ r, open, onToggle }: { r: Release; open: boolean; onToggle?: () => void }) {
  return (
    <section className="rounded-lg border border-line bg-surface-1/85 backdrop-blur">
      <button
        type="button"
        onClick={onToggle}
        disabled={!onToggle}
        aria-expanded={open}
        className="flex w-full items-center gap-3 px-5 py-4 text-left"
      >
        <span className="font-display text-xl font-bold">v{r.version}</span>
        {r.label && <span className="text-xs text-muted">{r.label}</span>}
        {onToggle && (
          <ChevronDown
            size={16}
            className={`ml-auto text-muted transition-transform ${open ? "rotate-180" : ""}`}
          />
        )}
      </button>
      {open && (
        <div className="border-t border-line px-5 py-4">
          <Blocks blocks={r.blocks} />
        </div>
      )}
    </section>
  );
}

/** Release notes of the running version, then older ones (collapsed). */
export function WhatsNewPage() {
  const { t } = useTranslation();
  const markSeen = useApp((s) => s.markWhatsNewSeen);
  const version = useApp((s) => s.boot?.version ?? "");
  const releases = useMemo(() => parseChangelog(changelogRaw), []);
  const current = releases.find((r) => r.version === version) ?? releases[0];
  const older = releases.filter((r) => r !== current);
  const [openOld, setOpenOld] = useState<string | null>(null);

  useEffect(() => markSeen(), [markSeen]);

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-5 pb-6">
      <div className="flex items-center gap-3">
        <Sparkles size={26} className="text-accent neon-drop" />
        <h1 className="font-display text-3xl font-bold tracking-wide">{t("whatsNew.title")}</h1>
      </div>
      <p className="-mt-3 text-sm text-muted">{t("whatsNew.running", { version })}</p>
      {current ? (
        <ReleaseCard r={current} open />
      ) : (
        <p className="text-sm text-muted">{t("whatsNew.empty")}</p>
      )}
      {older.length > 0 && (
        <>
          <h2 className="mt-2 font-display text-lg font-semibold">{t("whatsNew.older")}</h2>
          {older.map((r) => (
            <ReleaseCard
              key={r.version}
              r={r}
              open={openOld === r.version}
              onToggle={() => setOpenOld(openOld === r.version ? null : r.version)}
            />
          ))}
        </>
      )}
    </div>
  );
}
