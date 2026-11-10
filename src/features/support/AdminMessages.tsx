import { Ban, RotateCcw, XCircle } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Badge, Button } from "../../components/ui";
import type { AdminSupportThread } from "../../lib/ipc/bindings/AdminSupportThread";
import type { ErrorPayload } from "../../lib/ipc/bindings/ErrorPayload";
import { formatIso } from "../../lib/format";
import { useSupport } from "../../stores/support";
import { ChatView, ErrorText } from "../friends/ChatView";

function playerName(x: AdminSupportThread, fallback: string) {
  return x.names[0] ?? fallback;
}

/** Admin sayfasının ban penceresine gönderilen oyuncu. */
export interface BanTarget {
  userId: string;
  label: string;
}

/** Adminin seçtiği konu: oyuncunun adı, sohbet, kapat/yeniden aç. */
function AdminConversation({
  thread,
  onBan,
}: {
  thread: AdminSupportThread;
  onBan: (target: BanTarget) => void;
}) {
  const { t } = useTranslation();
  const lines = useSupport((s) => s.messages[thread.id]);
  const poll = useSupport((s) => s.poll);
  const send = useSupport((s) => s.send);
  const setOpen = useSupport((s) => s.setOpen);
  const [error, setError] = useState<ErrorPayload | null>(null);
  const onPoll = useCallback(() => poll(thread.id), [poll, thread.id]);
  const onSend = useCallback((body: string) => send(thread.id, body), [send, thread.id]);
  const player = playerName(thread, t("admin.messages.player"));

  useEffect(() => {
    void poll(thread.id);
  }, [poll, thread.id]);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex flex-wrap items-center gap-2 border-b border-line px-4 py-3">
        <div className="min-w-0 flex-1">
          <h2 className="truncate font-semibold">{thread.subject}</h2>
          <p className="text-xs text-muted">
            {thread.names.join(", ") || t("admin.messages.player")}
          </p>
        </div>
        {thread.banned ? (
          <Badge tone="danger">{t("admin.messages.banned")}</Badge>
        ) : (
          // Kötü bir şey yazan oyuncu doğrudan buradan banlanır.
          <Button
            size="sm"
            variant="ghost"
            onClick={() => onBan({ userId: thread.owner, label: player })}
          >
            <Ban size={13} />
            {t("admin.messages.ban")}
          </Button>
        )}
        <Button
          size="sm"
          variant="ghost"
          onClick={async () => setError(await setOpen(thread.id, !thread.open))}
        >
          {thread.open ? <XCircle size={13} /> : <RotateCcw size={13} />}
          {thread.open ? t("admin.messages.close") : t("admin.messages.reopen")}
        </Button>
      </div>
      <ErrorText error={error} />
      <ChatView
        label={thread.subject}
        lines={lines?.map((m) => ({
          id: m.id,
          mine: m.mine,
          author: m.fromAdmin ? (m.author ?? t("support.team")) : player,
          body: m.body,
          createdAt: m.createdAt,
        }))}
        onPoll={onPoll}
        onSend={onSend}
      />
    </div>
  );
}

/** Admin sayfasındaki "Mesajlar" sekmesi (K101). `version` ban sonrası
 * listeyi tazeler (banlı rozeti). */
export function AdminMessages({
  onBan,
  version,
}: {
  onBan: (target: BanTarget) => void;
  version: number;
}) {
  const { t, i18n } = useTranslation();
  const [open, setOpenList] = useState(true);
  const [selected, setSelected] = useState<number | null>(null);
  const list = useSupport((s) => (open ? s.adminThreads.open : s.adminThreads.closed));
  const error = useSupport((s) => s.error);
  const load = useSupport((s) => s.loadAdmin);

  useEffect(() => {
    void load(open);
  }, [load, open, version]);

  const current = list?.find((x) => x.id === selected) ?? null;

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <p className="text-xs text-muted">{t("admin.messages.hint")}</p>
      <div className="flex gap-1">
        {[true, false].map((o) => (
          <Button
            key={String(o)}
            size="sm"
            variant={open === o ? "primary" : "ghost"}
            onClick={() => {
              setOpenList(o);
              setSelected(null);
            }}
          >
            {o ? t("admin.messages.openList") : t("admin.messages.closedList")}
          </Button>
        ))}
      </div>
      <ErrorText error={error} />
      <div className="grid min-h-[420px] flex-1 grid-cols-[300px_1fr] overflow-hidden rounded-lg border border-line bg-surface-1/85">
        <ul className="flex min-h-0 flex-col gap-1 overflow-y-auto border-r border-line p-2">
          {list === null ? (
            <li className="p-2 text-xs text-muted">{t("common.loading")}</li>
          ) : list.length === 0 ? (
            <li className="p-2 text-xs text-muted">
              {open ? t("admin.messages.empty") : t("admin.messages.emptyClosed")}
            </li>
          ) : (
            list.map((x) => (
              <li key={x.id}>
                <button
                  type="button"
                  onClick={() => setSelected(x.id)}
                  aria-current={x.id === selected ? "true" : undefined}
                  className={`flex w-full flex-col gap-0.5 rounded-md px-2 py-1.5 text-left transition-colors ${
                    x.id === selected ? "bg-accent/10" : "hover:bg-surface-2"
                  }`}
                >
                  <span className="flex items-center gap-2">
                    <span className="min-w-0 flex-1 truncate text-sm font-semibold">
                      {playerName(x, t("admin.messages.player"))}
                    </span>
                    {x.banned && <Badge tone="danger">{t("admin.messages.banned")}</Badge>}
                    {x.unread > 0 && (
                      <span className="rounded-full bg-accent px-1.5 text-[11px] font-bold text-on-accent">
                        {x.unread}
                      </span>
                    )}
                  </span>
                  <span className="truncate text-xs">{x.subject}</span>
                  <span className="truncate text-[11px] text-muted">
                    {x.lastFromAdmin ? `${t("support.team")}: ` : ""}
                    {x.preview}
                  </span>
                  <span className="text-[10px] text-muted">
                    {formatIso(x.updatedAt, i18n.language)}
                  </span>
                </button>
              </li>
            ))
          )}
        </ul>
        <section className="flex min-h-0 flex-col">
          {current ? (
            <AdminConversation key={current.id} thread={current} onBan={onBan} />
          ) : (
            <p className="m-auto text-xs text-muted">{t("admin.messages.pick")}</p>
          )}
        </section>
      </div>
    </div>
  );
}
