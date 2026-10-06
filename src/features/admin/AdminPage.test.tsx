import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  adminLibrary: vi.fn(),
  adminScanMod: vi.fn(),
  adminReviewMod: vi.fn(),
  adminRemoveMod: vi.fn(),
  adminReports: vi.fn(),
  adminDismissReports: vi.fn(),
  adminHideTexture: vi.fn(),
  adminTextures: vi.fn(),
  adminRestoreTexture: vi.fn(),
  adminFindUsers: vi.fn(),
  adminBan: vi.fn(),
  adminUnban: vi.fn(),
  adminBans: vi.fn(),
  adminList: vi.fn(),
  adminSetRank: vi.fn(),
  adminPrivateTextures: vi.fn(),
  adminUploadPrivateTexture: vi.fn(),
  adminDeletePrivateTexture: vi.fn(),
  adminTextureGrants: vi.fn(),
  adminSetTextureGrant: vi.fn(),
  pickPath: vi.fn(),
}));
vi.mock("../../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../../lib/ipc")>()),
  ipc: ipcMock,
}));

import "../../i18n";
import { applyLanguage } from "../../i18n";
import type { AdminMod } from "../../lib/ipc/bindings/AdminMod";
import { useAuth } from "../../stores/auth";
import { AdminPage } from "./AdminPage";

const FRIEND = "22222222-2222-2222-2222-222222222222";

const pending = (): AdminMod => ({
  owner: FRIEND,
  reports: 0,
  mod: {
    id: 7,
    author: "Alex",
    name: "Cool Mod",
    description: "",
    modId: "coolmod",
    version: "1.0",
    loaders: ["fabric"],
    gameVersions: "",
    sha1: "a".repeat(40),
    size: 2048,
    fileName: "cool.jar",
    status: "pending",
    findings: [],
    reviewNote: null,
    createdAt: "2026-10-06T10:00:00Z",
    mine: false,
  },
});

const asRank = (rank: number) =>
  useAuth.setState({
    status: {
      signedIn: true,
      email: "a@b.co",
      anonymousIdentity: false,
      rank,
      ban: null,
      offline: false,
    },
  });

