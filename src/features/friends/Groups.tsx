import { Crown, LogOut, Pencil, Plus, UserMinus, UserPlus, Users, X } from "lucide-react";
import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  Badge,
  Button,
  ConfirmDialog,
  Field,
  IconButton,
  Modal,
  TextInput,
} from "../../components/ui";
import type { ChatGroup } from "../../lib/ipc/bindings/ChatGroup";
import type { ErrorPayload } from "../../lib/ipc/bindings/ErrorPayload";
import type { Friend } from "../../lib/ipc/bindings/Friend";
import { friendName, memberName, useFriends } from "../../stores/friends";
import { ChatView, ErrorText } from "./ChatView";

/** Members besides the creator (the database allows 16 in all). */
const MAX_OTHERS = 15;
const MAX_GROUP_NAME = 48;
const MAX_NICKNAME = 32;

function GroupIcon({ className = "h-7 w-7" }: { className?: string }) {
  return (
    <span
      className={`grid shrink-0 place-items-center rounded bg-accent/15 text-accent ${className}`}
    >
      <Users size={14} />
    </span>
  );
}

/** Friends to tick, at most `max`. */
function FriendPicker({
  friends,
  picked,
  setPicked,
  max,
}: {
  friends: Friend[];
  picked: string[];
  setPicked: (ids: string[]) => void;
  max: number;
}) {
  const { t } = useTranslation();
  if (friends.length === 0)
    return <p className="text-xs text-muted">{t("friends.groups.noFriends")}</p>;
  return (
    <ul className="flex max-h-60 flex-col gap-1 overflow-y-auto rounded-md border border-line p-1">
      {friends.map((f) => {
        const on = picked.includes(f.id);
        return (
          <li key={f.id}>
            <label className="flex items-center gap-2 rounded px-2 py-1.5 text-sm hover:bg-surface-2">
              <input
                type="checkbox"
                checked={on}
                disabled={!on && picked.length >= max}
                onChange={() =>
                  setPicked(on ? picked.filter((p) => p !== f.id) : [...picked, f.id])
                }
                className="accent-[rgb(var(--mc-accent-rgb))]"
              />
              <span className="min-w-0 flex-1 truncate">{friendName(f)}</span>
            </label>
          </li>
        );
      })}
    </ul>
  );
}

