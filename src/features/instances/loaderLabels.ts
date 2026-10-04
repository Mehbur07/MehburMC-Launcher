import { useTranslation } from "react-i18next";

import type { Instance } from "../../lib/ipc/bindings/Instance";

/** `1.20.1-47.4.26` → `47.4.26`, `1.7.10-10.13.4.1614-1.7.10` → `10.13.4.1614`. */
export function forgeLabel(mc: string, version: string): string {
  let s = version.startsWith(`${mc}-`) ? version.slice(mc.length + 1) : version;
  if (s.endsWith(`-${mc}`)) s = s.slice(0, -(mc.length + 1));
  return s.replaceAll("_", " ");
}

export function loaderLabel(kind: Instance["loader"]["kind"]) {
  return {
    vanilla: "Vanilla",
    fabric: "Fabric",
    quilt: "Quilt",
    legacyFabric: "Legacy Fabric",
    forge: "Forge",
    neoForge: "NeoForge",
    optifine: "OptiFine",
  }[kind];
}

export function usePlayTimeUnits() {
  const { t } = useTranslation();
  return { h: t("units.h"), m: t("units.m"), s: t("units.s") };
}
