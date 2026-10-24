import { create } from "zustand";

import { ipc, subscribeCalls, toErrorPayload } from "../lib/ipc";
import type { Call } from "../lib/ipc/bindings/Call";
import type { CallPoll } from "../lib/ipc/bindings/CallPoll";
import type { CallSignal } from "../lib/ipc/bindings/CallSignal";
import type { ErrorPayload } from "../lib/ipc/bindings/ErrorPayload";
import type { IceConfig } from "../lib/ipc/bindings/IceConfig";
import { watchSpeaking } from "../lib/voice/audio";
import {
  type PeerStatus,
  type RemoteScreen,
  SCREEN_QUALITY,
  type ScreenQuality,
  VoiceMesh,
} from "../lib/voice/mesh";
import { useApp } from "./app";
import { useFriends } from "./friends";

export type CallTarget = { friend: string } | { group: number };

interface CallsStore {
  /** Live calls we ring in, are in or could join (our groups). */
  calls: Call[];
  /** The call we are in (or joining). */
  activeId: number | null;
  joining: boolean;
  /** Date.now() when we joined, for the call timer. */
  since: number | null;
  muted: boolean;
  deafened: boolean;
  speaking: Record<string, boolean>;
  peers: Record<string, PeerStatus>;
  volumes: Record<string, number>;
  /** Microphone problem (`calls.micDenied` / `calls.micMissing`): listen only. */
  micError: string | null;
  error: ErrorPayload | null;
  /** Our screen share (K89): its quality, or null when not sharing. */
  sharing: ScreenQuality | null;
  /** Our share includes system audio. */
  sharingAudio: boolean;
  /** Last picked quality (remembered on this PC). */
  shareQuality: ScreenQuality;
  /** Members sharing their screen with us; the value changes with their tracks. */
  screens: Record<string, number>;

  /** Starts listening to the poll loop; returns the unsubscribe function. */
  connect: () => Promise<() => void>;
  onPoll: (p: CallPoll) => void;
  start: (target: CallTarget) => Promise<void>;
  accept: (callId: number) => Promise<void>;
  decline: (callId: number) => Promise<void>;
  leave: () => Promise<void>;
  toggleMute: () => void;
  toggleDeafen: () => void;
  setVolume: (peer: string, volume: number) => void;
  dismissError: () => void;
  /** Opens the screen picker and shares the choice with everyone in the call. */
  startShare: (quality: ScreenQuality) => Promise<void>;
  stopShare: () => void;
}

const QUALITY_KEY = "mehbur.shareQuality";

function savedQuality(): ScreenQuality {
  try {
    const q = localStorage.getItem(QUALITY_KEY);
    if (q && q in SCREEN_QUALITY) return q as ScreenQuality;
  } catch {
    // Storage blocked: the default.
  }
  return "1080p30";
}

/** A member's shared screen (video + optional system audio) for a `<video>`. */
export const screenStream = (peer: string): MediaStream | null => rt.screens.get(peer) ?? null;

/** Our own share, for the preview. */
export const localScreenStream = (): MediaStream | null => rt.screen;

/** STUN only, when the TURN credentials request itself fails. */
const FALLBACK_ICE: IceConfig = {
  iceServers: [{ urls: ["stun:stun.cloudflare.com:3478", "stun:stun.l.google.com:19302"] }],
  relay: false,
};
/** Re-joins after the server dropped us (e.g. a network hiccup over 20 s). */
const MAX_REJOINS = 2;

/** Non-serialisable parts of the active call. */
const rt = {
  mesh: null as VoiceMesh | null,
  stream: null as MediaStream | null,
  audio: new Map<string, HTMLAudioElement>(),
  watchers: new Map<string, () => void>(),
  /** Signals are applied strictly in order. */
  chain: Promise.resolve() as Promise<unknown>,
  /** Signals that arrived while the call id was not known yet. */
  stash: [] as CallSignal[],
  rejoins: 0,
  mutedBeforeDeafen: false,
  /** Our screen capture (K89). */
  screen: null as MediaStream | null,
  /** Others' shared screens and the players of their system audio. */
  screens: new Map<string, MediaStream>(),
  screenAudio: new Map<string, HTMLAudioElement>(),
};

