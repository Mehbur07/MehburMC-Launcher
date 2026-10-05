import {
  Ban,
  Check,
  Copy,
  MessageCircle,
  Package,
  Send,
  ShieldCheck,
  UserMinus,
  UserPlus,
  Users,
  WifiOff,
  X,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { Avatar } from "../../components/Avatar";
import {
  Badge,
  Button,
  ConfirmDialog,
  EmptyState,
  IconButton,
  TextInput,
} from "../../components/ui";
import type { ErrorPayload } from "../../lib/ipc/bindings/ErrorPayload";
import type { Friend } from "../../lib/ipc/bindings/Friend";
import { useFriends } from "../../stores/friends";
import { FriendMods } from "./FriendMods";
import { MyShares } from "./MyShares";

const CHAT_POLL_MS = 3000;
const MAX_CHARS = 2000;

function ErrorText({ error }: { error: ErrorPayload | null }) {
  const { t } = useTranslation();
  if (!error) return null;
  return (
    <p className="text-xs text-warn">
      {t(`errors.${error.code}`, { ...error.params, defaultValue: error.detail })}
    </p>
  );
}

function Consent() {
  const { t } = useTranslation();
  const enable = useFriends((s) => s.enable);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ErrorPayload | null>(null);
  return (
    <section className="mx-auto flex max-w-xl flex-col gap-4 rounded-lg border border-line bg-surface-1/85 p-6 backdrop-blur">
      <h2 className="flex items-center gap-2 font-display text-xl font-semibold">
        <ShieldCheck size={20} className="text-accent" />
        {t("friends.consentTitle")}
      </h2>
      <p className="text-sm text-muted">{t("friends.consentBody")}</p>
      <ul className="flex list-disc flex-col gap-1 pl-5 text-sm text-muted">
        <li>{t("friends.consentStored")}</li>
        <li>{t("friends.consentPrivate")}</li>
        <li>{t("friends.consentDelete")}</li>
      </ul>
      <ErrorText error={error} />
      <Button
        variant="primary"
        disabled={busy}
        onClick={async () => {
          setBusy(true);
          setError(await enable());
          setBusy(false);
        }}
      >
        <Users size={15} />
        {t("friends.enable")}
      </Button>
    </section>
  );
}

function MyCode() {
  const { t } = useTranslation();
  const profile = useFriends((s) => s.profile);
  const sendRequest = useFriends((s) => s.sendRequest);
  const [code, setCode] = useState("");
  const [result, setResult] = useState<ErrorPayload | string | null>(null);
  const [copied, setCopied] = useState(false);

  return (
    <section className="grid gap-4 rounded-lg border border-line bg-surface-1/85 p-4 backdrop-blur md:grid-cols-2">
      <div className="flex flex-col gap-1">
        <span className="text-xs text-muted">{t("friends.myCode")}</span>
        <div className="flex items-center gap-2">
          <span data-selectable className="font-mono text-xl font-bold tracking-wider text-accent">
            {profile?.friendCode ?? "…"}
          </span>
          <IconButton
            label={t("friends.copyCode")}
            onClick={() => {
              if (!profile) return;
              void navigator.clipboard?.writeText(profile.friendCode);
              setCopied(true);
              window.setTimeout(() => setCopied(false), 1500);
            }}
          >
            {copied ? <Check size={15} /> : <Copy size={15} />}
          </IconButton>
        </div>
        <span className="text-xs text-muted">
          {t("friends.shownAs", { name: profile?.displayName ?? "" })}
        </span>
      </div>
      <form
        className="flex flex-col gap-1"
        onSubmit={async (e) => {
          e.preventDefault();
          if (!code.trim()) return;
          const r = await sendRequest(code);
          setResult(r);
          if (typeof r === "string") setCode("");
        }}
      >
        <span className="text-xs text-muted">{t("friends.addByCode")}</span>
        <div className="flex gap-2">
          <TextInput
            value={code}
            maxLength={32}
            spellCheck={false}
            placeholder="MEHBUR-XXXX"
            aria-label={t("friends.addByCode")}
            onChange={(e) => {
              setCode(e.target.value.toUpperCase());
              setResult(null);
            }}
          />
          <Button
            type="submit"
            variant="primary"
            disabled={!code.trim()}
            className="shrink-0 whitespace-nowrap"
          >
            <UserPlus size={14} />
            {t("friends.send")}
          </Button>
        </div>
        {typeof result === "string" ? (
          <p className="text-xs text-success">
            {t(result === "accepted" ? "friends.nowFriends" : "friends.requestSent")}
          </p>
        ) : (
          <ErrorText error={result} />
        )}
      </form>
    </section>
  );
}

function Requests({ friends }: { friends: Friend[] }) {
  const { t } = useTranslation();
  const respond = useFriends((s) => s.respond);
  const remove = useFriends((s) => s.remove);
  const pending = friends.filter((f) => f.status === "pending");
  if (pending.length === 0) return null;
  return (
    <section className="flex flex-col gap-2 rounded-lg border border-line bg-surface-1/85 p-4 backdrop-blur">
      <h2 className="text-sm font-semibold">{t("friends.requests")}</h2>
      {pending.map((f) => (
        <div key={f.id} className="flex items-center gap-3 text-sm">
          <span className="min-w-0 flex-1 truncate">
            <span className="font-semibold">{f.displayName}</span>{" "}
            <span className="font-mono text-xs text-muted">{f.friendCode}</span>
          </span>
          {f.incoming ? (
            <>
              <Button size="sm" variant="primary" onClick={() => void respond(f.requestId, true)}>
                {t("friends.accept")}
              </Button>
              <Button size="sm" variant="ghost" onClick={() => void respond(f.requestId, false)}>
                {t("friends.decline")}
              </Button>
            </>
          ) : (
            <>
              <Badge>{t("friends.waiting")}</Badge>
              <IconButton label={t("friends.cancelRequest")} onClick={() => void remove(f.id)}>
                <X size={14} />
              </IconButton>
            </>
          )}
        </div>
      ))}
    </section>
  );
}

function Chat({ friend }: { friend: Friend }) {
  const { t, i18n } = useTranslation();
  const messages = useFriends((s) => s.messages[friend.id]);
  const pollChat = useFriends((s) => s.pollChat);
  const send = useFriends((s) => s.send);
  const [text, setText] = useState("");
  const [error, setError] = useState<ErrorPayload | null>(null);
  const [sending, setSending] = useState(false);
  const bottom = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const id = window.setInterval(() => void pollChat(), CHAT_POLL_MS);
    return () => window.clearInterval(id);
  }, [pollChat]);

  useEffect(() => {
    bottom.current?.scrollIntoView?.({ block: "end" });
  }, [messages?.length]);

  const submit = async () => {
    const body = text.trim();
    if (!body || sending) return;
    setSending(true);
    const err = await send(body);
    setSending(false);
    setError(err);
    if (!err) setText("");
  };

  const time = (iso: string) =>
    new Date(iso).toLocaleTimeString(i18n.language, { hour: "2-digit", minute: "2-digit" });

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div
        className="flex min-h-[240px] flex-1 flex-col gap-1.5 overflow-y-auto p-3"
        role="log"
        aria-label={t("friends.chatWith", { name: friend.displayName })}
      >
        {messages === undefined ? (
          <p className="m-auto text-xs text-muted">{t("common.loading")}</p>
        ) : messages.length === 0 ? (
          <p className="m-auto text-xs text-muted">{t("friends.noMessages")}</p>
        ) : (
          messages.map((m) => {
            const mine = m.sender !== friend.id;
            return (
              <div
                key={m.id}
                className={`flex max-w-[80%] flex-col rounded-lg px-3 py-1.5 text-sm ${
                  mine
                    ? "self-end bg-accent/20 text-fg"
                    : "self-start border border-line bg-surface-2/80"
                }`}
              >
                <span data-selectable className="whitespace-pre-wrap break-words">
                  {m.body}
                </span>
                <span className="self-end text-[10px] text-muted">{time(m.createdAt)}</span>
              </div>
            );
          })
        )}
        <div ref={bottom} />
      </div>
      <form
        className="flex flex-col gap-1 border-t border-line p-3"
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <div className="flex items-end gap-2">
          <textarea
            value={text}
            rows={2}
            maxLength={MAX_CHARS}
            placeholder={t("friends.messagePlaceholder")}
            aria-label={t("friends.messagePlaceholder")}
            onChange={(e) => {
              setText(e.target.value);
              setError(null);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                void submit();
              }
            }}
            className="min-h-[2.5rem] flex-1 resize-none rounded-md border border-line bg-surface-2/80 px-3 py-2 text-sm outline-none focus:border-accent/60"
          />
          <Button type="submit" variant="primary" disabled={!text.trim() || sending}>
            <Send size={14} />
            {t("friends.sendMessage")}
          </Button>
        </div>
        <div className="flex justify-between text-[11px] text-muted">
          <ErrorText error={error} />
          <span className="ml-auto">
            {text.length}/{MAX_CHARS}
          </span>
        </div>
      </form>
    </div>
  );
}

