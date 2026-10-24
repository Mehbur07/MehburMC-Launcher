/**
 * Full-mesh WebRTC audio for one call (K86): one RTCPeerConnection per other
 * member. Of each pair the later joiner (by `joinedAt`, then id) sends the
 * offer, so both sides agree without talking first. Every connection attempt
 * has a session tag; signals of an older attempt are ignored. A connection
 * that fails is rebuilt from scratch with a new session (no ICE restarts,
 * which would need both sides to keep the same DTLS state).
 *
 * Screen sharing (K89): every connection is set up with three transceivers
 * in a fixed order — microphone, screen video, screen audio — plus a
 * negotiated data channel for the sharing state, so starting or stopping a
 * share only swaps tracks (no renegotiation). Launchers before K89 offer the
 * microphone only; with them the extra transceivers and the channel simply
 * stay unused.
 */
import type { CallMember } from "../ipc/bindings/CallMember";
import type { CallSignal } from "../ipc/bindings/CallSignal";
import type { OutgoingSignal } from "../ipc/bindings/OutgoingSignal";
import type { SignalKind } from "../ipc/bindings/SignalKind";

export type PeerStatus = "connecting" | "connected" | "failed";

export type ScreenQuality = "720p30" | "1080p30" | "1080p60";

/** Capture size and the bitrate one viewer gets at most. */
export const SCREEN_QUALITY: Record<
  ScreenQuality,
  { width: number; height: number; fps: number; bitrate: number }
> = {
  "720p30": { width: 1280, height: 720, fps: 30, bitrate: 2_500_000 },
  "1080p30": { width: 1920, height: 1080, fps: 30, bitrate: 4_000_000 },
  "1080p60": { width: 1920, height: 1080, fps: 60, bitrate: 6_000_000 },
};

/** Upload for a screen share, split across the viewers (full mesh). */
const SCREEN_UPLOAD_BUDGET = 8_000_000;
const SCREEN_MIN_BITRATE = 600_000;

/** Per-viewer encoding: the budget split, smaller and slower when it gets thin. */
export function screenEncoding(quality: ScreenQuality, viewers: number) {
  const q = SCREEN_QUALITY[quality];
  const bitrate = Math.min(
    q.bitrate,
    Math.max(SCREEN_MIN_BITRATE, Math.floor(SCREEN_UPLOAD_BUDGET / Math.max(1, viewers))),
  );
  return {
    maxBitrate: bitrate,
    maxFramerate: bitrate < 2_500_000 ? Math.min(q.fps, 30) : q.fps,
    scaleResolutionDownBy: bitrate < 1_500_000 && q.height > 720 ? q.height / 720 : 1,
  };
}

export interface LocalScreen {
  video: MediaStreamTrack;
  /** System audio, when the user ticked it in the picker. */
  audio: MediaStreamTrack | null;
  quality: ScreenQuality;
}

export interface RemoteScreen {
  video: MediaStreamTrack;
  audio: MediaStreamTrack | null;
}

export interface MeshOptions {
  me: string;
  iceServers: RTCIceServer[];
  /** Relay through TURN only (hides our IP from the other members). */
  relayOnly: boolean;
  send: (signals: OutgoingSignal[]) => Promise<unknown>;
  onRemoteStream: (peer: string, stream: MediaStream | null) => void;
  onPeerStatus: (peer: string, status: PeerStatus | null) => void;
  /** A member started (tracks) or stopped (null) sharing their screen. */
  onRemoteScreen?: (peer: string, screen: RemoteScreen | null) => void;
  createPeer?: (config: RTCConfiguration) => RTCPeerConnection;
  /** ICE candidates are batched for this long. */
  flushMs?: number;
  /** An offerer rebuilds a connection that is not up after this long. */
  connectTimeoutMs?: number;
}

/** Rebuild attempts before a peer counts as failed (until they rejoin). */
const MAX_ATTEMPTS = 5;
const SIGNALS_PER_REQUEST = 50;

/** Transceiver order of every connection (K89). */
const MIC = 0;
const SCREEN_VIDEO = 1;
const SCREEN_AUDIO = 2;
const KINDS = ["audio", "video", "audio"] as const;

