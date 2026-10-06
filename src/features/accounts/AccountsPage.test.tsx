import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  listAccounts: vi.fn(),
  addOfflineAccount: vi.fn(),
  removeAccount: vi.fn(),
  selectAccount: vi.fn(),
  renameAccount: vi.fn(),
  accountAvatars: vi.fn(),
  pickPath: vi.fn(),
  setAccountAvatar: vi.fn(),
  clearAccountAvatar: vi.fn(),
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
    ipcMock.accountAvatars.mockResolvedValue({ photos: {}, defaultSkins: {}, nameConflicts: [] });
    useSkins.setState({ skins: [], capes: [], assignments: {}, loaded: true });
    useAccounts.setState({
      accounts: [acc("Steve"), acc("Alex")],
      selected: "offline-Steve",
      avatars: {},
    });
    useApp.setState({ view: "accounts" });
  });

  it("validates the name before adding and shows backend errors inline", async () => {
    render(<AccountsPage />);
    const input = screen.getByPlaceholderText("Player name");
    const add = screen.getByRole("button", { name: "Create" });
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
    fireEvent.click(screen.getByRole("button", { name: "Create" }));
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

  it("renames an account inline and shows a taken name as an error", async () => {
    render(<AccountsPage />);
    expect(screen.getByText("Create a new account")).toBeInTheDocument();
    expect(screen.queryByText(/offline/i)).toBeNull();

    fireEvent.click(screen.getAllByRole("button", { name: "Rename" })[0]!);
    const input = screen.getByRole("textbox", { name: "Rename" });
    expect(input).toHaveValue("Steve");
    fireEvent.change(input, { target: { value: "x" } });
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();

    ipcMock.renameAccount.mockRejectedValueOnce({
      code: "account.nameTaken",
      params: { name: "Alex" },
      detail: "",
    });
    fireEvent.change(input, { target: { value: "Alex" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(
      await screen.findByText("This username is taken. Please choose another one."),
    ).toBeInTheDocument();

    ipcMock.renameAccount.mockResolvedValueOnce({ ...acc("Mehbur"), id: "offline-Steve" });
    ipcMock.listAccounts.mockResolvedValue({
      accounts: [{ ...acc("Mehbur"), id: "offline-Steve" }, acc("Alex")],
      selected: "offline-Steve",
    });
    fireEvent.change(input, { target: { value: "Mehbur" } });
    expect(screen.getByText(/next PLAY/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(ipcMock.renameAccount).toHaveBeenLastCalledWith("offline-Steve", "Mehbur");
    expect(await screen.findByText("Mehbur")).toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "Rename" })).toBeNull();
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

  it("sets a photo with the + button and goes back to the skin head", async () => {
    const photo = "data:image/png;base64,UE5H";
    ipcMock.pickPath.mockResolvedValueOnce(null).mockResolvedValueOnce("C:/me.png");
    ipcMock.setAccountAvatar.mockResolvedValue(photo);
    ipcMock.clearAccountAvatar.mockResolvedValue(undefined);
    const { container } = render(<AccountsPage />);
    expect(container.querySelector("img")).toBeNull();
    const plus = screen.getByRole("button", { name: /profile photo for Steve/ });

    // Cancelled picker: nothing changes.
    fireEvent.click(plus);
    await vi.waitFor(() => expect(ipcMock.pickPath).toHaveBeenCalledTimes(1));
    expect(ipcMock.pickPath.mock.calls[0]?.[0]).toBe("avatar");
    expect(ipcMock.setAccountAvatar).not.toHaveBeenCalled();

    fireEvent.click(plus);
    await vi.waitFor(() => expect(container.querySelector("img")).toHaveAttribute("src", photo));
    expect(ipcMock.setAccountAvatar).toHaveBeenCalledWith("offline-Steve");

    fireEvent.click(screen.getByRole("button", { name: "Back to skin head" }));
    await vi.waitFor(() => expect(container.querySelector("img")).toBeNull());
    expect(ipcMock.clearAccountAvatar).toHaveBeenCalledWith("offline-Steve");
    expect(screen.queryByRole("button", { name: "Back to skin head" })).toBeNull();
  });

  it("shows the taken-name message in Turkish while the server checks", async () => {
    applyLanguage("tr");
    let reject: (e: unknown) => void = () => {};
    ipcMock.addOfflineAccount.mockReturnValue(
      new Promise((_, r) => {
        reject = r;
      }),
    );
    render(<AccountsPage />);
    fireEvent.change(screen.getByPlaceholderText("Oyuncu adı"), { target: { value: "Notch" } });
    fireEvent.click(screen.getByRole("button", { name: "Oluştur" }));
    const busy = await screen.findByRole("button", { name: "Kontrol ediliyor…" });
    expect(busy).toBeDisabled();
    reject({ code: "account.nameTaken", params: { name: "" }, detail: "" });
    expect(
      await screen.findByText("Bu kullanıcı adı alındı. Lütfen başka bir ad seçiniz."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Oluştur" })).toBeEnabled();
  });

  it("flags accounts whose name another user has and offers a rename", async () => {
    ipcMock.accountAvatars.mockResolvedValue({
      photos: {},
      defaultSkins: {},
      nameConflicts: ["offline-Alex"],
    });
    render(<AccountsPage />);
    const flag = await screen.findByRole("button", { name: /Another user has this name/ });
    fireEvent.click(flag);
    expect(screen.getByRole("textbox", { name: "Rename" })).toHaveValue("Alex");
  });
});
