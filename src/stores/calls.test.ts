import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  callIceConfig: vi.fn(),
  callStart: vi.fn(),
  callJoin: vi.fn(),
  callLeave: vi.fn(),
  callSignal: vi.fn(),
  callsSnapshot: vi.fn(),
}));
const meshes = vi.hoisted(() => [] as MockMesh[]);
type MockMesh = {
  opts: {
    relayOnly: boolean;
    send: (s: unknown[]) => Promise<unknown>;
    onRemoteScreen: (peer: string, screen: unknown) => void;
  };
  sync: ReturnType<typeof vi.fn>;
  handle: ReturnType<typeof vi.fn>;
  setTrack: ReturnType<typeof vi.fn>;
  setScreen: ReturnType<typeof vi.fn>;
  close: ReturnType<typeof vi.fn>;
};

vi.mock("../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../lib/ipc")>()),
  ipc: ipcMock,
  subscribeCalls: vi.fn(async () => () => undefined),
}));
vi.mock("../lib/voice/mesh", async (orig) => ({
  ...(await orig<typeof import("../lib/voice/mesh")>()),
  VoiceMesh: class {
    sync = vi.fn();
    handle = vi.fn(async () => undefined);
    setTrack = vi.fn(async () => undefined);
    setScreen = vi.fn(async () => undefined);
    close = vi.fn();
    constructor(public opts: MockMesh["opts"]) {
      meshes.push(this as unknown as MockMesh);
    }
  },
}));
vi.mock("../lib/voice/audio", () => ({
  watchSpeaking: vi.fn(() => () => undefined),
  playTone: vi.fn(() => () => undefined),
}));

import type { Call } from "../lib/ipc/bindings/Call";
import type { Settings } from "../lib/ipc/bindings/Settings";
import { useApp } from "./app";
import { useCalls } from "./calls";
import { useFriends } from "./friends";

const track = { enabled: true, stop: vi.fn() };
const stream = { getAudioTracks: () => [track], getTracks: () => [track] };
const getUserMedia = vi.fn();
const getDisplayMedia = vi.fn();

/** jsdom has no MediaStream; enough of one for the screen streams. */
class FakeStream {
  constructor(public tracks: unknown[] = []) {}
  getTracks() {
    return this.tracks;
  }
}
vi.stubGlobal("MediaStream", FakeStream);
vi.stubGlobal(
  "Audio",
  class {
    autoplay = false;
    muted = false;
    volume = 1;
    srcObject: unknown = null;
    pause() {}
    play() {
      return Promise.resolve();
    }
  },
);

function screenCapture(withAudio: boolean) {
  const video = {
    kind: "video",
    contentHint: "",
    onended: null as null | (() => void),
    stop: vi.fn(),
  };
  const audio = { kind: "audio", stop: vi.fn() };
  const tracks = withAudio ? [video, audio] : [video];
  return {
    video,
    audio,
    stream: {
      getVideoTracks: () => [video],
      getAudioTracks: () => (withAudio ? [audio] : []),
      getTracks: () => tracks,
    },
  };
}

const call = (over: Partial<Call> = {}): Call => ({
  id: 7,
  groupId: null,
  createdBy: "me",
  createdAt: "t",
  myState: "joined",
  members: [
    { id: "me", displayName: "Ben", nickname: null, state: "joined", joinedAt: "t0" },
    { id: "f1", displayName: "Ali", nickname: null, state: "ringing", joinedAt: null },
  ],
  ...over,
});

const signal = (id: number, callId = 7) => ({
  id,
  callId,
  sender: "f1",
  kind: "offer" as const,
  payload: "{}",
});

