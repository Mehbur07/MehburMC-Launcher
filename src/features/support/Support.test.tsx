import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  supportThreads: vi.fn(),
  supportOpen: vi.fn(),
  supportSend: vi.fn(),
  supportMessages: vi.fn(),
  supportMarkRead: vi.fn(),
  supportSetOpen: vi.fn(),
  supportUnread: vi.fn(),
  adminSupportThreads: vi.fn(),
}));
vi.mock("../../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../../lib/ipc")>()),
  ipc: ipcMock,
}));

import "../../i18n";
import { applyLanguage } from "../../i18n";
import type { AdminSupportThread } from "../../lib/ipc/bindings/AdminSupportThread";
import type { SupportMessage } from "../../lib/ipc/bindings/SupportMessage";
import type { SupportThread } from "../../lib/ipc/bindings/SupportThread";
import { useSupport } from "../../stores/support";
import { AdminMessages } from "./AdminMessages";
import { SupportPage } from "./SupportPanel";

const thread = (over: Partial<SupportThread> = {}): SupportThread => ({
  id: 7,
  subject: "Ban appeal",
  open: true,
  createdAt: "2026-11-10T10:00:00Z",
  updatedAt: "2026-11-10T11:00:00Z",
  unread: 1,
  ...over,
});
const message = (over: Partial<SupportMessage>): SupportMessage => ({
  id: 1,
  fromAdmin: false,
  author: null,
  body: "",
  createdAt: "2026-11-10T10:00:00Z",
  mine: false,
  ...over,
});
const adminThread = (over: Partial<AdminSupportThread> = {}): AdminSupportThread => ({
  id: 7,
  owner: "u-1",
  names: ["Griefer"],
  banned: false,
  subject: "Help",
  open: true,
  createdAt: "2026-11-10T10:00:00Z",
  updatedAt: "2026-11-10T11:00:00Z",
  unread: 1,
  lastFromAdmin: false,
  preview: "you are all noobs",
  ...over,
});

