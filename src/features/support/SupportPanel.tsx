import { MessageSquarePlus, Send, XCircle } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Badge, Button, TextInput } from "../../components/ui";
import type { ErrorPayload } from "../../lib/ipc/bindings/ErrorPayload";
import type { SupportThread } from "../../lib/ipc/bindings/SupportThread";
import { formatIso } from "../../lib/format";
import { useSupport } from "../../stores/support";
import { ChatView, ErrorText, MAX_CHARS } from "../friends/ChatView";

export const MAX_SUBJECT = 80;

/** Yeni konu formu: konu başlığı ve ilk mesaj. */
function NewThread({ onOpened }: { onOpened: (id: number) => void }) {
  const { t } = useTranslation();
  const open = useSupport((s) => s.open);
  const [subject, setSubject] = useState("");
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ErrorPayload | null>(null);
  const ready = subject.trim() !== "" && body.trim() !== "" && !busy;

  return (
    <form
      className="flex flex-col gap-3 p-4"
      onSubmit={async (e) => {
        e.preventDefault();
        if (!ready) return;
        setBusy(true);
        const r = await open(subject, body);
        setBusy(false);
        if (typeof r === "number") {
          setSubject("");
          setBody("");
          onOpened(r);
        } else setError(r);
      }}
    >
      <h2 className="font-display text-lg font-semibold">{t("support.newThread")}</h2>
      <label className="flex flex-col gap-1 text-xs text-muted">
        {t("support.subject")}
        <TextInput
          value={subject}
          maxLength={MAX_SUBJECT}
          placeholder={t("support.subjectPlaceholder")}
          onChange={(e) => {
            setSubject(e.target.value);
            setError(null);
          }}
        />
      </label>
      <label className="flex flex-col gap-1 text-xs text-muted">
        {t("support.body")}
        <textarea
          value={body}
          rows={6}
          maxLength={MAX_CHARS}
          placeholder={t("support.bodyPlaceholder")}
          onChange={(e) => {
            setBody(e.target.value);
            setError(null);
          }}
          className="resize-none rounded-md border border-line bg-surface-2/80 px-3 py-2 text-sm text-fg outline-none focus:border-accent/60"
        />
      </label>
      <div className="flex items-center justify-between gap-2 text-[11px] text-muted">
        <span>{t("support.limits")}</span>
        <span>
          {body.length}/{MAX_CHARS}
        </span>
      </div>
      <ErrorText error={error} />
      <Button type="submit" variant="primary" disabled={!ready} className="self-end">
        <Send size={14} />
        {t("support.send")}
      </Button>
    </form>
  );
}

/** Seçili konunun sohbeti. */
function Conversation({ thread }: { thread: SupportThread }) {
  const { t } = useTranslation();
  const lines = useSupport((s) => s.messages[thread.id]);
  const poll = useSupport((s) => s.poll);
  const send = useSupport((s) => s.send);
  const setOpen = useSupport((s) => s.setOpen);
  const [error, setError] = useState<ErrorPayload | null>(null);
  const onPoll = useCallback(() => poll(thread.id), [poll, thread.id]);
  const onSend = useCallback((body: string) => send(thread.id, body), [send, thread.id]);

  useEffect(() => {
    void poll(thread.id);
  }, [poll, thread.id]);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center gap-2 border-b border-line px-4 py-3">
        <h2 className="min-w-0 flex-1 truncate font-semibold">{thread.subject}</h2>
        <Badge tone={thread.open ? "accent" : "neutral"}>
          {thread.open ? t("support.open") : t("support.closed")}
        </Badge>
        {thread.open && (
          <Button
            size="sm"
            variant="ghost"
            onClick={async () => setError(await setOpen(thread.id, false))}
          >
            <XCircle size={13} />
            {t("support.close")}
          </Button>
        )}
      </div>
      <ErrorText error={error} />
      <ChatView
        label={thread.subject}
        lines={lines?.map((m) => ({
          id: m.id,
          mine: m.mine,
          author: m.fromAdmin ? t("support.team") : undefined,
          body: m.body,
          createdAt: m.createdAt,
        }))}
        onPoll={onPoll}
        onSend={onSend}
        closedNote={thread.open ? undefined : t("support.closedNote")}
      />
    </div>
  );
}

/** Oyuncunun konuları, seçili sohbet ve yeni konu formu (sayfa ve ban ekranı). */
export function SupportPanel() {
  const { t, i18n } = useTranslation();
  const threads = useSupport((s) => s.threads);
  const error = useSupport((s) => s.error);
  const loadThreads = useSupport((s) => s.loadThreads);
  // null: yeni konu formu.
  const [selected, setSelected] = useState<number | null>(null);

  useEffect(() => {
    void loadThreads();
  }, [loadThreads]);

  const current = threads?.find((x) => x.id === selected) ?? null;

  return (
    <div className="grid min-h-[420px] flex-1 grid-cols-[260px_1fr] overflow-hidden rounded-lg border border-line bg-surface-1/85 backdrop-blur">
      <aside className="flex min-h-0 flex-col border-r border-line">
        <div className="p-3">
          <Button
            size="sm"
            variant={selected === null ? "primary" : "ghost"}
            className="w-full"
            onClick={() => setSelected(null)}
          >
            <MessageSquarePlus size={14} />
            {t("support.newThread")}
          </Button>
        </div>
        <p className="px-3 pb-1 text-xs font-semibold text-muted">{t("support.threads")}</p>
        <ul className="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto px-2 pb-2">
          {threads === null ? (
            <li className="p-2 text-xs text-muted">{t("common.loading")}</li>
          ) : threads.length === 0 ? (
            <li className="p-2 text-xs text-muted">{t("support.empty")}</li>
          ) : (
            threads.map((x) => (
              <li key={x.id}>
                <button
                  type="button"
                  onClick={() => setSelected(x.id)}
                  aria-current={x.id === selected ? "true" : undefined}
                  className={`flex w-full flex-col gap-0.5 rounded-md px-2 py-1.5 text-left transition-colors ${
                    x.id === selected ? "bg-accent/10 text-accent" : "hover:bg-surface-2"
                  }`}
                >
                  <span className="flex items-center gap-2">
                    <span className="min-w-0 flex-1 truncate text-sm font-medium">{x.subject}</span>
                    {x.unread > 0 && (
                      <span className="rounded-full bg-accent px-1.5 text-[11px] font-bold text-on-accent">
                        {x.unread}
                      </span>
                    )}
                  </span>
                  <span className="text-[11px] text-muted">
                    {x.open ? t("support.open") : t("support.closed")} ·{" "}
                    {formatIso(x.updatedAt, i18n.language)}
                  </span>
                </button>
              </li>
            ))
          )}
        </ul>
      </aside>
      <section className="flex min-h-0 flex-col overflow-y-auto">
        {error && threads?.length === 0 && (
          <div className="px-4 pt-3">
            <ErrorText error={error} />
          </div>
        )}
        {current ? (
          <Conversation key={current.id} thread={current} />
        ) : (
          <NewThread onOpened={setSelected} />
        )}
      </section>
    </div>
  );
}

/** Kenar çubuğundaki "Admine yaz" sayfası. */
export function SupportPage() {
  const { t } = useTranslation();
  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <div>
        <h1 className="font-display text-3xl font-bold tracking-wide">{t("support.title")}</h1>
        <p className="mt-1 text-sm text-muted">{t("support.intro")}</p>
      </div>
      <SupportPanel />
    </div>
  );
}