export function FriendsPage() {
  const { t } = useTranslation();
  const { enabled, friends, chatWith, offline, status, openChat, remove, block, disable } =
    useFriends();
  const [tab, setTab] = useState<"chat" | "mods">("chat");
  const [confirm, setConfirm] = useState<"remove" | "block" | "disable" | null>(null);

  useEffect(() => {
    void status();
  }, [status]);

  const accepted = friends.filter((f) => f.status === "accepted");
  const blocked = friends.filter((f) => f.status === "blocked");
  const current = accepted.find((f) => f.id === chatWith) ?? null;

  if (enabled === null) {
    return <p className="mt-24 text-center text-sm text-muted">{t("common.loading")}</p>;
  }

  return (
    <div className="mx-auto flex max-w-6xl flex-col gap-5 pb-6">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <h1 className="font-display text-3xl font-bold tracking-wide">{t("nav.friends")}</h1>
        {offline && (
          <Badge tone="warn">
            <WifiOff size={12} />
            {t("friends.offline")}
          </Badge>
        )}
      </div>

      {!enabled ? (
        <Consent />
      ) : (
        <>
          <MyCode />
          <Requests friends={friends} />

          <div className="grid min-h-[480px] gap-5 lg:grid-cols-[260px_1fr]">
            <section className="flex flex-col gap-1 rounded-lg border border-line bg-surface-1/85 p-2 backdrop-blur">
              <h2 className="px-2 py-1 text-xs font-semibold text-muted">
                {t("friends.list", { count: accepted.length })}
              </h2>
              {accepted.length === 0 ? (
                <p className="px-2 py-4 text-xs text-muted">{t("friends.noFriends")}</p>
              ) : (
                accepted.map((f) => {
                  const active = f.id === current?.id;
                  return (
                    <button
                      key={f.id}
                      type="button"
                      onClick={() => void openChat(f.id)}
                      aria-current={active ? "true" : undefined}
                      className={`flex items-center gap-2 rounded-md px-3 py-2 text-left text-sm transition-colors ${
                        active ? "bg-accent/10 text-accent neon-ring" : "hover:bg-surface-2"
                      }`}
                    >
                      <Avatar photo={f.avatar} className="h-7 w-7 shrink-0 rounded" />
                      <span className="min-w-0 flex-1 truncate font-semibold">{f.displayName}</span>
                      {f.unread > 0 && (
                        <span className="rounded-full bg-accent px-1.5 text-[11px] font-bold text-on-accent">
                          {f.unread}
                        </span>
                      )}
                    </button>
                  );
                })
              )}
              {blocked.length > 0 && (
                <div className="mt-auto border-t border-line pt-2">
                  <h3 className="px-2 text-[11px] text-muted">{t("friends.blocked")}</h3>
                  {blocked.map((f) => (
                    <div
                      key={f.id}
                      className="flex items-center gap-2 px-2 py-1 text-xs text-muted"
                    >
                      <span className="min-w-0 flex-1 truncate">{f.displayName}</span>
                      <button
                        type="button"
                        className="hover:text-accent"
                        onClick={() => void remove(f.id)}
                      >
                        {t("friends.unblock")}
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </section>

            <section className="flex min-w-0 flex-col rounded-lg border border-line bg-surface-1/85 backdrop-blur">
              {!current ? (
                <EmptyState icon={<MessageCircle size={30} />} title={t("friends.pickFriend")}>
                  <p>{t("friends.pickFriendHint")}</p>
                </EmptyState>
              ) : (
                <>
                  <div className="flex flex-wrap items-center gap-2 border-b border-line px-3 py-2">
                    <Avatar photo={current.avatar} className="h-8 w-8 rounded" />
                    <span className="font-display text-lg font-semibold">
                      {current.displayName}
                    </span>
                    <span className="font-mono text-xs text-muted">{current.friendCode}</span>
                    <div role="tablist" className="ml-auto flex gap-1">
                      <Button
                        size="sm"
                        role="tab"
                        aria-selected={tab === "chat"}
                        variant={tab === "chat" ? "secondary" : "ghost"}
                        onClick={() => setTab("chat")}
                      >
                        <MessageCircle size={13} />
                        {t("friends.tabChat")}
                      </Button>
                      <Button
                        size="sm"
                        role="tab"
                        aria-selected={tab === "mods"}
                        variant={tab === "mods" ? "secondary" : "ghost"}
                        onClick={() => setTab("mods")}
                      >
                        <Package size={13} />
                        {t("friends.tabMods")}
                      </Button>
                    </div>
                    <IconButton
                      label={t("friends.remove")}
                      onClick={() => setConfirm("remove")}
                      className="hover:text-danger"
                    >
                      <UserMinus size={15} />
                    </IconButton>
                    <IconButton
                      label={t("friends.block")}
                      onClick={() => setConfirm("block")}
                      className="hover:text-danger"
                    >
                      <Ban size={15} />
                    </IconButton>
                  </div>
                  {tab === "chat" ? (
                    <Chat key={current.id} friend={current} />
                  ) : (
                    <FriendMods key={current.id} friend={current} />
                  )}
                </>
              )}
            </section>
          </div>

          <MyShares />

          <div className="flex justify-end">
            <Button size="sm" variant="danger" onClick={() => setConfirm("disable")}>
              {t("friends.disable")}
            </Button>
          </div>
        </>
      )}

      <ConfirmDialog
        open={confirm !== null}
        title={t(`friends.confirm.${confirm ?? "remove"}Title`, {
          name: current?.displayName ?? "",
        })}
        message={t(`friends.confirm.${confirm ?? "remove"}Body`, {
          name: current?.displayName ?? "",
        })}
        confirmLabel={t(`friends.confirm.${confirm ?? "remove"}Action`)}
        danger
        onConfirm={() => {
          if (confirm === "disable") void disable();
          else if (current && confirm === "remove") void remove(current.id);
          else if (current && confirm === "block") void block(current.id);
        }}
        onClose={() => setConfirm(null)}
      />
    </div>
  );
}
