import { create } from "zustand";

import { ipc, subscribeCalls, toErrorPayload } from "../lib/ipc";
import type { Call } from "../lib/ipc/bindings/Call";
import type { CallPoll } from "../lib/ipc/bindings/CallPoll";
import type { CallSignal } from "../lib/ipc/bindings/CallSignal";
import type { ErrorPayload } from "../lib/ipc/bindings/ErrorPayload";
import type { IceConfig } from "../lib/ipc/bindings/IceConfig";
import { watchSpeaking } from "../lib/voice/audio";
import { type PeerStatus, VoiceMesh } from "../lib/voice/mesh";
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
}

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
      set((s) => ({ volumes: { ...s.volumes, [peer]: v } }));
    },

    dismissError: () => set({ error: null }),
  };
});

/** Speaking state for a member of the active call (our own id included). */
export const isSpeaking = (id: string) => useCalls.getState().speaking[id] ?? false;