interface Peer {
  id: string;
  joinedAt: string;
  offerer: boolean;
  session: string;
  pc: RTCPeerConnection;
  attempts: number;
  timer: ReturnType<typeof setTimeout> | null;
  /** Sharing state channel; never opens with launchers before K89. */
  channel: RTCDataChannel | null;
  /** Their screen tracks (always there once negotiated) and whether they share. */
  remote: {
    video: MediaStreamTrack | null;
    audio: MediaStreamTrack | null;
    on: boolean;
    withAudio: boolean;
    shown: boolean;
  };
}

/** Sharing state sent on the data channel. */
interface ScreenMessage {
  t: "screen";
  on: boolean;
  audio: boolean;
}

interface Payload {
  s: string;
  type?: RTCSdpType;
  sdp?: string;
  candidate?: RTCIceCandidateInit;
}

const newSession = () => Math.random().toString(36).slice(2, 12);

/** Later joiner of the pair offers; ids break ties (both sides see the same strings). */
export function isOfferer(
  me: { id: string; joinedAt: string },
  other: { id: string; joinedAt: string },
): boolean {
  const a = Date.parse(me.joinedAt);
  const b = Date.parse(other.joinedAt);
  if (a !== b) return a > b;
  if (me.joinedAt !== other.joinedAt) return me.joinedAt > other.joinedAt;
  return me.id > other.id;
}

export class VoiceMesh {
  private readonly opts: Required<Omit<MeshOptions, "createPeer">> & {
    createPeer: (config: RTCConfiguration) => RTCPeerConnection;
  };
  private readonly peers = new Map<string, Peer>();
  /** Other joined members and when they joined (last sync). */
  private joined = new Map<string, string>();
  /** Joined members we wait on for an offer. */
  private readonly waiting = new Set<string>();
  /** Candidates that arrived before their session's offer/answer. */
  private readonly early = new Map<string, RTCIceCandidateInit[]>();
  private queue: OutgoingSignal[] = [];
  private flushTimer: ReturnType<typeof setTimeout> | null = null;
  private track: MediaStreamTrack | null = null;
  private screen: LocalScreen | null = null;
  private myJoinedAt: string | null = null;
  private closed = false;

  constructor(opts: MeshOptions) {
    this.opts = {
      flushMs: 120,
      connectTimeoutMs: 20000,
      createPeer: (config) => new RTCPeerConnection(config),
      onRemoteScreen: () => undefined,
      ...opts,
    };
  }

  /** The microphone track sent to everyone (null = listen only). */
  async setTrack(track: MediaStreamTrack | null) {
    this.track = track;
    await Promise.all([...this.peers.values()].map((p) => this.sendTrack(p, MIC)));
  }

  /** Starts (tracks) or stops (null) sharing our screen with everyone. */
  async setScreen(screen: LocalScreen | null) {
    this.screen = screen;
    await Promise.all(
      [...this.peers.values()].map(async (p) => {
        await this.sendTrack(p, SCREEN_VIDEO);
        await this.sendTrack(p, SCREEN_AUDIO);
        this.sendState(p);
      }),
    );
    await this.applyEncodings();
  }

  /** Applies the server's member list of this call. */
  sync(members: CallMember[]) {
    if (this.closed) return;
    const mine = members.find((m) => m.id === this.opts.me && m.state === "joined");
    if (!mine?.joinedAt) {
      this.dropAll();
      return;
    }
    if (mine.joinedAt !== this.myJoinedAt) {
      // We (re)joined: every connection starts over.
      this.dropAll();
      this.myJoinedAt = mine.joinedAt;
    }
    const others = new Map(
      members
        .filter((m) => m.state === "joined" && m.id !== this.opts.me && m.joinedAt)
        .map((m) => [m.id, m.joinedAt as string]),
    );
    this.joined = others;
    const before = this.peers.size;
    for (const [id, peer] of this.peers) {
      if (others.get(id) !== peer.joinedAt) this.dropPeer(id);
    }
    // Fewer viewers: the screen share may use more of the budget again.
    if (this.peers.size !== before) void this.applyEncodings();
    for (const id of this.waiting) {
      if (!others.has(id)) {
        this.waiting.delete(id);
        this.opts.onPeerStatus(id, null);
      }
    }
    for (const [id, joinedAt] of others) {
      if (this.peers.has(id)) continue;
      const me = { id: this.opts.me, joinedAt: this.myJoinedAt as string };
      if (isOfferer(me, { id, joinedAt })) {
        void this.offer(id, joinedAt, 0);
      } else if (!this.waiting.has(id)) {
        this.waiting.add(id);
        this.opts.onPeerStatus(id, "connecting");
      }
    }
  }

