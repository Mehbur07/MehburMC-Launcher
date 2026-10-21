import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  callsSnapshot: vi.fn(),
  callLeave: vi.fn(),
}));
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
});
