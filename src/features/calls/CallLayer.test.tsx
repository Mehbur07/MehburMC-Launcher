import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  callsSnapshot: vi.fn(),
  callLeave: vi.fn(),
}));
const win = vi.hoisted(() => ({ setFullscreen: vi.fn(async () => undefined) }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => win }));
const tones = vi.hoisted(() => ({ playTone: vi.fn(() => () => undefined) }));

vi.mock("../../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../../lib/ipc")>()),
  ipc: ipcMock,
  subscribeCalls: vi.fn(async () => () => undefined),
}));
vi.mock("../../lib/voice/audio", () => ({
  playTone: tones.playTone,
  watchSpeaking: vi.fn(() => () => undefined),
}));

import "../../i18n";
import { applyLanguage } from "../../i18n";
import type { Call } from "../../lib/ipc/bindings/Call";
import { useCalls } from "../../stores/calls";
import { useFriends } from "../../stores/friends";
import { CallLayer } from "./CallLayer";

const ringing: Call = {
  id: 7,
  groupId: null,
  createdBy: "f1",
  createdAt: "t",
  myState: "ringing",
  members: [
    { id: "f1", displayName: "Ali", nickname: "Kanka", state: "joined", joinedAt: "t0" },
    { id: "me", displayName: "Ben", nickname: null, state: "ringing", joinedAt: null },
  ],
};

