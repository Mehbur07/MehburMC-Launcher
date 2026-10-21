/** Speaking detection and call tones (K86), all with Web Audio; no files. */

let shared: AudioContext | null = null;

function context(): AudioContext | null {
  if (typeof AudioContext === "undefined") return null;
  shared ??= new AudioContext();
  if (shared.state === "suspended") void shared.resume().catch(() => undefined);
  return shared;
}

/** Above this RMS level (0..1) a stream counts as speaking. */
const SPEAKING_RMS = 0.035;
/** Speaking stays on this long after the level drops (no flicker). */
const HOLD_MS = 350;
const TICK_MS = 100;

/** Calls `onChange` when the stream starts or stops carrying speech. */
export function watchSpeaking(stream: MediaStream, onChange: (speaking: boolean) => void) {
  const ctx = context();
  if (!ctx || stream.getAudioTracks().length === 0) return () => undefined;
  const source = ctx.createMediaStreamSource(stream);
  const analyser = ctx.createAnalyser();
  analyser.fftSize = 512;
  source.connect(analyser);
  const data = new Uint8Array(analyser.fftSize);
  let speaking = false;
  let lastLoud = 0;
  const timer = setInterval(() => {
    analyser.getByteTimeDomainData(data);
    let sum = 0;
    for (const v of data) {
      const x = (v - 128) / 128;
      sum += x * x;
    }
    const now = Date.now();
    if (Math.sqrt(sum / data.length) > SPEAKING_RMS) lastLoud = now;
    const next = now - lastLoud < HOLD_MS;
    if (next !== speaking) {
      speaking = next;
      onChange(next);
    }
  }, TICK_MS);
  return () => {
    clearInterval(timer);
    source.disconnect();
    if (speaking) onChange(false);
  };
}

/** Live input level 0..1 for the microphone test in Settings. */
export function watchLevel(stream: MediaStream, onLevel: (level: number) => void) {
  const ctx = context();
  if (!ctx) return () => undefined;
  const source = ctx.createMediaStreamSource(stream);
  const analyser = ctx.createAnalyser();
  analyser.fftSize = 512;
  source.connect(analyser);
  const data = new Uint8Array(analyser.fftSize);
  const timer = setInterval(() => {
    analyser.getByteTimeDomainData(data);
    let peak = 0;
    for (const v of data) peak = Math.max(peak, Math.abs(v - 128) / 128);
    onLevel(Math.min(1, peak * 1.6));
  }, TICK_MS);
  return () => {
    clearInterval(timer);
    source.disconnect();
  };
}

type Tone = { freq: number[]; on: number; off: number; gain: number };

/** Incoming: a soft two-note chime; outgoing: the 450 Hz Turkish ring-back tone. */
const TONES: Record<"incoming" | "outgoing", Tone> = {
  incoming: { freq: [659.25, 880], on: 0.9, off: 1.4, gain: 0.12 },
  outgoing: { freq: [450], on: 1.0, off: 4.0, gain: 0.05 },
};

/** Plays a call tone until the returned function is called. */
export function playTone(kind: keyof typeof TONES) {
  const ctx = context();
  if (!ctx) return () => undefined;
  const tone = TONES[kind];
  const out = ctx.createGain();
  out.gain.value = tone.gain;
  out.connect(ctx.destination);
  let stopped = false;
  const ring = () => {
    if (stopped) return;
    const t = ctx.currentTime;
    const step = tone.on / tone.freq.length;
    tone.freq.forEach((f, i) => {
      const osc = ctx.createOscillator();
      const env = ctx.createGain();
      osc.type = "sine";
      osc.frequency.value = f;
      env.gain.setValueAtTime(0, t + i * step);
      env.gain.linearRampToValueAtTime(1, t + i * step + 0.02);
      env.gain.setValueAtTime(1, t + (i + 1) * step - 0.05);
      env.gain.linearRampToValueAtTime(0, t + (i + 1) * step);
      osc.connect(env).connect(out);
      osc.start(t + i * step);
      osc.stop(t + (i + 1) * step);
    });
  };
  ring();
  const timer = setInterval(ring, (tone.on + tone.off) * 1000);
  return () => {
    stopped = true;
    clearInterval(timer);
    out.disconnect();
  };
}
