/** `1.20.1-47.4.26` → `47.4.26`, `1.7.10-10.13.4.1614-1.7.10` → `10.13.4.1614`. */
export function forgeLabel(mc: string, version: string): string {
  let s = version.startsWith(`${mc}-`) ? version.slice(mc.length + 1) : version;
  if (s.endsWith(`-${mc}`)) s = s.slice(0, -(mc.length + 1));
  return s.replaceAll("_", " ");
}
