import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  libraryMods: vi.fn(),
  pickPath: vi.fn(),
  libraryScanPick: vi.fn(),
  librarySubmit: vi.fn(),
  libraryInstall: vi.fn(),
  libraryWithdraw: vi.fn(),
  libraryReport: vi.fn(),
}));
vi.mock("../../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../../lib/ipc")>()),
  ipc: ipcMock,
}));

import "../../i18n";
import { applyLanguage } from "../../i18n";
import type { LibraryMod } from "../../lib/ipc/bindings/LibraryMod";
import type { LibraryScan } from "../../lib/ipc/bindings/LibraryScan";
import { useAccounts } from "../../stores/accounts";
import { instance } from "../../test/fixtures";
import { LibraryPanel, fitsLoader } from "./LibraryPanel";

const mod = (over: Partial<LibraryMod> = {}): LibraryMod => ({
  id: 1,
  author: "Alex",
  name: "Cool Mod",
  description: "Does cool things",
  modId: "coolmod",
  version: "1.2.0",
  loaders: ["fabric", "quilt"],
  gameVersions: ">=1.21",
  sha1: "a".repeat(40),
  size: 1000,
  fileName: "cool.jar",
  status: "approved",
  findings: [],
  reviewNote: null,
  createdAt: "2026-10-06T10:00:00Z",
  mine: false,
  ...over,
});

const scan = (verdict: "pass" | "warn" | "block"): LibraryScan => ({
  fileName: "cool.jar",
  report: {
    verdict,
    descriptor:
      verdict === "block"
        ? null
        : {
            modId: "coolmod",
            name: "Cool Mod",
            version: "1.2.0",
            loaders: ["fabric"],
            gameVersions: ">=1.21",
          },
    findings:
      verdict === "block"
        ? [{ severity: "block", code: "webhook", examples: ["a/Steal.class"] }]
        : [],
    sha1: "b".repeat(40),
    size: 2048,
    classes: 12,
    scanner: 1,
  },
});