const me = () => useFriends.getState().profile?.id ?? null;

async function openMic(): Promise<{ stream: MediaStream | null; error: string | null }> {
  const device = useApp.getState().settings?.voiceInputDevice;
  const audio: MediaTrackConstraints = {
    echoCancellation: true,
    noiseSuppression: true,
    autoGainControl: true,
    ...(device ? { deviceId: { ideal: device } } : {}),
  };
  try {
    if (!navigator.mediaDevices?.getUserMedia) return { stream: null, error: "calls.micMissing" };
    return { stream: await navigator.mediaDevices.getUserMedia({ audio }), error: null };
  } catch (e) {
    const name = e instanceof DOMException ? e.name : "";
    return {
      stream: null,
      error:
        name === "NotAllowedError" || name === "SecurityError"
          ? "calls.micDenied"
          : "calls.micMissing",
    };
  }
}

/**
 * The screen picker (WebView2's own). The audio box appears for screens and
 * windows; our call audio is asked to be left out of it (no echo).
 */
async function openScreen(quality: ScreenQuality): Promise<MediaStream> {
  const q = SCREEN_QUALITY[quality];
  return navigator.mediaDevices.getDisplayMedia({
    video: {
      width: { ideal: q.width },
      height: { ideal: q.height },
      frameRate: { ideal: q.fps, max: q.fps },
    },
    audio: {
      echoCancellation: false,
      noiseSuppression: false,
      autoGainControl: false,
      restrictOwnAudio: true,
    } as MediaTrackConstraints,
    systemAudio: "include",
    selfBrowserSurface: "exclude",
  } as DisplayMediaStreamOptions);
}

export const callTitle = (call: Call | undefined) => {
  if (!call) return "";
  if (call.groupId !== null)
    return useFriends.getState().groups.find((g) => g.id === call.groupId)?.name ?? "";
  const other = call.members.find((m) => m.id !== me());
  return other ? (other.nickname ?? other.displayName) : "";
};