  /** A set-up message from another member of this call. */
  async handle(signal: CallSignal) {
    if (this.closed) return;
    let data: Payload;
    try {
      data = JSON.parse(signal.payload) as Payload;
    } catch {
      return;
    }
    if (typeof data?.s !== "string") return;
    try {
      if (signal.kind === "offer") await this.onOffer(signal.sender, data);
      else if (signal.kind === "answer") await this.onAnswer(signal.sender, data);
      else await this.onIce(signal.sender, data);
    } catch {
      // A broken message must not stop the others; the connect timer retries.
    }
  }

  /** Peers that are set up or awaited. */
  peerIds(): string[] {
    return [...new Set([...this.peers.keys(), ...this.waiting])];
  }

  close() {
    this.closed = true;
    this.dropAll();
    if (this.flushTimer) clearTimeout(this.flushTimer);
    this.queue = [];
  }

  // ------------------------------------------------------------- internals

  private createPc(id: string, session: string): RTCPeerConnection {
    const pc = this.opts.createPeer({
      iceServers: this.opts.iceServers,
      iceTransportPolicy: this.opts.relayOnly ? "relay" : "all",
    });
    pc.onicecandidate = (e) => {
      if (e.candidate) this.enqueue(id, "ice", { s: session, candidate: e.candidate.toJSON() });
    };
    pc.ontrack = (e) => {
      const peer = this.peers.get(id);
      if (peer?.session !== session) return;
      // The role is the transceiver's place (both sides keep the offer's order).
      const role = e.transceiver ? pc.getTransceivers().indexOf(e.transceiver) : MIC;
      if (role === SCREEN_VIDEO || role === SCREEN_AUDIO) {
        if (role === SCREEN_VIDEO) peer.remote.video = e.track;
        else peer.remote.audio = e.track;
        this.emitScreen(peer);
        return;
      }
      if (role !== MIC && role !== -1) return;
      this.opts.onRemoteStream(id, e.streams[0] ?? new MediaStream([e.track]));
    };
    pc.onconnectionstatechange = () => this.onState(id, session);
    return pc;
  }

  private newPeer(
    id: string,
    joinedAt: string,
    offerer: boolean,
    session: string,
    attempts: number,
  ): Peer {
    const pc = this.createPc(id, session);
    const peer: Peer = {
      id,
      joinedAt,
      offerer,
      session,
      pc,
      attempts,
      timer: null,
      channel: null,
      remote: { video: null, audio: null, on: false, withAudio: false, shown: false },
    };
    this.peers.set(id, peer);
    return peer;
  }

  /** The negotiated channel both sides open with the same id, before the answer. */
  private openChannel(peer: Peer) {
    let ch: RTCDataChannel;
    try {
      ch = peer.pc.createDataChannel("screen", { negotiated: true, id: 0 });
    } catch {
      return;
    }
    peer.channel = ch;
    ch.onopen = () => this.sendState(peer);
    ch.onmessage = (e) => {
      if (this.peers.get(peer.id) !== peer || typeof e.data !== "string") return;
      let msg: ScreenMessage;
      try {
        msg = JSON.parse(e.data) as ScreenMessage;
      } catch {
        return;
      }
      if (msg?.t !== "screen") return;
      peer.remote.on = msg.on === true;
      peer.remote.withAudio = msg.audio === true;
      this.emitScreen(peer);
    };
    ch.onclose = () => {
      if (this.peers.get(peer.id) !== peer) return;
      peer.remote.on = false;
      this.emitScreen(peer);
    };
  }

  private sendState(peer: Peer) {
    if (peer.channel?.readyState !== "open") return;
    const msg: ScreenMessage = {
      t: "screen",
      on: this.screen !== null,
      audio: this.screen?.audio != null,
    };
    try {
      peer.channel.send(JSON.stringify(msg));
    } catch {
      // Closing; the other side sees the channel close.
    }
  }

  /** Tells the store about a remote share only when it changes. */
  private emitScreen(peer: Peer) {
    const r = peer.remote;
    const show = r.on && r.video !== null;
    if (!show && !r.shown) return;
    r.shown = show;
    this.opts.onRemoteScreen(
      peer.id,
      show ? { video: r.video as MediaStreamTrack, audio: r.withAudio ? r.audio : null } : null,
    );
  }