describe("LibraryPanel", () => {
  beforeEach(() => {
    applyLanguage("en");
    vi.clearAllMocks();
    ipcMock.pickPath.mockResolvedValue("C:/mods/cool.jar");
    useAccounts.setState({
      accounts: [{ id: "acc", kind: "offline", name: "Mehbur", uuid: "u", addedAt: 1 }],
      selected: "acc",
    });
  });

  it("matches loaders like the Rust side", () => {
    expect(fitsLoader(["fabric"], "legacyFabric")).toBe(true);
    expect(fitsLoader(["neoForge"], "neoForge")).toBe(true);
    expect(fitsLoader(["fabric", "quilt"], "forge")).toBe(false);
    expect(fitsLoader(["fabric"], "vanilla")).toBe(false);
  });

  it("lists own uploads with status and shows an empty library", async () => {
    ipcMock.libraryMods.mockResolvedValue([
      mod({ id: 5, mine: true, status: "pending", name: "My Mod" }),
      mod({ id: 6, mine: true, status: "rejected", name: "Old", reviewNote: "Stolen" }),
    ]);
    render(<LibraryPanel inst={instance()} gameBusy={false} />);
    const mine = (await screen.findByText("My Mod")).closest("li")!;
    expect(mine).toHaveTextContent("Awaiting approval");
    expect(await screen.findByText("Admin note: Stolen")).toBeInTheDocument();
    expect(screen.getByText("No approved mods yet")).toBeInTheDocument();

    ipcMock.libraryWithdraw.mockResolvedValue(undefined);
    fireEvent.click(within(mine).getByRole("button", { name: /Withdraw/ }));
    const dialog = await screen.findByRole("dialog", { name: "Withdraw this mod?" });
    await act(async () =>
      fireEvent.click(within(dialog).getByRole("button", { name: "Withdraw" })),
    );
    expect(ipcMock.libraryWithdraw).toHaveBeenCalledWith(5);
    expect(ipcMock.libraryMods).toHaveBeenCalledTimes(2);
  });

  it("scans a picked jar and submits it for review", async () => {
    ipcMock.libraryMods.mockResolvedValue([]);
    ipcMock.libraryScanPick.mockResolvedValue(scan("pass"));
    ipcMock.librarySubmit.mockResolvedValue(9);
    render(<LibraryPanel inst={instance()} gameBusy={false} />);
    await act(async () => fireEvent.click(screen.getByRole("button", { name: /Upload mod/ })));
    expect(ipcMock.pickPath).toHaveBeenCalledWith("libraryMod", undefined, expect.any(String));
    const dialog = await screen.findByRole("dialog", { name: /Upload to the library/ });
    expect(dialog).toHaveTextContent("Check clean");
    expect(dialog).toHaveTextContent('Shown as uploaded by "Mehbur".');
    const name = within(dialog).getByRole("textbox", { name: "Name" });
    expect(name).toHaveValue("Cool Mod");
    fireEvent.change(name, { target: { value: "Cool Mod+" } });
    await act(async () =>
      fireEvent.click(within(dialog).getByRole("button", { name: /Submit for review/ })),
    );
    expect(ipcMock.librarySubmit).toHaveBeenCalledWith("Cool Mod+", "");
    expect(ipcMock.libraryMods).toHaveBeenCalledTimes(2);
  });

  it("explains a blocked jar and offers no submit", async () => {
    ipcMock.libraryMods.mockResolvedValue([]);
    ipcMock.libraryScanPick.mockResolvedValue(scan("block"));
    render(<LibraryPanel inst={instance()} gameBusy={false} />);
    await act(async () => fireEvent.click(screen.getByRole("button", { name: /Upload mod/ })));
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveTextContent("This file cannot be uploaded.");
    expect(dialog).toHaveTextContent("Discord webhook");
    expect(dialog).toHaveTextContent("a/Steal.class");
    expect(within(dialog).queryByRole("button", { name: /Submit/ })).toBeNull();
  });

  it("asks before installing a mod with warnings, then installs", async () => {
    ipcMock.libraryMods.mockResolvedValue([mod()]);
    ipcMock.libraryInstall
      .mockResolvedValueOnce({
        kind: "needsConfirm",
        findings: [{ severity: "warn", code: "network", examples: [] }],
      })
      .mockResolvedValueOnce({ kind: "installed", fileName: "cool.jar" });
    render(<LibraryPanel inst={instance({ id: "i1" })} gameBusy={false} />);
    const row = (await screen.findByText("Cool Mod")).closest("li")!;
    await act(async () => fireEvent.click(within(row).getByRole("button", { name: /Install/ })));
    expect(ipcMock.libraryInstall).toHaveBeenLastCalledWith(1, "i1", false);
    const dialog = await screen.findByRole("dialog", { name: 'Install "Cool Mod"?' });
    expect(dialog).toHaveTextContent("Connects to the internet");
    await act(async () =>
      fireEvent.click(within(dialog).getByRole("button", { name: "Install anyway" })),
    );
    expect(ipcMock.libraryInstall).toHaveBeenLastCalledWith(1, "i1", true);
    expect(await within(row).findByRole("button", { name: /Installed/ })).toBeDisabled();
  });

  it("disables install for the wrong loader or a running game", async () => {
    ipcMock.libraryMods.mockResolvedValue([mod()]);
    const { unmount } = render(
      <LibraryPanel
        inst={instance({ loader: { kind: "forge", version: "47" } })}
        gameBusy={false}
      />,
    );
    let row = (await screen.findByText("Cool Mod")).closest("li")!;
    expect(within(row).getByRole("button", { name: /Install/ })).toBeDisabled();
    unmount();
    render(<LibraryPanel inst={instance()} gameBusy />);
    row = (await screen.findByText("Cool Mod")).closest("li")!;
    expect(within(row).getByRole("button", { name: /Install/ })).toBeDisabled();
  });

  it("reports an approved mod", async () => {
    ipcMock.libraryMods.mockResolvedValue([mod()]);
    ipcMock.libraryReport.mockResolvedValue(undefined);
    render(<LibraryPanel inst={instance()} gameBusy={false} />);
    const row = (await screen.findByText("Cool Mod")).closest("li")!;
    fireEvent.click(within(row).getByRole("button", { name: "Report" }));
    const dialog = await screen.findByRole("dialog", { name: 'Report "Cool Mod"' });
    fireEvent.click(within(dialog).getByRole("radio", { name: /Broken/ }));
    await act(async () => fireEvent.click(within(dialog).getByRole("button", { name: "Report" })));
    expect(ipcMock.libraryReport).toHaveBeenCalledWith(1, "broken", "");
  });
});
