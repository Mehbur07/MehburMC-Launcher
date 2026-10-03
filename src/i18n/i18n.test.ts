import { describe, expect, it } from "vitest";

import en from "./en.json";
import { resolveLocale } from "./index";
import tr from "./tr.json";

function keys(obj: object, prefix = ""): string[] {
  return Object.entries(obj).flatMap(([k, v]) =>
    typeof v === "object" && v !== null ? keys(v, `${prefix}${k}.`) : [`${prefix}${k}`],
  );
}

describe("i18n", () => {
  it("tr and en define exactly the same keys", () => {
    expect(keys(tr).sort()).toEqual(keys(en).sort());
  });

  it("resolves the system language", () => {
    expect(resolveLocale("system", "tr-TR")).toBe("tr");
    expect(resolveLocale("system", "de-DE")).toBe("en");
    expect(resolveLocale("en", "tr-TR")).toBe("en");
    expect(resolveLocale("tr", "en-US")).toBe("tr");
  });
});
