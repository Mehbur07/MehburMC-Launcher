/**
 * Full-mesh WebRTC audio for one call (K86): one RTCPeerConnection per other
 * member. Of each pair the later joiner (by `joinedAt`, then id) sends the
 * offer, so both sides agree without talking first. Every connection attempt
 * has a session tag; signals of an older attempt are ignored. A connection
 * that fails is rebuilt from scratch with a new session (no ICE restarts,
 * which would need both sides to keep the same DTLS state).
 */
import type { CallMember } from "../ipc/bindings/CallMember";
import type { CallSignal } from "../ipc/bindings/CallSignal";
import type { OutgoingSignal } from "../ipc/bindings/OutgoingSignal";
import type { SignalKind } from "../ipc/bindings/SignalKind";

export type PeerStatus = "connecting" | "connected" | "failed";

export interface MeshOptions {
  me: string;
  iceServers: RTCIceServer[];
  /** Relay through TURN only (hides our IP from the other members). */
  relayOnly: boolean;
  send: (signals: OutgoingSignal[]) => Promise<unknown>;
  onRemoteStream: (peer: string, stream: MediaStream | null) => void;
  onPeerStatus: (peer: string, status: PeerStatus | null) => void;
  createPeer?: (config: RTCConfiguration) => RTCPeerConnection;
  /** ICE candidates are batched for this long. */
  flushMs?: number;
  /** An offerer rebuilds a connection that is not up after this long. */
  connectTimeoutMs?: number;
}

/** Rebuild attempts before a peer counts as failed (until they rejoin). */
const MAX_ATTEMPTS = 5;
const SIGNALS_PER_REQUEST = 50;

interface Peer {
  id: string;
  joinedAt: string;
  offerer: boolean;
  session: string;
  pc: RTCPeerConnection;
  attempts: number;
  timer: ReturnType<typeof setTimeout> | null;
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
  private myJoinedAt: string | null = null;
  private closed = false;

  constructor(opts: MeshOptions) {
    this.opts = {
      flushMs: 120,
      connectTimeoutMs: 20000,
      createPeer: (config) => new RTCPeerConnection(config),
      ...opts,
    };
  }

  /** The microphone track sent to everyone (null = listen only). */
  async setTrack(track: MediaStreamTrack | null) {
    this.track = track;
    await Promise.all(
      [...this.peers.values()].map((p) =>
        Promise.all(
          p.pc.getTransceivers().map((t) => t.sender.replaceTrack(track).catch(() => undefined)),
        ),
      ),
    );
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
    for (const [id, peer] of this.peers) {
      if (others.get(id) !== peer.joinedAt) this.dropPeer(id);
    }
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
      if (this.peers.get(id)?.session !== session) return;
      this.opts.onRemoteStream(id, e.streams[0] ?? new MediaStream([e.track]));
    };
    pc.onconnectionstatechange = () => this.onState(id, session);
    return pc;
  }

  private async offer(id: string, joinedAt: string, attempts: number) {
    this.waiting.delete(id);
    const session = newSession();
    const pc = this.createPc(id, session);
    const peer: Peer = { id, joinedAt, offerer: true, session, pc, attempts, timer: null };
    this.peers.set(id, peer);
    this.opts.onPeerStatus(id, "connecting");
    this.armTimer(peer);
    const t = pc.addTransceiver("audio", { direction: "sendrecv" });
    await t.sender.replaceTrack(this.track).catch(() => undefined);
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
    const pc = this.createPc(from, data.s);
    const peer: Peer = {
      id: from,
      joinedAt,
      offerer: false,
      session: data.s,
      pc,
      attempts: 0,
      timer: null,
    };
    this.peers.set(from, peer);
    this.opts.onPeerStatus(from, "connecting");
    await pc.setRemoteDescription({ type: data.type ?? "offer", sdp: data.sdp });
    for (const t of pc.getTransceivers()) {
      t.direction = "sendrecv";
      await t.sender.replaceTrack(this.track).catch(() => undefined);
    }
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
      peer.pc.close();
      this.peers.delete(id);
      this.opts.onRemoteStream(id, null);
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