describe("calls store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    meshes.length = 0;
    track.enabled = true;
    Object.defineProperty(navigator, "mediaDevices", {
      value: { getUserMedia, getDisplayMedia },
      configurable: true,
    });
    getUserMedia.mockResolvedValue(stream);
    ipcMock.callIceConfig.mockResolvedValue({ iceServers: [], relay: true });
    ipcMock.callStart.mockResolvedValue(7);
    ipcMock.callJoin.mockResolvedValue(undefined);
    ipcMock.callLeave.mockResolvedValue(undefined);
    useFriends.setState({ profile: { id: "me", friendCode: "MEHBUR-MEME", displayName: "Ben" } });
    useApp.setState({ settings: { voiceHideIp: true } as Settings });
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

  it("starts a call with the microphone, relaying when the IP is hidden", async () => {
    await useCalls.getState().start({ friend: "f1" });
    expect(getUserMedia).toHaveBeenCalledWith({
      audio: expect.objectContaining({ echoCancellation: true, noiseSuppression: true }),
    });
    expect(ipcMock.callStart).toHaveBeenCalledWith({ friend: "f1" });
    const s = useCalls.getState();
    expect(s.activeId).toBe(7);
    expect(s.joining).toBe(false);
    expect(meshes[0]!.opts.relayOnly).toBe(true);
    expect(meshes[0]!.setTrack).toHaveBeenCalledWith(track);
  });

  it("goes direct when hiding is off or no relay exists", async () => {
    useApp.setState({ settings: { voiceHideIp: false } as Settings });
    await useCalls.getState().start({ friend: "f1" });
    expect(meshes[0]!.opts.relayOnly).toBe(false);
    await useCalls.getState().leave();
    useApp.setState({ settings: { voiceHideIp: true } as Settings });
    ipcMock.callIceConfig.mockResolvedValue({ iceServers: [], relay: false });
    await useCalls.getState().start({ friend: "f1" });
    expect(meshes[1]!.opts.relayOnly).toBe(false);
  });

  it("joins listen-only when the microphone is refused", async () => {
    getUserMedia.mockRejectedValue(new DOMException("no", "NotAllowedError"));
    await useCalls.getState().accept(7);
    expect(ipcMock.callJoin).toHaveBeenCalledWith(7);
    const s = useCalls.getState();
    expect(s.activeId).toBe(7);
    expect(s.micError).toBe("calls.micDenied");
    expect(meshes[0]!.setTrack).toHaveBeenCalledWith(null);
  });

  it("feeds the members and this call's signals to the mesh", async () => {
    await useCalls.getState().start({ friend: "f1" });
    const c = call();
    useCalls.getState().onPoll({ calls: [c], signals: [signal(1), signal(2, 99)] });
    expect(meshes[0]!.sync).toHaveBeenCalledWith(c.members);
    await vi.waitFor(() => expect(meshes[0]!.handle).toHaveBeenCalledTimes(1));
    expect(meshes[0]!.handle).toHaveBeenCalledWith(signal(1));
  });

  it("applies signals that arrived while joining", async () => {
    let release: (id: number) => void = () => undefined;
    ipcMock.callStart.mockReturnValue(new Promise<number>((r) => (release = r)));
    const started = useCalls.getState().start({ friend: "f1" });
    await vi.waitFor(() => expect(ipcMock.callStart).toHaveBeenCalled());
    useCalls.getState().onPoll({ calls: [call()], signals: [signal(5)] });
    release(7);
    await started;
    await vi.waitFor(() => expect(meshes[0]!.handle).toHaveBeenCalledWith(signal(5)));
  });

  it("ends the call when the server no longer lists it", async () => {
    await useCalls.getState().start({ friend: "f1" });
    useCalls.getState().onPoll({ calls: [], signals: [] });
    expect(useCalls.getState().activeId).toBeNull();
    expect(meshes[0]!.close).toHaveBeenCalled();
    expect(track.stop).toHaveBeenCalled();
  });

  it("rejoins a group call it was dropped from, at most twice", async () => {
    await useCalls.getState().start({ group: 3 });
    const dropped = call({ groupId: 3, myState: "left" });
    useCalls.getState().onPoll({ calls: [dropped], signals: [] });
    useCalls.getState().onPoll({ calls: [dropped], signals: [] });
    expect(ipcMock.callJoin).toHaveBeenCalledTimes(2);
    expect(useCalls.getState().activeId).toBe(7);
    useCalls.getState().onPoll({ calls: [dropped], signals: [] });
    expect(useCalls.getState().activeId).toBeNull();
  });

  it("mutes, deafens and restores the microphone", async () => {
    await useCalls.getState().start({ friend: "f1" });
    const { toggleMute, toggleDeafen } = useCalls.getState();
    toggleMute();
    expect(track.enabled).toBe(false);
    toggleMute();
    expect(track.enabled).toBe(true);
    toggleDeafen();
    expect(useCalls.getState()).toMatchObject({ deafened: true, muted: true });
    expect(track.enabled).toBe(false);
    toggleDeafen();
    expect(useCalls.getState()).toMatchObject({ deafened: false, muted: false });
    expect(track.enabled).toBe(true);
  });

  it("declines and leaves through the server", async () => {
    await useCalls.getState().decline(9);
    expect(ipcMock.callLeave).toHaveBeenCalledWith(9);
    await useCalls.getState().start({ friend: "f1" });
    await useCalls.getState().leave();
    expect(ipcMock.callLeave).toHaveBeenLastCalledWith(7);
    expect(useCalls.getState().activeId).toBeNull();
  });

  it("shows the error and frees the microphone when joining fails", async () => {
    ipcMock.callJoin.mockRejectedValue({ code: "calls.ended", params: {}, detail: "" });
    await useCalls.getState().accept(7);
    const s = useCalls.getState();
    expect(s.activeId).toBeNull();
    expect(s.error?.code).toBe("calls.ended");
    expect(track.stop).toHaveBeenCalled();
  });

  it("sends signals only once the call id is known", async () => {
    await useCalls.getState().start({ friend: "f1" });
    await meshes[0]!.opts.send([{ recipient: "f1", kind: "ice", payload: "{}" }]);
    expect(ipcMock.callSignal).toHaveBeenCalledWith(7, [
      { recipient: "f1", kind: "ice", payload: "{}" },
    ]);
  });

  it("shares the picked screen with system audio and stops it", async () => {
    await useCalls.getState().start({ friend: "f1" });
    const cap = screenCapture(true);
    getDisplayMedia.mockResolvedValue(cap.stream);
    await useCalls.getState().startShare("1080p60");
    expect(getDisplayMedia).toHaveBeenCalledWith(
      expect.objectContaining({
        video: expect.objectContaining({ frameRate: { ideal: 60, max: 60 } }),
        audio: expect.objectContaining({ restrictOwnAudio: true }),
        systemAudio: "include",
      }),
    );
    expect(meshes[0]!.setScreen).toHaveBeenCalledWith({
      video: cap.video,
      audio: cap.audio,
      quality: "1080p60",
    });
    expect(cap.video.contentHint).toBe("motion");
    let s = useCalls.getState();
    expect(s.sharing).toBe("1080p60");
    expect(s.sharingAudio).toBe(true);
    expect(s.shareQuality).toBe("1080p60");

    // The browser's own "Stop sharing" ends the track.
    cap.video.onended?.();
    expect(meshes[0]!.setScreen).toHaveBeenLastCalledWith(null);
    expect(cap.video.stop).toHaveBeenCalled();
    expect(cap.audio.stop).toHaveBeenCalled();
    s = useCalls.getState();
    expect(s.sharing).toBeNull();
  });

  it("treats a closed picker as no share and other failures as an error", async () => {
    await useCalls.getState().start({ friend: "f1" });
    getDisplayMedia.mockRejectedValueOnce(new DOMException("no", "NotAllowedError"));
    await useCalls.getState().startShare("1080p30");
    expect(useCalls.getState().error).toBeNull();
    expect(useCalls.getState().sharing).toBeNull();
    getDisplayMedia.mockRejectedValueOnce(new DOMException("x", "NotReadableError"));
    await useCalls.getState().startShare("1080p30");
    expect(useCalls.getState().error?.code).toBe("calls.screenFailed");
    expect(meshes[0]!.setScreen).not.toHaveBeenCalled();
  });

  it("does nothing outside a call", async () => {
    await useCalls.getState().startShare("720p30");
    expect(getDisplayMedia).not.toHaveBeenCalled();
  });

  it("tracks others' shares and clears everything when leaving", async () => {
    await useCalls.getState().start({ friend: "f1" });
    const cap = screenCapture(false);
    getDisplayMedia.mockResolvedValue(cap.stream);
    await useCalls.getState().startShare("720p30");
    expect(cap.video.contentHint).toBe("detail");
    expect(useCalls.getState().sharingAudio).toBe(false);
    const video = { kind: "video" };
    meshes[0]!.opts.onRemoteScreen("f1", { video, audio: { kind: "audio" } });
    expect(useCalls.getState().screens).toEqual({ f1: 1 });
    meshes[0]!.opts.onRemoteScreen("f1", { video, audio: null });
    expect(useCalls.getState().screens).toEqual({ f1: 2 });
    meshes[0]!.opts.onRemoteScreen("f1", null);
    expect(useCalls.getState().screens).toEqual({});
    meshes[0]!.opts.onRemoteScreen("f1", { video, audio: null });
    await useCalls.getState().leave();
    const s = useCalls.getState();
    expect(s.screens).toEqual({});
    expect(s.sharing).toBeNull();
    expect(cap.video.stop).toHaveBeenCalled();
  });
});