describe("Contact admins", () => {
  beforeEach(() => {
    applyLanguage("en");
    vi.clearAllMocks();
    useSupport.setState({
      unread: { mine: 0, admin: 0 },
      threads: null,
      adminThreads: { open: null, closed: null },
      messages: {},
      error: null,
    });
    ipcMock.supportUnread.mockResolvedValue({ mine: 0, admin: 0 });
    ipcMock.supportMarkRead.mockResolvedValue(undefined);
  });

  it("opens a topic and shows it", async () => {
    ipcMock.supportThreads.mockResolvedValueOnce([]).mockResolvedValue([thread({ unread: 0 })]);
    ipcMock.supportOpen.mockResolvedValue(7);
    ipcMock.supportMessages.mockResolvedValue([
      message({ id: 1, body: "Please unban me", mine: true }),
    ]);
    render(<SupportPage />);
    expect(await screen.findByText("You have not opened a topic yet.")).toBeInTheDocument();

    const send = screen.getByRole("button", { name: "Send" });
    expect(send).toBeDisabled();
    fireEvent.change(screen.getByPlaceholderText("What is it about, in short?"), {
      target: { value: "Ban appeal" },
    });
    fireEvent.change(screen.getByPlaceholderText("Tell us what happened…"), {
      target: { value: "Please unban me" },
    });
    fireEvent.click(send);

    await waitFor(() =>
      expect(ipcMock.supportOpen).toHaveBeenCalledWith("Ban appeal", "Please unban me"),
    );
    expect(await screen.findByRole("log", { name: "Ban appeal" })).toBeInTheDocument();
    expect(await screen.findByText("Please unban me")).toBeInTheDocument();
  });

  it("labels replies as the admin team and marks them read", async () => {
    ipcMock.supportThreads.mockResolvedValue([thread()]);
    ipcMock.supportMessages.mockResolvedValue([
      message({ id: 1, body: "Hi", mine: true }),
      message({ id: 2, body: "We looked at it", fromAdmin: true }),
    ]);
    render(<SupportPage />);
    fireEvent.click(await screen.findByRole("button", { name: /Ban appeal/ }));

    const log = await screen.findByRole("log", { name: "Ban appeal" });
    expect(await within(log).findByText("We looked at it")).toBeInTheDocument();
    expect(within(log).getByText("MehburMC admins")).toBeInTheDocument();
    await waitFor(() => expect(ipcMock.supportMarkRead).toHaveBeenCalledWith(7));
  });

  it("shows a note instead of the composer on a closed topic", async () => {
    ipcMock.supportThreads.mockResolvedValue([thread({ open: false, unread: 0 })]);
    ipcMock.supportMessages.mockResolvedValue([message({ id: 1, body: "Old", mine: true })]);
    render(<SupportPage />);
    fireEvent.click(await screen.findByRole("button", { name: /Ban appeal/ }));

    expect(
      await screen.findByText("This topic is closed. Open a new topic for a new problem."),
    ).toBeInTheDocument();
    expect(screen.queryByPlaceholderText(/Enter sends/)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Close topic" })).not.toBeInTheDocument();
  });

  it("shows server errors in plain words", async () => {
    ipcMock.supportThreads.mockResolvedValue([]);
    ipcMock.supportOpen.mockRejectedValue({
      code: "support.tooManyOpen",
      detail: "support.tooManyOpen",
      params: {},
    });
    render(<SupportPage />);
    fireEvent.change(await screen.findByPlaceholderText("What is it about, in short?"), {
      target: { value: "Again" },
    });
    fireEvent.change(screen.getByPlaceholderText("Tell us what happened…"), {
      target: { value: "Hello" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    expect(
      await screen.findByText(
        "You can have at most 5 open topics. Close an old one and try again.",
      ),
    ).toBeInTheDocument();
  });

  it("lets admins see who wrote and ban them", async () => {
    const onBan = vi.fn();
    ipcMock.adminSupportThreads.mockResolvedValue([adminThread()]);
    ipcMock.supportMessages.mockResolvedValue([
      message({ id: 1, body: "you are all noobs" }),
      message({ id: 2, body: "Please be kind", fromAdmin: true, author: "Mehbur", mine: true }),
    ]);
    render(<AdminMessages onBan={onBan} version={0} />);
    fireEvent.click(await screen.findByRole("button", { name: /Griefer/ }));

    const log = await screen.findByRole("log", { name: "Help" });
    // The player's account name labels their messages.
    expect(await within(log).findByText("Griefer")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Ban" }));
    expect(onBan).toHaveBeenCalledWith({ userId: "u-1", label: "Griefer" });
  });

  it("lets admins close and reopen topics", async () => {
    ipcMock.adminSupportThreads.mockImplementation(async (open: boolean) =>
      open ? [adminThread()] : [adminThread({ id: 8, subject: "Done", open: false, banned: true })],
    );
    ipcMock.supportMessages.mockResolvedValue([]);
    ipcMock.supportSetOpen.mockResolvedValue(undefined);
    render(<AdminMessages onBan={vi.fn()} version={0} />);
    fireEvent.click(await screen.findByRole("button", { name: /Griefer/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Close" }));
    await waitFor(() => expect(ipcMock.supportSetOpen).toHaveBeenCalledWith(7, false));

    fireEvent.click(screen.getByRole("button", { name: "Closed" }));
    fireEvent.click(await screen.findByRole("button", { name: /Done/ }));
    // A banned player gets no second ban button.
    expect(screen.queryByRole("button", { name: "Ban" })).not.toBeInTheDocument();
    fireEvent.click(await screen.findByRole("button", { name: "Reopen" }));
    await waitFor(() => expect(ipcMock.supportSetOpen).toHaveBeenCalledWith(8, true));
  });
});
