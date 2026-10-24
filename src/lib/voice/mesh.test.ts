import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { CallMember } from "../ipc/bindings/CallMember";
import type { CallSignal } from "../ipc/bindings/CallSignal";
import type { OutgoingSignal } from "../ipc/bindings/OutgoingSignal";
import {
  isOfferer,
  type MeshOptions,
  type PeerStatus,
  type RemoteScreen,
  screenEncoding,
  VoiceMesh,
} from "./mesh";

interface FakeTransceiver {
  direction: string;
  receiver: { track: { kind: string } };
  sender: {
    track: unknown;
    replaceTrack: (t: unknown) => Promise<void>;
    params: { encodings: Record<string, unknown>[]; degradationPreference?: string };
    getParameters: () => FakeTransceiver["sender"]["params"];
    setParameters: (p: FakeTransceiver["sender"]["params"]) => Promise<void>;
  };
}

/** Negotiated data channel; delivers to its twin on the other connection. */
class FakeChannel {
  readyState: RTCDataChannelState = "connecting";
  twin: FakeChannel | null = null;
  onopen: (() => void) | null = null;
  onmessage: ((e: { data: string }) => void) | null = null;
  onclose: (() => void) | null = null;
  send(data: string) {
    if (this.readyState !== "open") throw new Error("not open");
    if (this.twin?.readyState === "open") this.twin.onmessage?.({ data });
  }
}

/**
 * Just enough of RTCPeerConnection: connects once both descriptions and a
 * candidate are in. The fake SDP carries the offer's transceiver kinds and
 * whether it has a data channel, so the answer side mirrors them.
 */
class FakePC {
  static all: FakePC[] = [];
  static autoConnect = true;
  signalingState: RTCSignalingState = "stable";
  connectionState: RTCPeerConnectionState = "new";
  localDescription: RTCSessionDescriptionInit | null = null;
  remoteDescription: RTCSessionDescriptionInit | null = null;
  transceivers: FakeTransceiver[] = [];
  channel: FakeChannel | null = null;
  /** Index of the connection at the other end (from the fake SDP). */
  other = -1;
  remoteChannel = false;
  candidates: RTCIceCandidateInit[] = [];
  onicecandidate: ((e: { candidate: { toJSON(): RTCIceCandidateInit } | null }) => void) | null =
    null;
  ontrack:
    ((e: { streams: unknown[]; track: unknown; transceiver: FakeTransceiver }) => void) | null =
    null;
  onconnectionstatechange: (() => void) | null = null;
  closed = false;
  constructor(public config: RTCConfiguration) {
    FakePC.all.push(this);
  }
  private transceiver(kind: string, direction: string) {
    const t: FakeTransceiver = {
      direction,
      receiver: { track: { kind } },
      sender: {
        track: null,
        replaceTrack: async (track: unknown) => {
          t.sender.track = track;
        },
        params: { encodings: [] },
        getParameters: () => ({ ...t.sender.params }),
        setParameters: async (p) => {
          t.sender.params = p;
        },
      },
    };
    this.transceivers.push(t);
    return t;
  }
  addTransceiver(kind: string, init: { direction: string }) {
    return this.transceiver(kind, init.direction);
  }
  getTransceivers() {
    return this.transceivers;
  }
  createDataChannel() {
    this.channel = new FakeChannel();
    return this.channel;
  }
  private sdp(type: string) {
    const kinds = this.transceivers.map((t) => t.receiver.track.kind).join(",");
    return `${type}-${FakePC.all.indexOf(this)}|${kinds}|${this.channel ? "dc" : ""}`;
  }
  async createOffer() {
    return { type: "offer" as const, sdp: this.sdp("offer") };
  }
  async createAnswer() {
    return { type: "answer" as const, sdp: this.sdp("answer") };
  }
  async setLocalDescription(d: RTCSessionDescriptionInit) {
    this.localDescription = d;
    this.signalingState = d.type === "offer" ? "have-local-offer" : "stable";
    for (const t of this.transceivers) t.sender.params = { encodings: [{}] };
    queueMicrotask(() =>
      this.onicecandidate?.({
        candidate: {
          toJSON: () => ({ candidate: `cand-${FakePC.all.indexOf(this)}`, sdpMid: "0" }),
        },
      }),
    );
    this.maybeConnect();
  }
  async setRemoteDescription(d: RTCSessionDescriptionInit) {
    this.remoteDescription = d;
    const [head = "", kinds = "", dc = ""] = (d.sdp ?? "").split("|");
    this.other = Number(head.split("-")[1] ?? -1);
    this.remoteChannel = dc === "dc";
    if (d.type === "offer") {
      this.signalingState = "have-remote-offer";
      const list = kinds ? kinds.split(",") : ["audio"];
      if (this.transceivers.length === 0) for (const k of list) this.transceiver(k, "recvonly");
    } else {
      this.signalingState = "stable";
    }
    this.maybeConnect();
  }
  async addIceCandidate(c: RTCIceCandidateInit) {
    this.candidates.push(c);
    this.maybeConnect();
  }
  maybeConnect() {
    if (
      FakePC.autoConnect &&
      !this.closed &&
      this.localDescription &&
      this.remoteDescription &&
      this.candidates.length > 0 &&
      this.connectionState !== "connected"
    ) {
      this.connectionState = "connected";
      this.transceivers.forEach((t, i) =>
        this.ontrack?.({
          streams: i === 0 ? [{ id: "remote" }] : [],
          track: t.receiver.track,
          transceiver: t,
        }),
      );
      this.onconnectionstatechange?.();
      // Both ends negotiated a channel: open the pair.
      const other = FakePC.all[this.other];
      if (
        this.channel &&
        this.remoteChannel &&
        other?.channel &&
        other.connectionState === "connected"
      ) {
        this.channel.twin = other.channel;
        other.channel.twin = this.channel;
        // Like SCTP: both ends are open before either hears about it.
        other.channel.readyState = "open";
        this.channel.readyState = "open";
        other.channel.onopen?.();
        this.channel.onopen?.();
      }
    }
  }
  fail() {
    this.connectionState = "failed";
    this.onconnectionstatechange?.();
  }
  close() {
    this.closed = true;
    this.connectionState = "closed";
    if (this.channel) {
      this.channel.readyState = "closed";
      const twin = this.channel.twin;
      if (twin && twin.readyState === "open") {
        twin.readyState = "closed";
        twin.onclose?.();
      }
    }
  }
}

