import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  listNews: vi.fn(),
  contentIcon: vi.fn(async () => ""),
  openExternal: vi.fn(),
}));
vi.mock("../../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../../lib/ipc")>()),
  ipc: ipcMock,
}));

import "../../i18n";
import { applyLanguage } from "../../i18n";
import type { NewsItem } from "../../lib/ipc/bindings/NewsItem";
import { NEWS_REFRESH_MS, NewsFeed, useNews } from "./NewsFeed";

const item = (id: string, title = id): NewsItem => ({ id, title, text: "", date: "2026-10-20" });

let hidden = false;
const flush = () => act(async () => vi.advanceTimersByTimeAsync(0));

describe("NewsFeed", () => {
  beforeEach(() => {
    applyLanguage("en");
    vi.useFakeTimers();
    vi.clearAllMocks();
    hidden = false;
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      get: () => (hidden ? "hidden" : "visible"),
    });
    useNews.setState({ items: null, fresh: new Set(), loadedAt: 0, busy: false });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("refreshes on a timer and marks new stories", async () => {
    ipcMock.listNews.mockResolvedValueOnce([item("a", "Snapshot 26w43a")]);
    render(<NewsFeed />);
    await flush();
    expect(screen.getByText("Snapshot 26w43a")).toBeInTheDocument();
    expect(screen.queryByText("New")).toBeNull();

    ipcMock.listNews.mockResolvedValueOnce([
      item("b", "Minecraft 26.4"),
      item("a", "Snapshot 26w43a"),
    ]);
    await act(async () => vi.advanceTimersByTimeAsync(NEWS_REFRESH_MS));
    expect(screen.getByText("Minecraft 26.4")).toBeInTheDocument();
    expect(screen.getAllByText("New")).toHaveLength(1);
    expect(ipcMock.listNews).toHaveBeenCalledTimes(2);
  });

  it("does not poll while the window is hidden and catches up when it returns", async () => {
    ipcMock.listNews.mockResolvedValue([item("a")]);
    render(<NewsFeed />);
    await flush();
    hidden = true;
    await act(async () => vi.advanceTimersByTimeAsync(NEWS_REFRESH_MS * 2));
    expect(ipcMock.listNews).toHaveBeenCalledTimes(1);
    hidden = false;
    fireEvent(window, new Event("focus"));
    await flush();
    expect(ipcMock.listNews).toHaveBeenCalledTimes(2);
    // Right after a load, coming back again does not re-fetch.
    fireEvent(document, new Event("visibilitychange"));
    await flush();
    expect(ipcMock.listNews).toHaveBeenCalledTimes(2);
  });

  it("keeps the stories on screen when a refresh comes back empty (offline)", async () => {
    ipcMock.listNews.mockResolvedValueOnce([item("a", "Kept")]).mockResolvedValueOnce([]);
    render(<NewsFeed />);
    await flush();
    await act(async () => vi.advanceTimersByTimeAsync(NEWS_REFRESH_MS));
    expect(ipcMock.listNews).toHaveBeenCalledTimes(2);
    expect(screen.getByText("Kept")).toBeInTheDocument();
  });

  it("shows nothing until there is news", async () => {
    ipcMock.listNews.mockResolvedValue([]);
    const { container } = render(<NewsFeed />);
    await flush();
    expect(container).toBeEmptyDOMElement();
  });
});
