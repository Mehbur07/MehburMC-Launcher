import { Package } from "lucide-react";
import { useEffect, useState } from "react";

import { ipc } from "../lib/ipc";

// Icons are fetched by the backend (CSP blocks remote images) and kept for
// the session; failures are remembered so they are not retried per render.
const cache = new Map<string, string | null>();
const pending = new Map<string, Promise<string | null>>();

function load(url: string): Promise<string | null> {
  const hit = cache.get(url);
  if (hit !== undefined) return Promise.resolve(hit);
  let p = pending.get(url);
  if (!p) {
    p = ipc
      .contentIcon(url)
      .catch(() => null)
      .then((uri) => {
        cache.set(url, uri);
        pending.delete(url);
        return uri;
      });
    pending.set(url, p);
  }
  return p;
}

export function ProjectIcon({ url, size = 44 }: { url?: string | null; size?: number }) {
  const [src, setSrc] = useState<string | null>(url ? (cache.get(url) ?? null) : null);

  useEffect(() => {
    if (!url) return;
    let alive = true;
    void load(url).then((uri) => alive && setSrc(uri));
    return () => {
      alive = false;
    };
  }, [url]);

  return (
    <div
      className="grid shrink-0 place-items-center overflow-hidden rounded-md border border-line bg-surface-3"
      style={{ width: size, height: size }}
    >
      {src ? (
        <img src={src} alt="" width={size} height={size} className="h-full w-full object-cover" />
      ) : (
        <Package size={size * 0.45} className="text-muted" />
      )}
    </div>
  );
}
