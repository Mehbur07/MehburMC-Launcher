import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  launchInstance: vi.fn(async () => "task-1"),
  stopInstance: vi.fn(async () => true),
  selectInstance: vi.fn(async () => {}),
  listNews: vi.fn(async () => []),
}));
vi.mock("../../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../../lib/ipc")>()),
  ipc: ipcMock,
}));

import "../../i18n";
import { applyLanguage } from "../../i18n";
import { useAccounts } from "../../stores/accounts";
import { useApp } from "../../stores/app";
import { useInstances } from "../../stores/instances";
import { useTasks } from "../../stores/tasks";
import { instance, task } from "../../test/fixtures";
import { HomePage } from "./HomePage";

describe("HomePage", () => {
  beforeEach(() => {
    applyLanguage("en");
    vi.clearAllMocks();
    useTasks.setState({ tasks: {} });
    useAccounts.setState({
      accounts: [{ id: "a", kind: "offline", name: "Steve", uuid: "u", addedAt: 1 }],
      selected: "a",
    });
    useInstances.setState({
      instances: [instance(), instance({ id: "pack-2", name: "Second", mcVersion: "26.2" })],
      selected: "pack-1",
      loaded: true,
    });
  });

  it("invites creating an instance when there is none", () => {
    useInstances.setState({ instances: [], selected: null });
    render(<HomePage />);
    fireEvent.click(screen.getByRole("button", { name: /Create instance/i }));
    expect(useApp.getState().wizardOpen).toBe(true);
  });

  it("plays the selected instance and switches between instances", async () => {
    render(<HomePage />);
    expect(screen.getByRole("heading", { name: "My Pack" })).toBeInTheDocument();
    expect(screen.getByText("Steve")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /PLAY/ }));
    await vi.waitFor(() => expect(ipcMock.launchInstance).toHaveBeenCalledWith("pack-1"));

    fireEvent.click(screen.getByRole("button", { name: /Second/ }));
    expect(ipcMock.selectInstance).toHaveBeenCalledWith("pack-2");
  });

  it("turns PLAY into STOP while the game runs", async () => {
    useTasks.setState({ tasks: { "task-1": task({ status: "playing" }) } });
    render(<HomePage />);
    expect(screen.getByText("Game running")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /STOP/ }));
    await vi.waitFor(() => expect(ipcMock.stopInstance).toHaveBeenCalledWith("pack-1"));
  });
});
