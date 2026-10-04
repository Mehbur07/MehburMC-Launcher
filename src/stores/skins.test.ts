import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  listSkins: vi.fn(),
  importSkinFile: vi.fn(),
  importPlayerSkin: vi.fn(),
  assignSkin: vi.fn(),
}));
vi.mock("../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../lib/ipc")>()),
  ipc: ipcMock,
}));

import type { LibraryView } from "../lib/ipc/bindings/LibraryView";
import { useApp } from "./app";
import { accountCape, accountSkin, useSkins } from "./skins";

const lib: LibraryView = {
  skins: [{ id: "s1", name: "Steve", model: "classic", addedAt: 1, dataUri: "data:a" }],
  capes: [{ id: "c1", name: "Cape", addedAt: 1, dataUri: "data:b" }],
  assignments: { acc: { skin: "s1", cape: "c1" }, other: { skin: "gone" } },
};

describe("skins store", () => {
  beforeEach(() => {
    useSkins.setState({ skins: [], capes: [], assignments: {}, loaded: false });
    useApp.setState({ notice: null });
    vi.clearAllMocks();
    ipcMock.listSkins.mockResolvedValue(lib);
  });

  it("resolves an account's skin and cape, ignoring stale ids", async () => {
    await useSkins.getState().load();
    const s = useSkins.getState();
    expect(s.loaded).toBe(true);
    expect(accountSkin(s, "acc")?.name).toBe("Steve");
    expect(accountCape(s, "acc")?.name).toBe("Cape");
    expect(accountSkin(s, "other")).toBeNull();
    expect(accountSkin(s, null)).toBeNull();
  });

  it("imports the picked file and reloads; failures become a notice", async () => {
    ipcMock.importSkinFile.mockResolvedValueOnce("s1");
    expect(await useSkins.getState().importFile("skin")).toBe("s1");
    expect(ipcMock.importSkinFile).toHaveBeenCalledWith("skin");
    expect(ipcMock.listSkins).toHaveBeenCalled();

    ipcMock.importSkinFile.mockRejectedValueOnce({
      code: "skin.invalid",
      params: { reason: "bad size" },
      detail: "invalid",
    });
    expect(await useSkins.getState().importFile("cape")).toBeNull();
    expect(useApp.getState().notice?.code).toBe("skin.invalid");
  });

  it("returns player import errors instead of showing them", async () => {
    const err = { code: "skin.playerNotFound", params: { name: "x" }, detail: "" };
    ipcMock.importPlayerSkin.mockRejectedValueOnce(err);
    expect(await useSkins.getState().importPlayer("x")).toEqual(err);
    expect(useApp.getState().notice).toBeNull();
  });
});
