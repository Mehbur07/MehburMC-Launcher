import { describe, expect, it } from "vitest";

import { forgeLabel } from "./loaderLabels";

describe("forgeLabel", () => {
  it("strips the Minecraft version", () => {
    expect(forgeLabel("1.20.1", "1.20.1-47.4.26")).toBe("47.4.26");
    expect(forgeLabel("1.7.10", "1.7.10-10.13.4.1614-1.7.10")).toBe("10.13.4.1614");
    expect(forgeLabel("26.3", "0.19.5")).toBe("0.19.5");
    expect(forgeLabel("1.20.1", "HD_U_I6")).toBe("HD U I6");
  });
});