describe("CallLayer", () => {
  beforeEach(() => {
    applyLanguage("en");
    vi.clearAllMocks();
    useFriends.setState({
      profile: { id: "me", friendCode: "MEHBUR-MEME", displayName: "Ben" },
      friends: [],
      groups: [],
    });
    useCalls.setState({
      calls: [],
      activeId: null,
      joining: false,
      since: null,
      muted: false,
      deafened: false,
      speaking: {},
      peers: {},
      volumes: {},
      micError: null,
      error: null,
      sharing: null,
      sharingAudio: false,
      screens: {},
    });
  });

  it("rings for an incoming call and answers it", async () => {
    const accept = vi.fn(async () => undefined);
    const decline = vi.fn(async () => undefined);
    useCalls.setState({ accept, decline });
    ipcMock.callsSnapshot.mockResolvedValue({ calls: [ringing], signals: [] });
    render(<CallLayer />);
    const card = await screen.findByRole("alertdialog", { name: "Incoming voice call" });
    expect(card).toHaveTextContent("Kanka");
    expect(tones.playTone).toHaveBeenCalledWith("incoming");
    fireEvent.click(screen.getByRole("button", { name: "Accept" }));
    expect(accept).toHaveBeenCalledWith(7);
    fireEvent.click(screen.getByRole("button", { name: "Decline" }));
    expect(decline).toHaveBeenCalledWith(7);
  });

  it("shows the active call with its members and controls", async () => {
    const toggleMute = vi.fn();
    const leave = vi.fn(async () => undefined);
    ipcMock.callsSnapshot.mockResolvedValue({ calls: [], signals: [] });
    const joined: Call = {
      ...ringing,
      myState: "joined",
      members: [
        { id: "f1", displayName: "Ali", nickname: null, state: "joined", joinedAt: "t0" },
        { id: "me", displayName: "Ben", nickname: null, state: "joined", joinedAt: "t1" },
      ],
    };
    ipcMock.callsSnapshot.mockResolvedValue({ calls: [joined], signals: [] });
    useCalls.setState({
      toggleMute,
      leave,
      calls: [joined],
      activeId: 7,
      since: Date.now() - 65_000,
      peers: { f1: "connected" },
      speaking: { f1: true },
    });
    render(<CallLayer />);
    const bar = await screen.findByRole("region", { name: "Voice call" });
    expect(bar).toHaveTextContent("Ali");
    expect(bar).toHaveTextContent("You");
    expect(bar).toHaveTextContent("01:05");
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(screen.getByRole("slider", { name: "Volume of Ali" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Mute microphone" }));
    expect(toggleMute).toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Leave call" }));
    expect(leave).toHaveBeenCalled();
  });

  it("plays the ring-back tone while the friend has not answered", async () => {
    ipcMock.callsSnapshot.mockResolvedValue({ calls: [], signals: [] });
    const calling: Call = {
      ...ringing,
      createdBy: "me",
      myState: "joined",
      members: [
        { id: "me", displayName: "Ben", nickname: null, state: "joined", joinedAt: "t0" },
        { id: "f1", displayName: "Ali", nickname: null, state: "ringing", joinedAt: null },
      ],
    };
    ipcMock.callsSnapshot.mockResolvedValue({ calls: [calling], signals: [] });
    useCalls.setState({ calls: [calling], activeId: 7, since: Date.now() });
    render(<CallLayer />);
    expect(await screen.findByText("Calling…")).toBeInTheDocument();
    await waitFor(() => expect(tones.playTone).toHaveBeenCalledWith("outgoing"));
  });

  it("reports call errors until dismissed", async () => {
    ipcMock.callsSnapshot.mockResolvedValue({ calls: [], signals: [] });
    useCalls.setState({ error: { code: "calls.ended", params: {}, detail: "" } });
    render(<CallLayer />);
    expect(await screen.findByRole("alert")).toHaveTextContent("The call has ended.");
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.queryByRole("alert")).toBeNull();
  });

  const joinedCall = (): Call => ({
    ...ringing,
    groupId: 3,
    myState: "joined",
    members: [
      { id: "f1", displayName: "Ali", nickname: null, state: "joined", joinedAt: "t0" },
      { id: "f2", displayName: "Veli", nickname: null, state: "joined", joinedAt: "t0" },
      { id: "me", displayName: "Ben", nickname: null, state: "joined", joinedAt: "t1" },
    ],
  });

  it("picks a quality and starts sharing, then stops", async () => {
    const startShare = vi.fn(async () => undefined);
    const stopShare = vi.fn();
    const c = joinedCall();
    ipcMock.callsSnapshot.mockResolvedValue({ calls: [c], signals: [] });
    useCalls.setState({ startShare, stopShare, calls: [c], activeId: 7, since: Date.now() });
    const { rerender } = render(<CallLayer />);
    fireEvent.click(await screen.findByRole("button", { name: "Share screen" }));
    const menu = screen.getByRole("dialog", { name: "Video quality" });
    expect(within(menu).getByRole("radio", { name: /1080p · 30 FPS/ })).toBeChecked();
    fireEvent.click(within(menu).getByRole("radio", { name: /60 FPS/ }));
    fireEvent.click(within(menu).getByRole("button", { name: "Choose a screen or window" }));
    expect(startShare).toHaveBeenCalledWith("1080p60");
    expect(screen.queryByRole("dialog", { name: "Video quality" })).toBeNull();

    useCalls.setState({ sharing: "1080p60", sharingAudio: true });
    rerender(<CallLayer />);
    expect(screen.getByText("Sharing your screen")).toBeInTheDocument();
    expect(screen.getByLabelText("System audio included")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Stop sharing" }));
    expect(stopShare).toHaveBeenCalled();
  });

  it("shows others' screens, switches between sharers and goes full screen", async () => {
    const c = joinedCall();
    ipcMock.callsSnapshot.mockResolvedValue({ calls: [c], signals: [] });
    useCalls.setState({ calls: [c], activeId: 7, since: Date.now(), screens: { f1: 1 } });
    const { rerender } = render(<CallLayer />);
    const panel = await screen.findByRole("region", { name: "Ali is sharing their screen" });
    expect(within(panel).getByLabelText("Ali is sharing their screen")).toBeInTheDocument();

    useCalls.setState({ screens: { f1: 1, f2: 1 } });
    rerender(<CallLayer />);
    fireEvent.click(screen.getByRole("button", { name: "Veli" }));
    expect(
      screen.getByRole("region", { name: "Veli is sharing their screen" }),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Full screen" }));
    await waitFor(() => expect(win.setFullscreen).toHaveBeenCalledWith(true));
    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => expect(win.setFullscreen).toHaveBeenCalledWith(false));
    expect(screen.getByRole("button", { name: "Full screen" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Hide" }));
    const chip = screen.getByRole("button", { name: /2 people are sharing/ });
    fireEvent.click(chip);
    expect(screen.getByRole("button", { name: "Full screen" })).toBeInTheDocument();

    useCalls.setState({ screens: {} });
    rerender(<CallLayer />);
    expect(screen.queryByRole("button", { name: "Full screen" })).toBeNull();
  });
});