describe("AdminPage", () => {
  beforeEach(() => {
    applyLanguage("en");
    vi.clearAllMocks();
    ipcMock.adminLibrary.mockResolvedValue([pending()]);
    ipcMock.adminReports.mockResolvedValue([]);
    ipcMock.adminBans.mockResolvedValue([]);
    ipcMock.adminList.mockResolvedValue([]);
  });

  it("is empty without a rank", () => {
    asRank(0);
    const { container } = render(<AdminPage />);
    expect(container).toBeEmptyDOMElement();
  });

  it("rank 2 has no approval queue or admin management", async () => {
    asRank(2);
    render(<AdminPage />);
    const tabs = screen.getAllByRole("tab").map((t) => t.textContent);
    expect(tabs).toEqual(["Published mods", "Reports", "Bans"]);
    expect(ipcMock.adminTextures).not.toHaveBeenCalled();
    expect(ipcMock.adminLibrary).toHaveBeenCalledWith("approved");
  });

  it("approves only after this launcher's own scan", async () => {
    asRank(1);
    ipcMock.adminScanMod.mockResolvedValue({
      verdict: "warn",
      descriptor: null,
      findings: [{ severity: "warn", code: "network", examples: [] }],
      sha1: "a".repeat(40),
      size: 2048,
      classes: 3,
      scanner: 1,
    });
    ipcMock.adminReviewMod.mockResolvedValue(undefined);
    render(<AdminPage />);
    const row = (await screen.findByText("Cool Mod")).closest("li")!;
    const approve = within(row).getByRole("button", { name: /Approve/ });
    expect(approve).toBeDisabled();
    await act(async () =>
      fireEvent.click(within(row).getByRole("button", { name: /Download and scan/ })),
    );
    expect(ipcMock.adminScanMod).toHaveBeenCalledWith(7, "pending");
    expect(row).toHaveTextContent("Connects to the internet");
    await act(async () => fireEvent.click(approve));
    expect(ipcMock.adminReviewMod).toHaveBeenCalledWith(7, "pending", true, "");
  });

  it("rejects with a note", async () => {
    asRank(1);
    ipcMock.adminReviewMod.mockResolvedValue(undefined);
    render(<AdminPage />);
    const row = (await screen.findByText("Cool Mod")).closest("li")!;
    fireEvent.click(within(row).getByRole("button", { name: /Reject/ }));
    const dialog = await screen.findByRole("dialog", { name: 'Reject "Cool Mod"?' });
    fireEvent.change(within(dialog).getByRole("textbox"), { target: { value: "stolen" } });
    await act(async () => fireEvent.click(within(dialog).getByRole("button", { name: "Reject" })));
    expect(ipcMock.adminReviewMod).toHaveBeenCalledWith(7, "pending", false, "stolen");
  });

  it("finds a user and bans them for a week", async () => {
    asRank(2);
    ipcMock.adminFindUsers.mockResolvedValue([
      { userId: FRIEND, names: ["Griefer"], rank: 0, banned: false, banUntil: null },
    ]);
    ipcMock.adminBan.mockResolvedValue(undefined);
    render(<AdminPage />);
    fireEvent.click(screen.getByRole("tab", { name: "Bans" }));
    fireEvent.change(await screen.findByLabelText("Account name or friend code"), {
      target: { value: "grief" },
    });
    await act(async () => fireEvent.click(screen.getByRole("button", { name: /Search/ })));
    expect(ipcMock.adminFindUsers).toHaveBeenCalledWith("grief");
    const user = (await screen.findByText("Griefer")).closest("li")!;
    fireEvent.click(within(user).getByRole("button", { name: /Ban/ }));
    const dialog = await screen.findByRole("dialog", { name: "Ban Griefer?" });
    fireEvent.change(within(dialog).getByLabelText(/Reason/), { target: { value: "griefing" } });
    await act(async () => fireEvent.click(within(dialog).getByRole("button", { name: /Ban/ })));
    expect(ipcMock.adminBan).toHaveBeenCalledWith(FRIEND, 168, "griefing");
    expect(ipcMock.adminBans).toHaveBeenCalledTimes(2);
  });

  it("rank 1 sees texture reports and hides them", async () => {
    asRank(1);
    ipcMock.adminReports.mockResolvedValue([
      {
        kind: "texture",
        itemId: 3,
        name: "Bad Skin",
        author: "Alex",
        owner: FRIEND,
        sha1: "b".repeat(40),
        reports: 2,
        reasons: ["inappropriate"],
        notes: ["rude"],
        lastReport: "2026-10-06T10:00:00Z",
      },
    ]);
    ipcMock.adminHideTexture.mockResolvedValue(undefined);
    render(<AdminPage />);
    fireEvent.click(screen.getByRole("tab", { name: "Reports" }));
    const row = (await screen.findByText("Bad Skin")).closest("li")!;
    expect(row).toHaveTextContent("Inappropriate content");
    expect(row).toHaveTextContent("rude");
    await act(async () => fireEvent.click(within(row).getByRole("button", { name: /Hide/ })));
    expect(ipcMock.adminHideTexture).toHaveBeenCalledWith(3);
  });

  it("founder uploads a private texture and gives it to someone", async () => {
    asRank(1);
    ipcMock.adminPrivateTextures.mockResolvedValue([
      {
        id: 4,
        kind: "skin",
        model: "slim",
        name: "MehburMC",
        sha1: "c".repeat(40),
        createdAt: "2026-10-07T10:00:00Z",
        grants: 0,
        dataUri: "data:image/png;base64,QQ",
      },
    ]);
    ipcMock.pickPath.mockResolvedValue("C:/designs/mehbur.png");
    ipcMock.adminUploadPrivateTexture.mockResolvedValue(4);
    ipcMock.adminTextureGrants.mockResolvedValue([]);
    ipcMock.adminFindUsers.mockResolvedValue([
      { userId: FRIEND, names: ["Friend"], rank: 0, banned: false, banUntil: null },
    ]);
    ipcMock.adminSetTextureGrant.mockResolvedValue(undefined);
    render(<AdminPage />);
    fireEvent.click(screen.getByRole("tab", { name: "Private textures" }));

    await act(async () =>
      fireEvent.click(await screen.findByRole("button", { name: /Upload PNG/ })),
    );
    expect(ipcMock.pickPath).toHaveBeenCalledWith("privateTexture", undefined, expect.any(String));
    const dialog = await screen.findByRole("dialog", { name: "Upload a private texture" });
    fireEvent.click(within(dialog).getByRole("radio", { name: "Slim" }));
    await act(async () =>
      fireEvent.click(within(dialog).getByRole("button", { name: /Upload PNG/ })),
    );
    expect(ipcMock.adminUploadPrivateTexture).toHaveBeenCalledWith("skin", "slim", "MehburMC");

    const row = (await screen.findByText("MehburMC", { selector: "span" })).closest("li")!;
    fireEvent.click(within(row).getByRole("button", { name: /People/ }));
    fireEvent.change(await within(row).findByLabelText("Account name or friend code"), {
      target: { value: "frie" },
    });
    await act(async () => fireEvent.click(within(row).getByRole("button", { name: /Search/ })));
    await act(async () =>
      fireEvent.click(await within(row).findByRole("button", { name: /Give/ })),
    );
    expect(ipcMock.adminSetTextureGrant).toHaveBeenCalledWith(4, FRIEND, true);
  });

  it("founder removes any community share and can restore it", async () => {
    asRank(1);
    const share = (hidden: boolean) => ({
      id: 9,
      kind: "skin",
      model: "classic",
      name: "Rude Skin",
      author: "Alex",
      owner: FRIEND,
      visibility: "friends",
      createdAt: "2026-10-07T10:00:00Z",
      hidden,
      dataUri: null,
    });
    ipcMock.adminTextures.mockImplementation((hidden: boolean) => Promise.resolve([share(hidden)]));
    ipcMock.adminHideTexture.mockResolvedValue(undefined);
    ipcMock.adminRestoreTexture.mockResolvedValue(undefined);
    render(<AdminPage />);
    fireEvent.click(screen.getByRole("tab", { name: "Community" }));

    const row = (await screen.findByText("Rude Skin")).closest("li")!;
    expect(row).toHaveTextContent("Friends only");
    expect(ipcMock.adminTextures).toHaveBeenCalledWith(false);
    fireEvent.click(within(row).getByRole("button", { name: /Remove/ }));
    const dialog = await screen.findByRole("dialog");
    await act(async () => fireEvent.click(within(dialog).getByRole("button", { name: /Remove/ })));
    expect(ipcMock.adminHideTexture).toHaveBeenCalledWith(9);

    fireEvent.click(screen.getByRole("button", { name: "Removed" }));
    const gone = (await screen.findByText("Rude Skin")).closest("li")!;
    expect(ipcMock.adminTextures).toHaveBeenCalledWith(true);
    await act(async () => fireEvent.click(within(gone).getByRole("button", { name: /Restore/ })));
    expect(ipcMock.adminRestoreTexture).toHaveBeenCalledWith(9);
  });

  it("rank 2 has no private textures tab", () => {
    asRank(2);
    render(<AdminPage />);
    expect(screen.queryByRole("tab", { name: "Private textures" })).toBeNull();
  });
});