const T0 = "2026-10-21T10:00:00.1+00:00";
const T1 = "2026-10-21T10:00:05.25+00:00";
const T2 = "2026-10-21T10:00:09+00:00";

const member = (
  id: string,
  joinedAt: string | null,
  state: CallMember["state"] = "joined",
): CallMember => ({
  id,
  displayName: id,
  nickname: null,
  state,
  joinedAt,
});

/** Meshes wired to each other through an in-memory "server". */
function network(ids: string[], opts: Partial<MeshOptions> = {}) {
  let seq = 0;
  const meshes = new Map<string, VoiceMesh>();
  const status = new Map<string, Map<string, PeerStatus | null>>();
  const streams = new Map<string, Map<string, unknown>>();
  const screens = new Map<string, Map<string, RemoteScreen | null>>();
  const sent: { from: string; batch: OutgoingSignal[] }[] = [];
  const hold: { on: boolean; queue: CallSignal[] } = { on: false, queue: [] };
  for (const id of ids) {
    status.set(id, new Map());
    streams.set(id, new Map());
    screens.set(id, new Map());
    meshes.set(
      id,
      new VoiceMesh({
        me: id,
        iceServers: [],
        relayOnly: false,
        flushMs: 0,
        createPeer: (c) => new FakePC(c) as unknown as RTCPeerConnection,
        onPeerStatus: (p, s) => status.get(id)!.set(p, s),
        onRemoteStream: (p, s) => streams.get(id)!.set(p, s),
        onRemoteScreen: (p, s) => screens.get(id)!.set(p, s),
        send: async (batch) => {
          sent.push({ from: id, batch });
          for (const s of batch) {
            const sig: CallSignal = {
              id: ++seq,
              callId: 1,
              sender: id,
              kind: s.kind,
              payload: s.payload,
            };
            if (hold.on) hold.queue.push({ ...sig, sender: `${id}>${s.recipient}` });
            else await meshes.get(s.recipient)?.handle(sig);
          }
        },
        ...opts,
      }),
    );
  }
  return { meshes, status, streams, screens, sent, hold };
}

