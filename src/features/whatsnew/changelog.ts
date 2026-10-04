// Reads CHANGELOG.md (bundled at build time) into per-version sections.

export interface Block {
  kind: "heading" | "bullet" | "text";
  text: string;
}

export interface Release {
  version: string;
  /** Rest of the "## [x.y.z] — …" line, e.g. the date. */
  label: string;
  blocks: Block[];
}

/** Splits "## [x.y.z] — label" sections; text before the first is skipped. */
export function parseChangelog(md: string): Release[] {
  const releases: Release[] = [];
  let cur: Release | null = null;
  for (const raw of md.split(/\r?\n/)) {
    const line = raw.trimEnd();
    const head = /^## \[([^\]]+)\]\s*(?:[—-]\s*)?(.*)$/.exec(line);
    if (head) {
      cur = { version: head[1] ?? "", label: head[2] ?? "", blocks: [] };
      releases.push(cur);
      continue;
    }
    if (!cur || !line.trim()) continue;
    const h3 = /^###\s+(.*)$/.exec(line);
    const bullet = /^[-*]\s+(.*)$/.exec(line);
    const last = cur.blocks[cur.blocks.length - 1];
    if (h3) cur.blocks.push({ kind: "heading", text: h3[1] ?? "" });
    else if (bullet) cur.blocks.push({ kind: "bullet", text: bullet[1] ?? "" });
    // Indented continuation of a bullet (wrapped lines).
    else if (/^\s+/.test(raw) && last?.kind === "bullet") last.text += ` ${line.trim()}`;
    else if (last?.kind === "text") last.text += ` ${line.trim()}`;
    else cur.blocks.push({ kind: "text", text: line.trim() });
  }
  return releases;
}

export type Inline = { kind: "text" | "bold" | "code"; text: string };

/** **bold**, `code` and [links](url) (shown as their text only). */
export function parseInline(s: string): Inline[] {
  const out: Inline[] = [];
  const re = /\*\*(.+?)\*\*|`([^`]+)`|\[([^\]]+)\]\([^)]*\)/g;
  let at = 0;
  for (let m = re.exec(s); m; m = re.exec(s)) {
    if (m.index > at) out.push({ kind: "text", text: s.slice(at, m.index) });
    if (m[1] !== undefined) out.push({ kind: "bold", text: m[1] });
    else if (m[2] !== undefined) out.push({ kind: "code", text: m[2] });
    else out.push({ kind: "text", text: m[3] ?? "" });
    at = m.index + m[0].length;
  }
  if (at < s.length) out.push({ kind: "text", text: s.slice(at) });
  return out;
}
