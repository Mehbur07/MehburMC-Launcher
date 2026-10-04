import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  checkAppUpdate: vi.fn(),
  installAppUpdate: vi.fn(),
}));
const progress = vi.hoisted(() => ({ cb: null as null | ((p: [number, number]) => void) }));
vi.mock("../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../lib/ipc")>()),
  ipc: ipcMock,
  onUpdateProgress: vi.fn(async (cb: (p: [number, number]) => void) => {
    progress.cb = cb;
    return () => {
      progress.cb = null;
    };
  }),
}));

import { useApp } from "./app";
import { useUpdate } from "./update";

describe("update store", () => {
  beforeEach(() => {
    useUpdate.setState({ info: null, checking: false, installing: null, dismissed: false });
    useApp.setState({ notice: null });
    vi.clearAllMocks();
  });

  it("stores the check result and resets dismissal", async () => {
    useUpdate.setState({ dismissed: true });
    ipcMock.checkAppUpdate.mockResolvedValue({
      status: "available",
      current: "0.1.0",
      version: "0.2.0",
      notes: null,
    });
    await useUpdate.getState().check();
    expect(useUpdate.getState()).toMatchObject({
      checking: false,
      dismissed: false,
      info: { status: "available", version: "0.2.0" },
    });
  });

  it("tracks download progress and reports install failures", async () => {
    let fail: (e: unknown) => void = () => {};
    ipcMock.installAppUpdate.mockReturnValue(new Promise((_, rej) => (fail = rej)));
    const run = useUpdate.getState().install();
    await vi.waitFor(() => expect(progress.cb).not.toBeNull());
    progress.cb!([50, 200]);
    expect(useUpdate.getState().installing).toBe(0.25);

    fail({ code: "loader.installFailed", params: {}, detail: "signature mismatch" });
    await run;
    expect(useUpdate.getState().installing).toBeNull();
    expect(useApp.getState().notice?.detail).toBe("signature mismatch");
    expect(progress.cb).toBeNull();
  });
});
