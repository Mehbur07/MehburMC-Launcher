import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  listAccounts: vi.fn(),
  addOfflineAccount: vi.fn(),
  removeAccount: vi.fn(),
  selectAccount: vi.fn(),
}));
vi.mock("../../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../../lib/ipc")>()),
  ipc: ipcMock,
}));

import "../../i18n";
import { applyLanguage } from "../../i18n";
import type { Account } from "../../lib/ipc/bindings/Account";
import { useAccounts } from "../../stores/accounts";
import { useApp } from "../../stores/app";
import { useSkins } from "../../stores/skins";
import { AccountsPage } from "./AccountsPage";

const acc = (name: string): Account => ({
  id: `offline-${name}`,
  kind: "offline",
  name,
  uuid: `uuid-${name}`,
  addedAt: 1,
});

describe("AccountsPage", () => {
  beforeEach(() => {
    applyLanguage("en");
    vi.clearAllMocks();
    useSkins.setState({ skins: [], capes: [], assignments: {}, loaded: true });
    useAccounts.setState({ accounts: [acc("Steve"), acc("Alex")], selected: "offline-Steve" });
    useApp.setState({ view: "accounts" });
  });

  it("validates the name before adding and shows backend errors inline", async () => {
    render(<AccountsPage />);
    const input = screen.getByPlaceholderText("Player name");
    const add = screen.getByRole("button", { name: "Add" });
    fireEvent.change(input, { target: { value: "ab" } });
    expect(add).toBeDisabled();

    ipcMock.addOfflineAccount.mockRejectedValue({
      code: "auth.invalidName",
      params: { name: "Bad_Name" },
      detail: "",
    });
    fireEvent.change(input, { target: { value: "Bad_Name" } });
    fireEvent.click(add);
    expect(await screen.findByText(/Bad_Name/)).toBeInTheDocument();
  });

  it("adds a valid account and clears the field", async () => {
    ipcMock.addOfflineAccount.mockResolvedValue(acc("Notch"));
    ipcMock.listAccounts.mockResolvedValue({
      accounts: [acc("Steve"), acc("Alex"), acc("Notch")],
      selected: "offline-Notch",
    });
    render(<AccountsPage />);
    const input = screen.getByPlaceholderText("Player name") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "Notch" } });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(await screen.findByText("Notch")).toBeInTheDocument();
    expect(input.value).toBe("");
  });

  it("switches the active account and jumps to the skin editor", async () => {
    ipcMock.selectAccount.mockResolvedValue({
      accounts: [acc("Steve"), acc("Alex")],
      selected: "offline-Alex",
    });
    render(<AccountsPage />);
    fireEvent.click(screen.getByRole("button", { name: "Use" }));
    expect(ipcMock.selectAccount).toHaveBeenCalledWith("offline-Alex");
    await vi.waitFor(() => expect(useAccounts.getState().selected).toBe("offline-Alex"));

    fireEvent.click(screen.getAllByRole("button", { name: "Edit skin" })[0]!);
    expect(useApp.getState().view).toBe("skins");
  });

  it("asks before removing an account", async () => {
    ipcMock.removeAccount.mockResolvedValue({ accounts: [acc("Alex")], selected: "offline-Alex" });
    render(<AccountsPage />);
    fireEvent.click(screen.getAllByRole("button", { name: "Delete" })[0]!);
    expect(ipcMock.removeAccount).not.toHaveBeenCalled();
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(dialog.querySelector("footer button:last-child")!);
    expect(ipcMock.removeAccount).toHaveBeenCalledWith("offline-Steve");
  });
});