  /** What our transceiver `role` sends. */
  private trackFor(role: number): MediaStreamTrack | null {
    if (role === MIC) return this.track;
    if (role === SCREEN_VIDEO) return this.screen?.video ?? null;
    if (role === SCREEN_AUDIO) return this.screen?.audio ?? null;
    return null;
  }

  private async sendTrack(peer: Peer, role: number) {
    const t = peer.pc.getTransceivers()[role];
    if (!t) return;
    // A launcher before K89 may offer something else in that place.
    const kind = t.receiver?.track?.kind;
    if (kind && kind !== KINDS[role]) return;
    await t.sender.replaceTrack(this.trackFor(role)).catch(() => undefined);
  }

  /** Bitrate, frame rate and size of the screen video for each viewer. */
  private async applyEncodings() {
    const screen = this.screen;
    if (!screen) return;
    const enc = screenEncoding(screen.quality, this.peers.size);
    await Promise.all(
      [...this.peers.values()].map(async (p) => {
        const sender = p.pc.getTransceivers()[SCREEN_VIDEO]?.sender;
        if (!sender?.getParameters) return;
        const params = sender.getParameters();
        if (!params.encodings?.length) return; // Not negotiated yet; "connected" retries.
        params.encodings = params.encodings.map((e) => ({ ...e, ...enc }));
        // Sharp text and HUD; frames drop first when the network is thin. With
        // maintain-framerate (even at 60 FPS) Chromium stayed at 480p in tests.
        params.degradationPreference = "maintain-resolution";
        await sender.setParameters(params).catch(() => undefined);
      }),
    );
  }

  private async offer(id: string, joinedAt: string, attempts: number) {
    this.waiting.delete(id);
    const session = newSession();
    const peer = this.newPeer(id, joinedAt, true, session, attempts);
    const pc = peer.pc;
    this.opts.onPeerStatus(id, "connecting");
    this.armTimer(peer);
    for (const kind of KINDS) pc.addTransceiver(kind, { direction: "sendrecv" });
    for (let role = 0; role < KINDS.length; role++) await this.sendTrack(peer, role);
    this.openChannel(peer);
    const desc = await pc.createOffer();
    if (this.peers.get(id) !== peer) return;
    await pc.setLocalDescription(desc);
    this.enqueue(id, "offer", { s: session, type: desc.type, sdp: desc.sdp });
  }

  private async onOffer(from: string, data: Payload) {
    const joinedAt = this.joined.get(from);
    // Not (yet) in the call as we know it, or we are the offerer of this
    // pair: the offer is stale or out of turn.
    if (!joinedAt || !this.myJoinedAt) return;
    if (isOfferer({ id: this.opts.me, joinedAt: this.myJoinedAt }, { id: from, joinedAt })) return;
    const known = this.peers.get(from);
    if (known) this.dropPeer(from, false);
    this.waiting.delete(from);
    const peer = this.newPeer(from, joinedAt, false, data.s, 0);
    const pc = peer.pc;
    this.opts.onPeerStatus(from, "connecting");
    await pc.setRemoteDescription({ type: data.type ?? "offer", sdp: data.sdp });
    const transceivers = pc.getTransceivers();
    for (let role = 0; role < transceivers.length; role++) {
      transceivers[role]!.direction = "sendrecv";
      await this.sendTrack(peer, role);
    }
    this.openChannel(peer);
    await this.flushEarly(peer);
    const answer = await pc.createAnswer();
    if (this.peers.get(from) !== peer) return;
    await pc.setLocalDescription(answer);
    this.enqueue(from, "answer", { s: data.s, type: answer.type, sdp: answer.sdp });
  }

  private async onAnswer(from: string, data: Payload) {
    const peer = this.peers.get(from);
    if (!peer || !peer.offerer || peer.session !== data.s) return;
    if (peer.pc.signalingState !== "have-local-offer") return;
    await peer.pc.setRemoteDescription({ type: data.type ?? "answer", sdp: data.sdp });
    await this.flushEarly(peer);
  }