describe("isOfferer", () => {
  it("lets the later joiner offer and breaks ties by id", () => {
    expect(isOfferer({ id: "a", joinedAt: T1 }, { id: "b", joinedAt: T0 })).toBe(true);
    expect(isOfferer({ id: "a", joinedAt: T0 }, { id: "b", joinedAt: T1 })).toBe(false);
    expect(isOfferer({ id: "b", joinedAt: T0 }, { id: "a", joinedAt: T0 })).toBe(true);
    expect(isOfferer({ id: "a", joinedAt: T0 }, { id: "b", joinedAt: T0 })).toBe(false);
  });
});

describe("VoiceMesh", () => {
  beforeEach(() => {
    FakePC.all = [];
    FakePC.autoConnect = true;
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("connects two members; only the later joiner offers", async () => {
    const { meshes, status, streams, sent } = network(["a", "b"]);
    const members = [member("a", T0), member("b", T1)];
    meshes.get("a")!.sync(members);
    meshes.get("b")!.sync(members);
    await vi.waitFor(() => {
      expect(status.get("a")!.get("b")).toBe("connected");
      expect(status.get("b")!.get("a")).toBe("connected");
    });
    const offers = sent.flatMap((s) => s.batch.map((x) => ({ from: s.from, kind: x.kind })));
    expect(offers.filter((o) => o.kind === "offer")).toEqual([{ from: "b", kind: "offer" }]);
    expect(offers.filter((o) => o.kind === "answer")).toEqual([{ from: "a", kind: "answer" }]);
    expect(streams.get("a")!.get("b")).toBeTruthy();
    expect(FakePC.all).toHaveLength(2);
  });

  it("sends our microphone and swaps it on every connection", async () => {
    const { meshes, status } = network(["a", "b"]);
    const mic = { id: "mic" } as unknown as MediaStreamTrack;
    await meshes.get("b")!.setTrack(mic);
    const members = [member("a", T0), member("b", T1)];
    meshes.get("a")!.sync(members);
    meshes.get("b")!.sync(members);
    await vi.waitFor(() => expect(status.get("a")!.get("b")).toBe("connected"));
    const offerer = FakePC.all[0]!;
    expect(offerer.transceivers[0]!.sender.track).toBe(mic);
    const other = { id: "mic2" } as unknown as MediaStreamTrack;
    await meshes.get("b")!.setTrack(other);
    expect(offerer.transceivers[0]!.sender.track).toBe(other);
  });

  it("uses relay-only ICE when asked to hide the IP", () => {
    const { meshes } = network(["a", "b"], { relayOnly: true });
    meshes.get("b")!.sync([member("a", T0), member("b", T1)]);
    expect(FakePC.all[0]!.config.iceTransportPolicy).toBe("relay");
  });

  it("ignores an offer out of turn and signals of an older session", async () => {
    const { meshes, status } = network(["a", "b"]);
    const a = meshes.get("a")!;
    a.sync([member("a", T0), member("b", T1)]);
    // "a" joined first, so an offer *from a* to b is out of turn for b…
    const b = meshes.get("b")!;
    b.sync([member("a", T0), member("b", T1)]);
    await vi.waitFor(() => expect(status.get("b")!.get("a")).toBe("connected"));
    const before = FakePC.all.length;
    await b.handle({
      id: 99,
      callId: 1,
      sender: "a",
      kind: "offer",
      payload: JSON.stringify({ s: "x", type: "offer", sdp: "rogue" }),
    });
    expect(FakePC.all.length).toBe(before);
    // …and an answer with the wrong session tag changes nothing.
    await b.handle({
      id: 100,
      callId: 1,
      sender: "a",
      kind: "answer",
      payload: JSON.stringify({ s: "old", type: "answer", sdp: "stale" }),
    });
    expect(FakePC.all[0]!.remoteDescription?.sdp).not.toBe("stale");
    // Garbage is ignored.
    await b.handle({ id: 101, callId: 1, sender: "a", kind: "ice", payload: "{nope" });
    expect(status.get("b")!.get("a")).toBe("connected");
  });

  it("does not apply an answer of another session while the offer waits", async () => {
    const { meshes, hold } = network(["a", "b"]);
    hold.on = true;
    const b = meshes.get("b")!;
    b.sync([member("a", T0), member("b", T1)]);
    await vi.waitFor(() => expect(FakePC.all[0]?.signalingState).toBe("have-local-offer"));
    await b.handle({
      id: 50,
      callId: 1,
      sender: "a",
      kind: "answer",
      payload: JSON.stringify({ s: "older-session", type: "answer", sdp: "stale" }),
    });
    expect(FakePC.all[0]!.remoteDescription).toBeNull();
    expect(FakePC.all[0]!.signalingState).toBe("have-local-offer");
  });

  it("keeps candidates that arrive before their offer", async () => {
    const { meshes, status, hold } = network(["a", "b"]);
    hold.on = true;
    meshes.get("a")!.sync([member("a", T0), member("b", T1)]);
    meshes.get("b")!.sync([member("a", T0), member("b", T1)]);
    await vi.waitFor(() => expect(hold.queue.some((s) => s.kind === "ice")).toBe(true));
    hold.on = false;
    // Deliver b's candidate first, then the offer.
    const toA = hold.queue.filter((s) => s.sender === "b>a");
    const ordered = [
      ...toA.filter((s) => s.kind === "ice"),
      ...toA.filter((s) => s.kind !== "ice"),
    ];
    for (const s of ordered) await meshes.get("a")!.handle({ ...s, sender: "b" });
    await vi.waitFor(() => expect(FakePC.all[1]!.candidates).toHaveLength(1));
    expect(status.get("a")!.get("b")).not.toBe("failed");
  });

  it("rebuilds a connection that does not come up, then gives up", async () => {
    vi.useFakeTimers();
    FakePC.autoConnect = false;
    const { meshes, status } = network(["a", "b"], { connectTimeoutMs: 1000 });
    const b = meshes.get("b")!;
    b.sync([member("a", T0), member("b", T1)]);
    await vi.advanceTimersByTimeAsync(0);
    expect(FakePC.all.filter((p) => p.localDescription?.type === "offer")).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1000);
    // Old connection closed, a new offer with a new session went out.
    const offers = FakePC.all.filter((p) => p.localDescription?.type === "offer");
    expect(offers).toHaveLength(2);
    expect(offers[0]!.closed).toBe(true);
    await vi.advanceTimersByTimeAsync(5000);
    expect(status.get("b")!.get("a")).toBe("failed");
    const count = FakePC.all.length;
    await vi.advanceTimersByTimeAsync(10000);
    expect(FakePC.all.length).toBe(count);
  });

  it("rebuilds right away when a connection fails", async () => {
    const { meshes, status } = network(["a", "b"]);
    meshes.get("a")!.sync([member("a", T0), member("b", T1)]);
    meshes.get("b")!.sync([member("a", T0), member("b", T1)]);
    await vi.waitFor(() => expect(status.get("b")!.get("a")).toBe("connected"));
    FakePC.all[0]!.fail();
    await vi.waitFor(() => {
      expect(FakePC.all.length).toBe(4);
      expect(status.get("b")!.get("a")).toBe("connected");
      expect(status.get("a")!.get("b")).toBe("connected");
    });
  });

  it("drops members who left and starts over after we rejoin", async () => {
    const { meshes, status, streams } = network(["a", "b"]);
    const a = meshes.get("a")!;
    const b = meshes.get("b")!;
    a.sync([member("a", T0), member("b", T1)]);
    b.sync([member("a", T0), member("b", T1)]);
    await vi.waitFor(() => expect(status.get("a")!.get("b")).toBe("connected"));
    // b left.
    a.sync([member("a", T0), member("b", T1, "left")]);
    expect(status.get("a")!.get("b")).toBeNull();
    expect(streams.get("a")!.get("b")).toBeNull();
    expect(FakePC.all[1]!.closed).toBe(true);
    expect(a.peerIds()).toEqual([]);
    // a rejoined (new joinedAt) and is now the later joiner: a offers.
    const before = FakePC.all.length;
    a.sync([member("a", T2), member("b", T1)]);
    b.sync([member("a", T2), member("b", T1)]);
    await vi.waitFor(() => expect(status.get("b")!.get("a")).toBe("connected"));
    expect(FakePC.all[before]!.localDescription?.type).toBe("offer");
    // Without our own joined row, everything is torn down.
    a.sync([member("b", T1)]);
    expect(a.peerIds()).toEqual([]);
  });

  it("builds a full mesh of three", async () => {
    const { meshes, status } = network(["a", "b", "c"]);
    const members = [member("a", T0), member("b", T1), member("c", T2)];
    for (const m of meshes.values()) m.sync(members);
    await vi.waitFor(() => {
      for (const [id, s] of status) {
        for (const other of ["a", "b", "c"].filter((x) => x !== id)) {
          expect(s.get(other), `${id}→${other}`).toBe("connected");
        }
      }
    });
    expect(FakePC.all).toHaveLength(6);
  });

  it("splits large batches into requests of 50", async () => {
    const batches: number[] = [];
    const mesh = new VoiceMesh({
      me: "a",
      iceServers: [],
      relayOnly: false,
      flushMs: 5,
      createPeer: (c) => new FakePC(c) as unknown as RTCPeerConnection,
      onPeerStatus: () => undefined,
      onRemoteStream: () => undefined,
      send: async (b) => {
        batches.push(b.length);
      },
    });
    mesh.sync([member("b", T0), member("a", T1)]);
    await vi.waitFor(() => expect(FakePC.all[0]?.onicecandidate).toBeTruthy());
    const pc = FakePC.all[0]!;
    await vi.waitFor(() => expect(batches.length).toBeGreaterThan(0));
    batches.length = 0;
    for (let i = 0; i < 120; i++)
      pc.onicecandidate?.({ candidate: { toJSON: () => ({ candidate: `c${i}` }) } });
    await vi.waitFor(() => expect(batches.reduce((x, y) => x + y, 0)).toBe(120));
    expect(Math.max(...batches)).toBeLessThanOrEqual(50);
    mesh.close();
  });

  it("sets up mic, screen video and screen audio plus a channel on every connection", async () => {
    const { meshes, status } = network(["a", "b"]);
    const members = [member("a", T0), member("b", T1)];
    meshes.get("a")!.sync(members);
    meshes.get("b")!.sync(members);
    await vi.waitFor(() => expect(status.get("a")!.get("b")).toBe("connected"));
    for (const pc of FakePC.all) {
      expect(pc.transceivers.map((t) => t.receiver.track.kind)).toEqual([
        "audio",
        "video",
        "audio",
      ]);
      expect(pc.transceivers.every((t) => t.direction === "sendrecv")).toBe(true);
      expect(pc.channel?.readyState).toBe("open");
    }
  });

  it("shares and stops a screen without renegotiating", async () => {
    const { meshes, status, screens, streams, sent } = network(["a", "b"]);
    const members = [member("a", T0), member("b", T1)];
    meshes.get("a")!.sync(members);
    meshes.get("b")!.sync(members);
    await vi.waitFor(() => expect(status.get("b")!.get("a")).toBe("connected"));
    const video = { id: "screen" } as unknown as MediaStreamTrack;
    const audio = { id: "sys" } as unknown as MediaStreamTrack;
    // "a" answered, so its transceivers came from b's offer.
    await meshes.get("a")!.setScreen({ video, audio, quality: "1080p60" });
    const answerer = FakePC.all[1]!;
    expect(answerer.transceivers[1]!.sender.track).toBe(video);
    expect(answerer.transceivers[2]!.sender.track).toBe(audio);
    expect(answerer.transceivers[0]!.sender.track).toBeNull();
    expect(answerer.transceivers[1]!.sender.params.encodings[0]).toMatchObject({
      maxBitrate: 6_000_000,
      maxFramerate: 60,
    });
    expect(answerer.transceivers[1]!.sender.params.degradationPreference).toBe(
      "maintain-resolution",
    );
    const got = screens.get("b")!.get("a");
    expect(got?.video).toEqual({ kind: "video" });
    expect(got?.audio).toEqual({ kind: "audio" });
    // The microphone stream is untouched by the share.
    expect(streams.get("b")!.get("a")).toEqual({ id: "remote" });

    await meshes.get("a")!.setScreen(null);
    expect(screens.get("b")!.get("a")).toBeNull();
    expect(answerer.transceivers[1]!.sender.track).toBeNull();
    const offers = sent.flatMap((s) => s.batch).filter((x) => x.kind === "offer");
    expect(offers).toHaveLength(1);
  });

  it("leaves out system audio that was not picked", async () => {
    const { meshes, status, screens } = network(["a", "b"]);
    const members = [member("a", T0), member("b", T1)];
    meshes.get("a")!.sync(members);
    meshes.get("b")!.sync(members);
    await vi.waitFor(() => expect(status.get("a")!.get("b")).toBe("connected"));
    const video = { id: "screen" } as unknown as MediaStreamTrack;
    await meshes.get("b")!.setScreen({ video, audio: null, quality: "720p30" });
    expect(screens.get("a")!.get("b")).toEqual({ video: { kind: "video" }, audio: null });
  });

  it("shows a running share to a member who joins later and to each viewer", async () => {
    const { meshes, status, screens } = network(["a", "b", "c"]);
    const two = [member("a", T0), member("b", T1)];
    meshes.get("a")!.sync(two);
    meshes.get("b")!.sync(two);
    await vi.waitFor(() => expect(status.get("a")!.get("b")).toBe("connected"));
    const video = { id: "screen" } as unknown as MediaStreamTrack;
    await meshes.get("a")!.setScreen({ video, audio: null, quality: "1080p30" });
    const three = [...two, member("c", T2)];
    for (const id of ["a", "b", "c"]) meshes.get(id)!.sync(three);
    await vi.waitFor(() => expect(screens.get("c")!.get("a")?.video).toBeTruthy());
    expect(screens.get("b")!.get("a")?.video).toBeTruthy();
    // Two viewers share the upload budget.
    const fromA = FakePC.all.filter((pc) => pc.transceivers[1]?.sender.track === video);
    expect(fromA).toHaveLength(2);
    for (const pc of fromA)
      expect(pc.transceivers[1]!.sender.params.encodings[0]).toMatchObject({
        maxBitrate: 4_000_000,
      });
    // The sharer leaving the call clears the share for the others.
    meshes.get("b")!.sync([member("b", T1), member("c", T2)]);
    expect(screens.get("b")!.get("a")).toBeNull();
  });

  it("keeps working with a launcher that only offers a microphone", async () => {
    const { meshes, screens } = network(["a"]);
    const a = meshes.get("a")!;
    const mic = { id: "mic" } as unknown as MediaStreamTrack;
    await a.setTrack(mic);
    a.sync([member("a", T0), member("old", T1)]);
    await a.handle({
      id: 1,
      callId: 1,
      sender: "old",
      kind: "offer",
      payload: JSON.stringify({ s: "s1", type: "offer", sdp: "offer-x|audio|" }),
    });
    const pc = FakePC.all[0]!;
    expect(pc.transceivers).toHaveLength(1);
    expect(pc.transceivers[0]!.sender.track).toBe(mic);
    const video = { id: "screen" } as unknown as MediaStreamTrack;
    await a.setScreen({ video, audio: null, quality: "1080p30" });
    // Nowhere to send it: the microphone stays on its transceiver.
    expect(pc.transceivers[0]!.sender.track).toBe(mic);
    expect(screens.get("a")!.size).toBe(0);
  });
});

describe("screenEncoding", () => {
  it("splits the upload budget and lowers size and rate when it gets thin", () => {
    expect(screenEncoding("1080p60", 1)).toEqual({
      maxBitrate: 6_000_000,
      maxFramerate: 60,
      scaleResolutionDownBy: 1,
    });
    expect(screenEncoding("1080p60", 4)).toEqual({
      maxBitrate: 2_000_000,
      maxFramerate: 30,
      scaleResolutionDownBy: 1,
    });
    expect(screenEncoding("1080p30", 15)).toEqual({
      maxBitrate: 600_000,
      maxFramerate: 30,
      scaleResolutionDownBy: 1.5,
    });
    expect(screenEncoding("720p30", 15).scaleResolutionDownBy).toBe(1);
  });
});