export function CreateGroupDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { t } = useTranslation();
  const all = useFriends((s) => s.friends);
  const friends = useMemo(() => all.filter((f) => f.status === "accepted"), [all]);
  const createGroup = useFriends((s) => s.createGroup);
  const [name, setName] = useState("");
  const [picked, setPicked] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ErrorPayload | null>(null);

  const close = () => {
    setName("");
    setPicked([]);
    setError(null);
    onClose();
  };
  const submit = async () => {
    setBusy(true);
    const r = await createGroup(name, picked);
    setBusy(false);
    if (typeof r === "number") close();
    else setError(r);
  };

  return (
    <Modal
      open={open}
      onClose={close}
      title={t("friends.groups.create")}
      footer={
        <>
          <Button variant="ghost" onClick={close}>
            {t("common.cancel")}
          </Button>
          <Button
            variant="primary"
            disabled={busy || !name.trim() || picked.length === 0}
            onClick={() => void submit()}
          >
            <Plus size={14} />
            {t("friends.groups.createSubmit")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-3">
        <Field label={t("friends.groups.name")}>
          <TextInput
            value={name}
            maxLength={MAX_GROUP_NAME}
            onChange={(e) => setName(e.target.value)}
          />
        </Field>
        <span className="text-xs text-muted">
          {t("friends.groups.pickMembers", { max: MAX_OTHERS })}
        </span>
        <FriendPicker friends={friends} picked={picked} setPicked={setPicked} max={MAX_OTHERS} />
        <ErrorText error={error} />
      </div>
    </Modal>
  );
}

/** Group entries for the left column. */
export function GroupList({ active }: { active: number | null }) {
  const { t } = useTranslation();
  const groups = useFriends((s) => s.groups);
  const openGroup = useFriends((s) => s.openGroup);
  const [creating, setCreating] = useState(false);
  return (
    <div className="mt-2 flex flex-col gap-1 border-t border-line pt-2">
      <div className="flex items-center px-2">
        <h2 className="flex-1 py-1 text-xs font-semibold text-muted">
          {t("friends.groups.title", { count: groups.length })}
        </h2>
        <IconButton
          label={t("friends.groups.create")}
          onClick={() => setCreating(true)}
          className="h-7 w-7"
        >
          <Plus size={14} />
        </IconButton>
      </div>
      {groups.length === 0 ? (
        <p className="px-2 py-2 text-xs text-muted">{t("friends.groups.none")}</p>
      ) : (
        groups.map((g) => {
          const on = g.id === active;
          return (
            <button
              key={g.id}
              type="button"
              onClick={() => void openGroup(g.id)}
              aria-current={on ? "true" : undefined}
              className={`flex items-center gap-2 rounded-md px-3 py-2 text-left text-sm transition-colors ${
                on ? "bg-accent/10 text-accent neon-ring" : "hover:bg-surface-2"
              }`}
            >
              <GroupIcon />
              <span className="min-w-0 flex-1 truncate font-semibold">{g.name}</span>
              {g.unread > 0 && (
                <span className="rounded-full bg-accent px-1.5 text-[11px] font-bold text-on-accent">
                  {g.unread}
                </span>
              )}
            </button>
          );
        })
      )}
      <CreateGroupDialog open={creating} onClose={() => setCreating(false)} />
    </div>
  );
}

function MembersDialog({
  group,
  open,
  onClose,
}: {
  group: ChatGroup;
  open: boolean;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const me = useFriends((s) => s.profile?.id);
  const all = useFriends((s) => s.friends);
  const friends = useMemo(() => all.filter((f) => f.status === "accepted"), [all]);
  const addMembers = useFriends((s) => s.addGroupMembers);
  const removeMember = useFriends((s) => s.removeGroupMember);
  const [picked, setPicked] = useState<string[]>([]);
  const [error, setError] = useState<ErrorPayload | null>(null);
  const inGroup = new Set(group.members.map((m) => m.id));
  const candidates = friends.filter((f) => !inGroup.has(f.id));
  const room = Math.max(0, MAX_OTHERS + 1 - group.members.length);

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={t("friends.groups.membersTitle", { name: group.name })}
      footer={
        <Button variant="ghost" onClick={onClose}>
          {t("common.close")}
        </Button>
      }
    >
      <div className="flex flex-col gap-3">
        <ul className="flex flex-col gap-1">
          {group.members.map((m) => (
            <li
              key={m.id}
              className="flex items-center gap-2 rounded-md bg-surface-2/60 px-3 py-1.5 text-sm"
            >
              {m.online && (
                <span
                  role="img"
                  aria-label={t("friends.online")}
                  className="online-dot inline-block h-2 w-2 rounded-full"
                />
              )}
              <span className="min-w-0 flex-1 truncate">
                {m.id === me ? t("friends.groups.you") : memberName(m)}
              </span>
              {m.id === group.owner && (
                <Badge tone="accent">
                  <Crown size={11} />
                  {t("friends.groups.owner")}
                </Badge>
              )}
              {group.isOwner && m.id !== me && (
                <IconButton
                  label={t("friends.groups.remove", { name: memberName(m) })}
                  className="h-7 w-7 hover:text-danger"
                  onClick={() => void removeMember(group.id, m.id).then(setError)}
                >
                  <UserMinus size={14} />
                </IconButton>
              )}
            </li>
          ))}
        </ul>
        {group.isOwner && (
          <section className="flex flex-col gap-2">
            <h3 className="text-xs font-semibold text-muted">{t("friends.groups.addTitle")}</h3>
            {candidates.length === 0 ? (
              <p className="text-xs text-muted">{t("friends.groups.allIn")}</p>
            ) : (
              <>
                <FriendPicker
                  friends={candidates}
                  picked={picked}
                  setPicked={setPicked}
                  max={room}
                />
                <Button
                  size="sm"
                  disabled={picked.length === 0}
                  onClick={() =>
                    void addMembers(group.id, picked).then((e) => {
                      setError(e);
                      if (!e) setPicked([]);
                    })
                  }
                >
                  <UserPlus size={13} />
                  {t("friends.groups.add")}
                </Button>
              </>
            )}
          </section>
        )}
        <ErrorText error={error} />
      </div>
    </Modal>
  );
}

function RenameDialog({
  group,
  open,
  onClose,
}: {
  group: ChatGroup;
  open: boolean;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const rename = useFriends((s) => s.renameGroup);
  const [name, setName] = useState(group.name);
  const [error, setError] = useState<ErrorPayload | null>(null);
  return (
    <Modal
      open={open}
      onClose={onClose}
      title={t("friends.groups.rename")}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button
            variant="primary"
            disabled={!name.trim()}
            onClick={() =>
              void rename(group.id, name).then((e) => {
                setError(e);
                if (!e) onClose();
              })
            }
          >
            {t("common.save")}
          </Button>
        </>
      }
    >
      <Field label={t("friends.groups.name")}>
        <TextInput
          value={name}
          maxLength={MAX_GROUP_NAME}
          onChange={(e) => setName(e.target.value)}
        />
      </Field>
      <ErrorText error={error} />
    </Modal>
  );
}

/** Header, log and composer of one group chat. */
export function GroupPanel({ group }: { group: ChatGroup }) {
  const { t } = useTranslation();
  const me = useFriends((s) => s.profile?.id);
  const messages = useFriends((s) => s.groupMessages[group.id]);
  const pollGroup = useFriends((s) => s.pollGroup);
  const sendGroup = useFriends((s) => s.sendGroup);
  const leaveGroup = useFriends((s) => s.leaveGroup);
  const [dialog, setDialog] = useState<"members" | "rename" | "leave" | null>(null);
  const names = new Map(group.members.map((m) => [m.id, memberName(m)]));
  const poll = useCallback(() => pollGroup(), [pollGroup]);

  return (
    <>
      <div className="flex flex-wrap items-center gap-2 border-b border-line px-3 py-2">
        <GroupIcon className="h-8 w-8" />
        <span className="font-display text-lg font-semibold">{group.name}</span>
        <button
          type="button"
          onClick={() => setDialog("members")}
          className="text-xs text-muted underline-offset-2 hover:text-accent hover:underline"
        >
          {t("friends.groups.members", { count: group.members.length })}
        </button>
        <div className="ml-auto flex gap-1">
          {group.isOwner && (
            <IconButton label={t("friends.groups.rename")} onClick={() => setDialog("rename")}>
              <Pencil size={15} />
            </IconButton>
          )}
          <IconButton
            label={t("friends.groups.membersButton")}
            onClick={() => setDialog("members")}
          >
            <Users size={15} />
          </IconButton>
          <IconButton
            label={t("friends.groups.leave")}
            onClick={() => setDialog("leave")}
            className="hover:text-danger"
          >
            <LogOut size={15} />
          </IconButton>
        </div>
      </div>
      <ChatView
        key={group.id}
        label={t("friends.groups.chatLabel", { name: group.name })}
        lines={messages?.map((m) => ({
          id: m.id,
          mine: m.sender === me,
          author: names.get(m.sender) ?? t("friends.groups.formerMember"),
          body: m.body,
          createdAt: m.createdAt,
        }))}
        onPoll={poll}
        onSend={sendGroup}
      />
      <MembersDialog group={group} open={dialog === "members"} onClose={() => setDialog(null)} />
      {dialog === "rename" && <RenameDialog group={group} open onClose={() => setDialog(null)} />}
      <ConfirmDialog
        open={dialog === "leave"}
        danger
        title={t("friends.groups.leaveTitle", { name: group.name })}
        message={t("friends.groups.leaveBody")}
        confirmLabel={t("friends.groups.leave")}
        onClose={() => setDialog(null)}
        onConfirm={() => void leaveGroup(group.id)}
      />
    </>
  );
}

/** Our private name for a friend (K85). */
export function NicknameDialog({
  friend,
  onClose,
}: {
  friend: Friend | null;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const setNickname = useFriends((s) => s.setNickname);
  const [value, setValue] = useState(friend?.nickname ?? "");
  const [error, setError] = useState<ErrorPayload | null>(null);
  if (!friend) return null;
  const save = (nickname: string | null) =>
    void setNickname(friend.id, nickname).then((e) => {
      setError(e);
      if (!e) onClose();
    });
  return (
    <Modal
      open
      onClose={onClose}
      title={t("friends.nickname.title", { name: friend.displayName })}
      footer={
        <>
          {friend.nickname && (
            <Button variant="ghost" className="mr-auto" onClick={() => save(null)}>
              <X size={14} />
              {t("friends.nickname.remove")}
            </Button>
          )}
          <Button variant="ghost" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button variant="primary" disabled={!value.trim()} onClick={() => save(value)}>
            {t("common.save")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-2">
        <Field label={t("friends.nickname.label")}>
          <TextInput
            value={value}
            maxLength={MAX_NICKNAME}
            placeholder={friend.displayName}
            onChange={(e) => setValue(e.target.value)}
          />
        </Field>
        <p className="text-xs text-muted">
          {t("friends.nickname.hint", { name: friend.displayName })}
        </p>
        <ErrorText error={error} />
      </div>
    </Modal>
  );
}
