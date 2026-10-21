import {
  AlertTriangle,
  ChevronDown,
  ChevronUp,
  Headphones,
  HeadphoneOff,
  Loader2,
  Mic,
  MicOff,
  Phone,
  PhoneOff,
  Users,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { Avatar } from "../../components/Avatar";
import { IconButton } from "../../components/ui";
import type { Call } from "../../lib/ipc/bindings/Call";
import type { CallMember } from "../../lib/ipc/bindings/CallMember";
import { playTone } from "../../lib/voice/audio";
import { callTitle, useCalls } from "../../stores/calls";
import { useFriends } from "../../stores/friends";

const memberName = (m: CallMember) => m.nickname ?? m.displayName;

function useAvatars() {
  const friends = useFriends((s) => s.friends);
  return useMemo(() => new Map(friends.map((f) => [f.id, f.avatar])), [friends]);
}

function Face({
  id,
  speaking,
  size = "h-9 w-9",
}: {
  id: string;
  speaking?: boolean;
  size?: string;
}) {
  const avatars = useAvatars();
  return (
    <span className={`shrink-0 rounded ${speaking ? "speaking-ring" : ""}`}>
      <Avatar photo={avatars.get(id)} className={`${size} rounded`} unit={4} />
    </span>
  );
}

function GroupFace() {
  return (
    <span className="grid h-9 w-9 shrink-0 place-items-center rounded bg-accent/15 text-accent">
      <Users size={16} />
    </span>
  );
}

function IncomingCard({ call }: { call: Call }) {
  const { t } = useTranslation();
  const accept = useCalls((s) => s.accept);
  const decline = useCalls((s) => s.decline);
  const me = useFriends((s) => s.profile?.id);
  const caller = call.members.find((m) => m.id === call.createdBy && m.id !== me);
  const title = callTitle(call);
  return (
    <div
      role="alertdialog"
      aria-label={t("calls.incoming")}
      className="call-ringing pop-in flex w-80 items-center gap-3 rounded-lg border border-line bg-surface-1/95 p-3 backdrop-blur"
    >
      {call.groupId === null && caller ? <Face id={caller.id} /> : <GroupFace />}
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-semibold">{title}</p>
        <p className="truncate text-xs text-muted">
          {call.groupId === null
            ? t("calls.incoming")
            : t("calls.groupIncoming", { name: caller ? memberName(caller) : "" })}
        </p>
      </div>
      <IconButton
        label={t("calls.decline")}
        onClick={() => void decline(call.id)}
        className="h-9 w-9 bg-danger/15 text-danger hover:bg-danger/25 hover:text-danger"
      >
        <PhoneOff size={16} />
      </IconButton>
      <IconButton
        label={t("calls.accept")}
        onClick={() => void accept(call.id)}
        className="h-9 w-9 bg-online/20 text-online hover:bg-online/30 hover:text-online"
      >
        <Phone size={16} />
      </IconButton>
    </div>
  );
}

function IncomingCalls() {
  const calls = useCalls((s) => s.calls);
  const activeId = useCalls((s) => s.activeId);
  const ringing = useMemo(
    () => calls.filter((c) => c.myState === "ringing" && c.id !== activeId),
    [calls, activeId],
  );
  const ring = ringing.length > 0 && activeId === null;
  useEffect(() => (ring ? playTone("incoming") : undefined), [ring]);
  if (ringing.length === 0) return null;
  return (
    <div className="pointer-events-auto fixed top-12 right-4 z-50 flex flex-col gap-2">
      {ringing.map((c) => (
        <IncomingCard key={c.id} call={c} />
      ))}
    </div>
  );
}

function clock(ms: number) {
  const s = Math.max(0, Math.floor(ms / 1000));
  const mm = String(Math.floor(s / 60)).padStart(2, "0");
  const ss = String(s % 60).padStart(2, "0");
  return s >= 3600 ? `${Math.floor(s / 3600)}:${mm.slice(-2)}:${ss}` : `${mm}:${ss}`;
}

function MemberRow({ m, me }: { m: CallMember; me: string | undefined }) {
  const { t } = useTranslation();
  const speaking = useCalls((s) => s.speaking[m.id] ?? false);
  const status = useCalls((s) => s.peers[m.id]);
  const volume = useCalls((s) => s.volumes[m.id] ?? 1);
  const setVolume = useCalls((s) => s.setVolume);
  const mine = m.id === me;
  const name = mine ? t("calls.you") : memberName(m);
  return (
    <li className={`flex items-center gap-2 ${m.state === "ringing" ? "opacity-50" : ""}`}>
      <Face id={m.id} speaking={speaking && m.state === "joined"} size="h-7 w-7" />
      <div className="min-w-0 flex-1">
        <p className="truncate text-xs font-semibold">{name}</p>
        {!mine && m.state === "joined" && (
          <input
            type="range"
            min={0}
            max={100}
            value={Math.round(volume * 100)}
            onChange={(e) => setVolume(m.id, Number(e.target.value) / 100)}
            aria-label={t("calls.volume", { name })}
            className="h-1 w-full accent-[var(--mc-accent)]"
          />
        )}
      </div>
      {m.state === "ringing" ? (
        <span className="text-[11px] text-muted">{t("calls.ringing")}</span>
      ) : status === "connecting" ? (
        <Loader2
          size={13}
          className="animate-spin text-muted"
          aria-label={t("calls.peerConnecting")}
        />
      ) : status === "failed" ? (
        <AlertTriangle size={13} className="text-warn" aria-label={t("calls.peerFailed")} />
      ) : null}
    </li>
  );
}

function CallBar() {
  const { t } = useTranslation();
  const activeId = useCalls((s) => s.activeId);
  const joining = useCalls((s) => s.joining);
  const call = useCalls((s) => s.calls.find((c) => c.id === s.activeId));
  const since = useCalls((s) => s.since);
  const muted = useCalls((s) => s.muted);
  const deafened = useCalls((s) => s.deafened);
  const micError = useCalls((s) => s.micError);
  const speakingMe = useCalls((s) => {
    const id = useFriends.getState().profile?.id;
    return id ? (s.speaking[id] ?? false) : false;
  });
  const { toggleMute, toggleDeafen, leave } = useCalls.getState();
  const me = useFriends((s) => s.profile?.id);
  const [open, setOpen] = useState(true);
  const [, tick] = useState(0);

  useEffect(() => {
    if (activeId === null) return;
    const id = window.setInterval(() => tick((n) => n + 1), 1000);
    return () => window.clearInterval(id);
  }, [activeId]);

  const others = call?.members.filter((m) => m.id !== me) ?? [];
  const waiting =
    call !== undefined &&
    call.groupId === null &&
    others.length > 0 &&
    others.every((m) => m.state === "ringing");
  useEffect(() => (waiting ? playTone("outgoing") : undefined), [waiting]);

  if (activeId === null && !joining) return null;
  const status = joining
    ? t("calls.connecting")
    : waiting
      ? t("calls.calling")
      : since
        ? clock(Date.now() - since)
        : "";

  return (
    <section
      aria-label={t("calls.title")}
      className="pointer-events-auto fixed right-4 bottom-4 z-50 w-72 rounded-lg border border-line bg-surface-1/95 p-3 neon-ring backdrop-blur"
    >
      <div className="flex items-center gap-2">
        <span
          className={`grid h-8 w-8 place-items-center rounded bg-online/15 text-online ${speakingMe ? "speaking-ring" : ""}`}
        >
          {call?.groupId !== null && call ? <Users size={15} /> : <Phone size={15} />}
        </span>
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-semibold">{callTitle(call) || t("calls.title")}</p>
          <p className="font-mono text-[11px] text-muted" aria-live="polite">
            {status}
          </p>
        </div>
        <IconButton
          label={open ? t("calls.collapse") : t("calls.expand")}
          onClick={() => setOpen(!open)}
          className="h-7 w-7"
        >
          {open ? <ChevronDown size={14} /> : <ChevronUp size={14} />}
        </IconButton>
      </div>

      {open && call && (
        <ul className="mt-3 flex max-h-64 flex-col gap-2 overflow-y-auto pr-1">
          {call.members.map((m) => (
            <MemberRow key={m.id} m={m} me={me} />
          ))}
        </ul>
      )}
      {micError && (
        <p className="mt-2 flex items-start gap-1.5 text-[11px] text-warn">
          <AlertTriangle size={12} className="mt-px shrink-0" />
          {t("calls.listenOnly", { reason: t(`errors.${micError}`) })}
        </p>
      )}

      <div className="mt-3 flex items-center justify-center gap-2">
        <IconButton
          label={muted ? t("calls.unmute") : t("calls.mute")}
          aria-pressed={muted}
          onClick={toggleMute}
          disabled={micError !== null}
          className={`h-10 w-10 ${muted ? "bg-danger/15 text-danger hover:text-danger" : "bg-surface-2"}`}
        >
          {muted ? <MicOff size={17} /> : <Mic size={17} />}
        </IconButton>
        <IconButton
          label={deafened ? t("calls.undeafen") : t("calls.deafen")}
          aria-pressed={deafened}
          onClick={toggleDeafen}
          className={`h-10 w-10 ${deafened ? "bg-danger/15 text-danger hover:text-danger" : "bg-surface-2"}`}
        >
          {deafened ? <HeadphoneOff size={17} /> : <Headphones size={17} />}
        </IconButton>
        <IconButton
          label={t("calls.leave")}
          onClick={() => void leave()}
          className="h-10 w-14 bg-danger text-white hover:bg-danger/85 hover:text-white"
        >
          <PhoneOff size={17} />
        </IconButton>
      </div>
    </section>
  );
}

function CallError() {
  const { t } = useTranslation();
  const error = useCalls((s) => s.error);
  const dismiss = useCalls((s) => s.dismissError);
  if (!error) return null;
  return (
    <div
      role="alert"
      className="pointer-events-auto fixed bottom-4 left-1/2 z-50 flex max-w-md -translate-x-1/2 items-center gap-2 rounded-lg border border-warn/40 bg-surface-1/95 px-3 py-2 text-xs text-warn backdrop-blur"
    >
      <AlertTriangle size={14} className="shrink-0" />
      <span className="flex-1">
        {t(`errors.${error.code}`, { ...error.params, defaultValue: error.detail })}
      </span>
      <IconButton label={t("common.close")} onClick={dismiss} className="h-6 w-6">
        <X size={12} />
      </IconButton>
    </div>
  );
}

/** Incoming calls, the active call and call errors, above every page (K86). */
export function CallLayer() {
  const connect = useCalls((s) => s.connect);
  useEffect(() => {
    let stop: (() => void) | null = null;
    let gone = false;
    void connect().then((s) => {
      if (gone) s();
      else stop = s;
    });
    return () => {
      gone = true;
      stop?.();
    };
  }, [connect]);
  return (
    <>
      <IncomingCalls />
      <CallBar />
      <CallError />
    </>
  );
}
