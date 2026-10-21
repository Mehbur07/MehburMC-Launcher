import {
  Ban,
  Check,
  Copy,
  MessageCircle,
  Package,
  Pencil,
  ShieldCheck,
  UserMinus,
  UserPlus,
  Users,
  WifiOff,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";
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
import { friendName, useFriends } from "../../stores/friends";
import { ChatView, ErrorText } from "./ChatView";
import { FriendMods } from "./FriendMods";
import { GroupList, GroupPanel, NicknameDialog } from "./Groups";
import { MyShares } from "./MyShares";

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
  const { t } = useTranslation();
  const messages = useFriends((s) => s.messages[friend.id]);
  const pollChat = useFriends((s) => s.pollChat);
  const send = useFriends((s) => s.send);
  return (
    <ChatView
      label={t("friends.chatWith", { name: friendName(friend) })}
      lines={messages?.map((m) => ({
        id: m.id,
        mine: m.sender !== friend.id,
        body: m.body,
        createdAt: m.createdAt,
      }))}
      onPoll={pollChat}
      onSend={send}
    />
  );
}

export function FriendsPage() {
  const { t } = useTranslation();
  const { enabled, friends, chatWith, offline, status, openChat, remove, block, disable } =
    useFriends();
  const groupWith = useFriends((s) => s.groupWith);
  const group = useFriends((s) => s.groups.find((g) => g.id === s.groupWith) ?? null);
  const [tab, setTab] = useState<"chat" | "mods">("chat");
  const [naming, setNaming] = useState(false);
  const [confirm, setConfirm] = useState<"remove" | "block" | "disable" | null>(null);

  useEffect(() => {
    void status();
  }, [status]);

  // Online friends first; the server already sorts by name.
  const accepted = friends
    .filter((f) => f.status === "accepted")
    .sort((a, b) => Number(b.online) - Number(a.online));
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
                      <Avatar
                        photo={f.avatar}
                        className="h-7 w-7 shrink-0 rounded"
                        online={f.online}
                        dot="sm"
                      />
                      <span className="min-w-0 flex-1 truncate font-semibold">{friendName(f)}</span>
                      {f.unread > 0 && (
                        <span className="rounded-full bg-accent px-1.5 text-[11px] font-bold text-on-accent">
                          {f.unread}
                        </span>
                      )}
                    </button>
                  );
                })
              )}
              <GroupList active={groupWith} />
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
              {group ? (
                <GroupPanel key={group.id} group={group} />
              ) : !current ? (
                <EmptyState icon={<MessageCircle size={30} />} title={t("friends.pickFriend")}>
                  <p>{t("friends.pickFriendHint")}</p>
                </EmptyState>
              ) : (
                <>
                  <div className="flex flex-wrap items-center gap-2 border-b border-line px-3 py-2">
                    <Avatar
                      photo={current.avatar}
                      className="h-8 w-8 rounded"
                      online={current.online}
                    />
                    <span className="font-display text-lg font-semibold">
                      {friendName(current)}
                    </span>
                    {current.nickname && (
                      <span className="text-xs text-muted">
                        {t("friends.nickname.realName", { name: current.displayName })}
                      </span>
                    )}
                    <span className="font-mono text-xs text-muted">{current.friendCode}</span>
                    <IconButton
                      label={t("friends.nickname.edit")}
                      onClick={() => setNaming(true)}
                      className="h-7 w-7"
                    >
                      <Pencil size={13} />
                    </IconButton>
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
          name: current ? friendName(current) : "",
        })}
        message={t(`friends.confirm.${confirm ?? "remove"}Body`, {
          name: current ? friendName(current) : "",
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
      {naming && current && (
        <NicknameDialog key={current.id} friend={current} onClose={() => setNaming(false)} />
      )}
    </div>
  );
}
