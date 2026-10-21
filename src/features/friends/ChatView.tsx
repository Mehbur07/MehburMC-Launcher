import { Send } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "../../components/ui";
import type { ErrorPayload } from "../../lib/ipc/bindings/ErrorPayload";

const CHAT_POLL_MS = 3000;
export const MAX_CHARS = 2000;

/** One message as shown; `author` labels others' messages in groups. */
export interface ChatLine {
  id: number;
  mine: boolean;
  author?: string;
  body: string;
  createdAt: string;
}

export function ErrorText({ error }: { error: ErrorPayload | null }) {
  const { t } = useTranslation();
  if (!error) return null;
  return (
    <p className="text-xs text-warn">
      {t(`errors.${error.code}`, { ...error.params, defaultValue: error.detail })}
    </p>
  );
}

/** Message log with polling and a composer (friend and group chats). */
export function ChatView({
  label,
  lines,
  onPoll,
  onSend,
}: {
  label: string;
  lines: ChatLine[] | undefined;
  onPoll: () => Promise<void>;
  onSend: (body: string) => Promise<ErrorPayload | null>;
}) {
  const { t, i18n } = useTranslation();
  const [text, setText] = useState("");
  const [error, setError] = useState<ErrorPayload | null>(null);
  const [sending, setSending] = useState(false);
  const bottom = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const id = window.setInterval(() => void onPoll(), CHAT_POLL_MS);
    return () => window.clearInterval(id);
  }, [onPoll]);

  useEffect(() => {
    bottom.current?.scrollIntoView?.({ block: "end" });
  }, [lines?.length]);

  const submit = async () => {
    const body = text.trim();
    if (!body || sending) return;
    setSending(true);
    const err = await onSend(body);
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
        aria-label={label}
      >
        {lines === undefined ? (
          <p className="m-auto text-xs text-muted">{t("common.loading")}</p>
        ) : lines.length === 0 ? (
          <p className="m-auto text-xs text-muted">{t("friends.noMessages")}</p>
        ) : (
          lines.map((m) => (
            <div
              key={m.id}
              className={`flex max-w-[80%] flex-col rounded-lg px-3 py-1.5 text-sm ${
                m.mine
                  ? "self-end bg-accent/20 text-fg"
                  : "self-start border border-line bg-surface-2/80"
              }`}
            >
              {!m.mine && m.author && (
                <span className="text-[11px] font-semibold text-accent">{m.author}</span>
              )}
              <span data-selectable className="whitespace-pre-wrap break-words">
                {m.body}
              </span>
              <span className="self-end text-[10px] text-muted">{time(m.createdAt)}</span>
            </div>
          ))
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
