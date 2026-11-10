import { create } from "zustand";

import { ipc, toErrorPayload } from "../lib/ipc";
import type { AdminSupportThread } from "../lib/ipc/bindings/AdminSupportThread";
import type { ErrorPayload } from "../lib/ipc/bindings/ErrorPayload";
import type { SupportMessage } from "../lib/ipc/bindings/SupportMessage";
import type { SupportThread } from "../lib/ipc/bindings/SupportThread";
import type { SupportUnread } from "../lib/ipc/bindings/SupportUnread";

/** Admine mesaj (K101): oyuncunun konuları, açık sohbetler ve rozetler. */
interface SupportStore {
  unread: SupportUnread;
  /** null: henüz yüklenmedi. */
  threads: SupportThread[] | null;
  /** Adminlerin listesi: açık ve kapalı konular ayrı. */
  adminThreads: { open: AdminSupportThread[] | null; closed: AdminSupportThread[] | null };
  /** Konu id'sine göre mesajlar (oyuncu ve admin ekranı aynı önbelleği kullanır). */
  messages: Record<number, SupportMessage[]>;
  error: ErrorPayload | null;

  refreshUnread: () => Promise<void>;
  loadThreads: () => Promise<void>;
  loadAdmin: (open: boolean) => Promise<void>;
  /** Yeni konu; başarılıysa konunun id'si, değilse hata. */
  open: (subject: string, body: string) => Promise<number | ErrorPayload>;
  /** İlk açılışta bütün mesajlar, sonra yalnızca yenileri; konuyu okundu sayar. */
  poll: (thread: number) => Promise<void>;
  send: (thread: number, body: string) => Promise<ErrorPayload | null>;
  setOpen: (thread: number, open: boolean) => Promise<ErrorPayload | null>;
}

const NONE: SupportUnread = { mine: 0, admin: 0 };

export const useSupport = create<SupportStore>((set, get) => {
  /** Değişiklikten sonra listeleri ve rozetleri tazeler. */
  const refreshLists = async () => {
    await Promise.all([
      get().refreshUnread(),
      get().threads !== null ? get().loadThreads() : null,
      get().adminThreads.open !== null ? get().loadAdmin(true) : null,
      get().adminThreads.closed !== null ? get().loadAdmin(false) : null,
    ]);
  };

  return {
    unread: NONE,
    threads: null,
    adminThreads: { open: null, closed: null },
    messages: {},
    error: null,

    refreshUnread: async () => {
      try {
        set({ unread: await ipc.supportUnread() });
      } catch {
        // Rozet sessizce eski haliyle kalır (çevrimdışı vb.).
      }
    },

    loadThreads: async () => {
      try {
        set({ threads: await ipc.supportThreads(), error: null });
      } catch (e) {
        set({ error: toErrorPayload(e), threads: get().threads ?? [] });
      }
    },

    loadAdmin: async (open) => {
      const key = open ? "open" : "closed";
      try {
        const list = await ipc.adminSupportThreads(open);
        set({ adminThreads: { ...get().adminThreads, [key]: list }, error: null });
      } catch (e) {
        set({
          error: toErrorPayload(e),
          adminThreads: { ...get().adminThreads, [key]: get().adminThreads[key] ?? [] },
        });
      }
    },

    open: async (subject, body) => {
      try {
        const id = await ipc.supportOpen(subject, body);
        await refreshLists();
        return id;
      } catch (e) {
        return toErrorPayload(e);
      }
    },

    poll: async (thread) => {
      const known = get().messages[thread];
      const last = known?.at(-1)?.id;
      try {
        const fresh = await ipc.supportMessages(thread, last);
        if (known && fresh.length === 0) return;
        set({ messages: { ...get().messages, [thread]: [...(known ?? []), ...fresh] } });
        // Karşı taraftan yeni bir şey geldiyse (ya da ilk açılışsa) okundu say.
        if (!known || fresh.some((m) => !m.mine)) {
          await ipc.supportMarkRead(thread).catch(() => undefined);
          await refreshLists();
        }
      } catch (e) {
        if (!known) set({ messages: { ...get().messages, [thread]: [] } });
        set({ error: toErrorPayload(e) });
      }
    },

    send: async (thread, body) => {
      try {
        await ipc.supportSend(thread, body);
        await get().poll(thread);
        await refreshLists();
        return null;
      } catch (e) {
        return toErrorPayload(e);
      }
    },

    setOpen: async (thread, open) => {
      try {
        await ipc.supportSetOpen(thread, open);
        await refreshLists();
        return null;
      } catch (e) {
        return toErrorPayload(e);
      }
    },
  };
});