  private async onIce(from: string, data: Payload) {
    if (!data.candidate) return;
    const peer = this.peers.get(from);
    if (peer && peer.session === data.s && peer.pc.remoteDescription) {
      await peer.pc.addIceCandidate(data.candidate).catch(() => undefined);
      return;
    }
    const key = `${from}/${data.s}`;
    const list = this.early.get(key) ?? [];
    if (list.length < 100) list.push(data.candidate);
    this.early.set(key, list);
  }

  private async flushEarly(peer: Peer) {
    const key = `${peer.id}/${peer.session}`;
    const list = this.early.get(key) ?? [];
    this.early.delete(key);
    for (const c of list) await peer.pc.addIceCandidate(c).catch(() => undefined);
  }

  private onState(id: string, session: string) {
    const peer = this.peers.get(id);
    if (!peer || peer.session !== session) return;
    const state = peer.pc.connectionState;
    if (state === "connected") {
      if (peer.timer) clearTimeout(peer.timer);
      peer.timer = null;
      peer.attempts = 0;
      this.opts.onPeerStatus(id, "connected");
      // Encodings exist only now; a share running already gets its limits.
      void this.applyEncodings();
    } else if (state === "failed") {
      this.retry(peer);
    } else if (state === "disconnected") {
      this.opts.onPeerStatus(id, "connecting");
      // Often recovers by itself; rebuild if it does not.
      this.armTimer(peer, Math.min(8000, this.opts.connectTimeoutMs));
    }
  }

  private armTimer(peer: Peer, ms = this.opts.connectTimeoutMs) {
    if (peer.timer) clearTimeout(peer.timer);
    peer.timer = setTimeout(() => {
      const now = this.peers.get(peer.id);
      if (now === peer && peer.pc.connectionState !== "connected") this.retry(peer);
    }, ms);
  }

  /** Offerer: new connection; answerer: wait for the offerer's next offer. */
  private retry(peer: Peer) {
    if (peer.offerer) {
      const attempts = peer.attempts + 1;
      this.dropPeer(peer.id, false);
      if (attempts >= MAX_ATTEMPTS) {
        this.opts.onPeerStatus(peer.id, "failed");
        return;
      }
      void this.offer(peer.id, peer.joinedAt, attempts);
    } else {
      this.dropPeer(peer.id, false);
      this.waiting.add(peer.id);
      this.opts.onPeerStatus(peer.id, "connecting");
    }
  }

  private dropPeer(id: string, report = true) {
    const peer = this.peers.get(id);
    if (peer) {
      if (peer.timer) clearTimeout(peer.timer);
      peer.pc.onicecandidate = null;
      peer.pc.ontrack = null;
      peer.pc.onconnectionstatechange = null;
      if (peer.channel) {
        peer.channel.onopen = null;
        peer.channel.onmessage = null;
        peer.channel.onclose = null;
      }
      peer.pc.close();
      this.peers.delete(id);
      this.opts.onRemoteStream(id, null);
      if (peer.remote.shown) this.opts.onRemoteScreen(id, null);
    }
    for (const key of this.early.keys()) if (key.startsWith(`${id}/`)) this.early.delete(key);
    this.queue = this.queue.filter((s) => s.recipient !== id);
    if (report) {
      this.waiting.delete(id);
      this.opts.onPeerStatus(id, null);
    }
  }

  private dropAll() {
    for (const id of [...this.peers.keys()]) this.dropPeer(id);
    for (const id of [...this.waiting]) {
      this.waiting.delete(id);
      this.opts.onPeerStatus(id, null);
    }
    this.myJoinedAt = null;
    this.joined = new Map();
  }

  private enqueue(recipient: string, kind: SignalKind, payload: Payload) {
    const signal: OutgoingSignal = { recipient, kind, payload: JSON.stringify(payload) };
    if (kind === "ice") {
      this.queue.push(signal);
      this.flushTimer ??= setTimeout(() => void this.flush(), this.opts.flushMs);
    } else {
      // Descriptions go first and right away; candidates follow them.
      this.queue.unshift(signal);
      void this.flush();
    }
  }

  private async flush() {
    if (this.flushTimer) clearTimeout(this.flushTimer);
    this.flushTimer = null;
    const batch = this.queue;
    this.queue = [];
    for (let i = 0; i < batch.length; i += SIGNALS_PER_REQUEST) {
      await this.opts.send(batch.slice(i, i + SIGNALS_PER_REQUEST)).catch(() => undefined);
    }
  }
}
