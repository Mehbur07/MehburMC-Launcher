import { describe, expect, it } from "vitest";

import { formatBytes, formatDuration } from "./format";

describe("format", () => {
  it("bytes", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(483_782_790)).toBe("461.4 MB");
  });

  it("durations", () => {
    expect(formatDuration(42)).toBe("42 s");
    expect(formatDuration(388)).toBe("6 m");
    expect(formatDuration(7500, { h: "sa", m: "dk", s: "sn" })).toBe("2 sa 5 dk");
  });
});