export const useCalls = create<CallsStore>((set, get) => {
  const setSpeaking = (id: string, on: boolean) =>
    set((s) => (s.speaking[id] === on ? s : { speaking: { ...s.speaking, [id]: on } }));

  const watch = (id: string, stream: MediaStream) => {
    rt.watchers.get(id)?.();
    rt.watchers.set(
      id,
      watchSpeaking(stream, (on) => setSpeaking(id, on)),
    );
  };

  const onRemoteStream = (peer: string, stream: MediaStream | null) => {
    rt.watchers.get(peer)?.();
    rt.watchers.delete(peer);
    const old = rt.audio.get(peer);
    if (old) {
      old.pause();
      old.srcObject = null;
      rt.audio.delete(peer);
    }
    if (!stream) {
      setSpeaking(peer, false);
      return;
    }
    const el = new Audio();
    el.autoplay = true;
    el.srcObject = stream;
    el.muted = get().deafened;
    el.volume = get().volumes[peer] ?? 1;
    void el.play?.()?.catch(() => undefined);
    rt.audio.set(peer, el);
    watch(peer, stream);
  };

  const onRemoteScreen = (peer: string, screen: RemoteScreen | null) => {
    const old = rt.screenAudio.get(peer);
    if (old) {
      old.pause();
      old.srcObject = null;
      rt.screenAudio.delete(peer);
    }
    rt.screens.delete(peer);
    if (screen) {
      rt.screens.set(
        peer,
        new MediaStream(screen.audio ? [screen.video, screen.audio] : [screen.video]),
      );
      if (screen.audio) {
        // Played here (the <video> stays muted) so deafen and volume apply.
        const el = new Audio();
        el.autoplay = true;
        el.srcObject = new MediaStream([screen.audio]);
        el.muted = get().deafened;
        el.volume = get().volumes[peer] ?? 1;
        void el.play?.()?.catch(() => undefined);
        rt.screenAudio.set(peer, el);
      }
    }
    set((s) => {
      const screens = { ...s.screens };
      if (screen) screens[peer] = (screens[peer] ?? 0) + 1;
      else delete screens[peer];
      return { screens };
    });
  };

  const stopScreen = () => {
    rt.screen?.getTracks().forEach((t) => {
      t.onended = null;
      t.stop();
    });
    rt.screen = null;
  };

  const onPeerStatus = (peer: string, status: PeerStatus | null) =>
    set((s) => {
      const peers = { ...s.peers };
      if (status) peers[peer] = status;
      else delete peers[peer];
      return { peers };
    });

  const teardown = () => {
    rt.mesh?.close();
    rt.mesh = null;
    rt.stream?.getTracks().forEach((t) => t.stop());
    rt.stream = null;
    for (const stop of rt.watchers.values()) stop();
    rt.watchers.clear();
    for (const el of rt.audio.values()) {
      el.pause();
      el.srcObject = null;
    }
    rt.audio.clear();
    stopScreen();
    for (const el of rt.screenAudio.values()) {
      el.pause();
      el.srcObject = null;
    }
    rt.screenAudio.clear();
    rt.screens.clear();
    rt.stash = [];
    rt.chain = Promise.resolve();
    rt.rejoins = 0;
    set({
      activeId: null,
      joining: false,
      since: null,
      speaking: {},
      peers: {},
      micError: null,
      muted: false,
      deafened: false,
      sharing: null,
      sharingAudio: false,
      screens: {},
    });
  };

  const apply = (call: Call, signals: CallSignal[]) => {
    rt.mesh?.sync(call.members);
    for (const s of signals) {
      if (s.callId !== call.id) continue;
      rt.chain = rt.chain.then(() => rt.mesh?.handle(s));
    }
  };

  /** Microphone + ICE servers, then `join` (the RPC), then the mesh. */
  const begin = async (join: () => Promise<number>) => {
    const self = me();
    if (!self) return;
    if (get().activeId !== null) await get().leave();
    set({ joining: true, error: null });
    const [{ stream, error: micError }, ice] = await Promise.all([
      openMic(),
      ipc.callIceConfig().catch(() => FALLBACK_ICE),
    ]);
    const relayOnly = ice.relay && (useApp.getState().settings?.voiceHideIp ?? true);
    const mesh = new VoiceMesh({
      me: self,
      iceServers: ice.iceServers as RTCIceServer[],
      relayOnly,
      send: (signals) => {
        const id = get().activeId;
        return id === null ? Promise.resolve() : ipc.callSignal(id, signals);
      },
      onRemoteStream,
      onPeerStatus,
      onRemoteScreen,
    });
    const track = stream?.getAudioTracks()[0] ?? null;
    await mesh.setTrack(track);
    rt.mesh = mesh;
    rt.stream = stream;
    let id: number;
    try {
      id = await join();
    } catch (e) {
      teardown();
      set({ error: toErrorPayload(e) });
      return;
    }
    set({ activeId: id, joining: false, since: Date.now(), micError, muted: false });
    if (stream) watch(self, stream);
    const call = get().calls.find((c) => c.id === id);
    const stash = rt.stash;
    rt.stash = [];
    if (call?.myState === "joined") apply(call, stash);
  };

  return {
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
    shareQuality: savedQuality(),
    screens: {},

    connect: async () => {
      const stop = await subscribeCalls((p) => get().onPoll(p));
      try {
        get().onPoll(await ipc.callsSnapshot());
      } catch {
        // The next poll fills it in.
      }
      return stop;
    },

    onPoll: (p) => {
      set({ calls: p.calls });
      const id = get().activeId;
      if (id === null) {
        // Joining: the RPC has not returned the id yet.
        if (get().joining) rt.stash.push(...p.signals);
        return;
      }
      if (!rt.mesh) return;
      const call = p.calls.find((c) => c.id === id);
      if (call?.myState === "joined") {
        rt.rejoins = 0;
        apply(call, p.signals);
        return;
      }
      // The server dropped us (time-out, a group call we can rejoin) or the call ended.
      if (call && call.groupId !== null && rt.rejoins < MAX_REJOINS) {
        rt.rejoins += 1;
        void ipc.callJoin(id).catch(() => teardown());
        return;
      }
      teardown();
    },

    start: (target) => begin(() => ipc.callStart(target)),

    accept: (callId) =>
      begin(async () => {
        await ipc.callJoin(callId);
        return callId;
      }),

    decline: async (callId) => {
      try {
        await ipc.callLeave(callId);
      } catch (e) {
        set({ error: toErrorPayload(e) });
      }
    },

    leave: async () => {
      const id = get().activeId;
      teardown();
      if (id === null) return;
      try {
        await ipc.callLeave(id);
      } catch (e) {
        set({ error: toErrorPayload(e) });
      }
    },

    toggleMute: () => {
      const muted = !get().muted;
      rt.stream?.getAudioTracks().forEach((t) => (t.enabled = !muted));
      const self = me();
      if (muted && self) setSpeaking(self, false);
      set({ muted, ...(muted ? {} : { deafened: false }) });
      if (!muted) for (const el of rt.audio.values()) el.muted = false;
    },

    // Deafened also mutes the microphone; undeafening restores it.
    toggleDeafen: () => {
      const deafened = !get().deafened;
      for (const el of rt.audio.values()) el.muted = deafened;
      for (const el of rt.screenAudio.values()) el.muted = deafened;
      let muted: boolean;
      if (deafened) {
        rt.mutedBeforeDeafen = get().muted;
        muted = true;
      } else {
        muted = rt.mutedBeforeDeafen;
      }
      rt.stream?.getAudioTracks().forEach((t) => (t.enabled = !muted));
      set({ deafened, muted });
    },

    setVolume: (peer, volume) => {
      const v = Math.min(1, Math.max(0, volume));
      const el = rt.audio.get(peer);
      if (el) el.volume = v;
      const screen = rt.screenAudio.get(peer);
      if (screen) screen.volume = v;
      set((s) => ({ volumes: { ...s.volumes, [peer]: v } }));
    },

    dismissError: () => set({ error: null }),

    startShare: async (quality) => {
      if (get().activeId === null || !rt.mesh) return;
      set({ shareQuality: quality });
      try {
        localStorage.setItem(QUALITY_KEY, quality);
      } catch {
        // Not remembered; fine.
      }
      let stream: MediaStream;
      try {
        if (!navigator.mediaDevices?.getDisplayMedia) throw new Error("unsupported");
        stream = await openScreen(quality);
      } catch (e) {
        // Closing the picker is not an error.
        if (e instanceof DOMException && e.name === "NotAllowedError") return;
        set({ error: { code: "calls.screenFailed", detail: String(e), params: {} } });
        return;
      }
      const mesh = rt.mesh;
      if (get().activeId === null || !mesh) {
        stream.getTracks().forEach((t) => t.stop());
        return;
      }
      stopScreen();
      const video = stream.getVideoTracks()[0] ?? null;
      if (!video) {
        stream.getTracks().forEach((t) => t.stop());
        return;
      }
      video.contentHint = SCREEN_QUALITY[quality].fps >= 60 ? "motion" : "detail";
      const audio = stream.getAudioTracks()[0] ?? null;
      rt.screen = stream;
      // The browser's own "Stop sharing" ends the track.
      video.onended = () => get().stopShare();
      set({ sharing: quality, sharingAudio: audio !== null });
      await mesh.setScreen({ video, audio, quality });
    },

    stopShare: () => {
      if (!rt.screen) return;
      stopScreen();
      set({ sharing: null, sharingAudio: false });
      void rt.mesh?.setScreen(null);
    },
  };
});

/** Speaking state for a member of the active call (our own id included). */
export const isSpeaking = (id: string) => useCalls.getState().speaking[id] ?? false;
