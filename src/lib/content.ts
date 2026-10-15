const DISABLED = ".disabled";

/** `a.jar.disabled` → `a.jar`. */
export const baseName = (name: string) =>
  name.endsWith(DISABLED) ? name.slice(0, -DISABLED.length) : name;

/** The name a file gets after being switched on/off. */
export const toggledName = (name: string) =>
  name.endsWith(DISABLED) ? baseName(name) : name + DISABLED;

/**
 * React keys that survive a toggle (`a.jar` ↔ `a.jar.disabled`), so the row
 * stays mounted, keeps focus and its switch animates. When both copies exist
 * the full names keep the keys unique.
 */
export function stableKeys(names: string[]): (name: string) => string {
  const all = new Set(names);
  return (name) => {
    const base = baseName(name);
    return all.has(base) && all.has(base + DISABLED) ? name : base;
  };
}
