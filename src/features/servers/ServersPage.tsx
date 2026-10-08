import {
  Check,
  Copy,
  Loader2,
  Play,
  Plus,
  RefreshCw,
  Server,
  Star,
  Trash2,
  Users,
  WifiOff,
} from "lucide-react";
import { useEffect, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";

import {
  Badge,
  Button,
  ConfirmDialog,
  EmptyState,
  Field,
  IconButton,
  Modal,
  TextInput,
} from "../../components/ui";
import type { ErrorPayload } from "../../lib/ipc/bindings/ErrorPayload";
import type { Instance } from "../../lib/ipc/bindings/Instance";
import type { MotdSpan } from "../../lib/ipc/bindings/MotdSpan";
import { useApp } from "../../stores/app";
import { selectedInstance, useInstances } from "../../stores/instances";
import { addressKey, isFavorite, useServers, type PingState } from "../../stores/servers";
import { selectedAccount, useAccounts } from "../../stores/accounts";
import { activeTaskFor, playBlocked, useTasks } from "../../stores/tasks";

/** Server description with its colours and styles. */
export function Motd({ spans }: { spans: MotdSpan[] }) {
  return (
    <p className="line-clamp-2 font-mono text-xs leading-snug whitespace-pre-wrap text-muted">
      {spans.map((s, i) => (
        <span
          key={i}
          style={{ color: s.color ?? undefined }}
          className={[
            s.bold && "font-bold",
            s.italic && "italic",
            s.underlined && "underline",
            s.strikethrough && "line-through",
          ]
            .filter(Boolean)
            .join(" ")}
        >
          {s.text}
        </span>
      ))}
    </p>
  );
}

function pingTone(ms: number) {
  if (ms < 150) return "text-success";
  if (ms < 300) return "text-warn";
  return "text-danger";
}

/** Signal bars like the in-game list. */
function PingBars({ ms }: { ms: number }) {
  const bars = ms < 150 ? 5 : ms < 300 ? 4 : ms < 600 ? 3 : ms < 1000 ? 2 : 1;
  return (
    <span className={`flex items-end gap-px ${pingTone(ms)}`} aria-hidden>
      {[1, 2, 3, 4, 5].map((b) => (
        <span
          key={b}
          className={`w-[3px] rounded-[1px] ${b <= bars ? "bg-current" : "bg-surface-3"}`}
          style={{ height: 3 + b * 2 }}
        />
      ))}
    </span>
  );
}

function StatusLine({ ping }: { ping: PingState | undefined }) {
  const { t } = useTranslation();
  if (!ping || ping.state === "pending") {
    return (
      <span className="flex items-center gap-1.5 text-xs text-muted">
        <Loader2 size={12} className="animate-spin" />
        {t("servers.pinging")}
      </span>
    );
  }
  if (ping.state === "error") {
    return (
      <span
        className="flex items-center gap-1.5 text-xs text-danger"
        title={t(`errors.${ping.code}`, { defaultValue: t("errors.unknown") })}
      >
        <WifiOff size={12} />
        {t("servers.offline")}
      </span>
    );
  }
  const s = ping.status;
  return (
    <span className="flex items-center gap-3 text-xs">
      <span
        className="flex items-center gap-1 text-fg"
        title={s.playersSample.length ? s.playersSample.join("\n") : undefined}
      >
        <Users size={12} className="text-muted" />
        {t("servers.players", {
          online: s.playersOnline.toLocaleString(),
          max: s.playersMax.toLocaleString(),
        })}
      </span>
      <span
        className="flex items-center gap-1.5"
        title={t("servers.version", { version: s.version })}
      >
        <PingBars ms={s.latencyMs} />
        <span className={pingTone(s.latencyMs)}>{t("servers.ping", { ms: s.latencyMs })}</span>
      </span>
    </span>
  );
}

interface RowProps {
  name: string;
  address: string;
  storedIcon?: string | null;
  inst: Instance | null;
  favorite: boolean;
  onToggleFavorite: () => void;
  onRemove?: () => void;
}

function ServerRow({
  name,
  address,
  storedIcon,
  inst,
  favorite,
  onToggleFavorite,
  onRemove,
}: RowProps) {
  const { t } = useTranslation();
  const ping = useServers((s) => s.pings[addressKey(address)]);
  const join = useServers((s) => s.join);
  const setView = useApp((s) => s.setView);
  const account = useAccounts(selectedAccount);
  const busy = useTasks((s) => playBlocked(s.tasks, inst?.id ?? null, account?.name ?? null));
  const [copied, setCopied] = useState(false);

  const icon = (ping?.state === "ok" && ping.status.icon) || storedIcon || null;
  const motd = ping?.state === "ok" ? ping.status.motd : [];

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(address);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard may be unavailable; the address is visible anyway.
    }
  };

  const onJoin = async () => {
    if (!inst) return;
    if (await join(inst.id, address)) setView("home");
  };

  return (
    <li className="group flex items-center gap-4 rounded-lg border border-line bg-surface-1/80 p-3 backdrop-blur transition-colors hover:border-accent/40">
      <div className="grid h-14 w-14 shrink-0 place-items-center overflow-hidden rounded-md border border-line bg-surface-2">
        {icon ? (
          <img src={icon} alt="" className="h-full w-full [image-rendering:pixelated]" />
        ) : (
          <Server size={26} className="text-muted" />
        )}
      </div>

      <div className="min-w-0 flex-1">
        <div className="flex min-w-0 items-baseline gap-2">
          <span className="truncate font-display text-base font-semibold" title={name}>
            {name}
          </span>
          <span className="truncate font-mono text-[11px] text-muted" data-selectable>
            {address}
          </span>
        </div>
        {motd.length > 0 && <Motd spans={motd} />}
        <div className="mt-1">
          <StatusLine ping={ping} />
        </div>
      </div>

      <div className="flex shrink-0 items-center gap-1">
        <IconButton
          label={favorite ? t("servers.unfavorite") : t("servers.favorite")}
          onClick={onToggleFavorite}
        >
          <Star
            size={16}
            fill={favorite ? "currentColor" : "none"}
            className={favorite ? "text-accent neon-drop" : ""}
          />
        </IconButton>
        <IconButton label={copied ? t("servers.copied") : t("servers.copy")} onClick={copy}>
          {copied ? <Check size={16} /> : <Copy size={16} />}
        </IconButton>
        {onRemove && (
          <IconButton label={t("servers.remove")} onClick={onRemove} className="hover:text-danger">
            <Trash2 size={16} />
          </IconButton>
        )}
        <Button
          variant="primary"
          className="ml-2"
          disabled={!inst || busy}
          title={inst ? t("servers.joinTitle", { instance: inst.name, server: name }) : undefined}
          onClick={() => void onJoin()}
        >
          <Play size={15} />
          {busy ? t("servers.running") : t("servers.join")}
        </Button>
      </div>
    </li>
  );
}

function AddServerDialog({
  open,
  onClose,
  inst,
}: {
  open: boolean;
  onClose: () => void;
  inst: Instance | null;
}) {
  const { t } = useTranslation();
  const addFavorite = useServers((s) => s.addFavorite);
  const addToGame = useServers((s) => s.addToGame);
  const [name, setName] = useState("");
  const [address, setAddress] = useState("");
  const [target, setTarget] = useState<"favorites" | "game">("favorites");
  const [error, setError] = useState<ErrorPayload | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (open) {
      setName("");
      setAddress("");
      setError(null);
    }
  }, [open]);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    const err =
      target === "game" && inst
        ? await addToGame(inst.id, name, address)
        : await addFavorite(name, address);
    setSaving(false);
    setError(err);
    if (!err) {
      void useServers.getState().ping(address, true);
      onClose();
    }
  };

  return (
    <Modal open={open} onClose={onClose} title={t("servers.add")}>
      <form id="add-server" className="flex flex-col gap-4" onSubmit={(e) => void submit(e)}>
        <Field label={t("servers.name")}>
          <TextInput
            value={name}
            maxLength={64}
            placeholder={t("servers.namePlaceholder")}
            onChange={(e) => setName(e.target.value)}
          />
        </Field>
        <Field label={t("servers.address")}>
          <TextInput
            value={address}
            autoFocus
            maxLength={261}
            spellCheck={false}
            placeholder={t("servers.addressPlaceholder")}
            onChange={(e) => setAddress(e.target.value)}
          />
        </Field>
        {inst && (
          <fieldset className="flex flex-col gap-1.5">
            <legend className="mb-1.5 text-xs font-semibold tracking-wide text-muted uppercase">
              {t("servers.addTo")}
            </legend>
            {(["favorites", "game"] as const).map((k) => (
              <label key={k} className="flex items-center gap-2 text-sm">
                <input
                  type="radio"
                  name="target"
                  checked={target === k}
                  onChange={() => setTarget(k)}
                  className="accent-[rgb(var(--mc-accent-rgb))]"
                />
                {k === "favorites"
                  ? t("servers.toFavorites")
                  : t("servers.toGame", { instance: inst.name })}
              </label>
            ))}
          </fieldset>
        )}
        {error && (
          <p role="alert" className="text-sm text-danger">
            {t(`errors.${error.code}`, { ...error.params, defaultValue: error.detail })}
          </p>
        )}
      </form>
      <div className="mt-5 flex justify-end gap-2">
        <Button variant="ghost" onClick={onClose}>
          {t("common.cancel")}
        </Button>
        <Button
          variant="primary"
          type="submit"
          form="add-server"
          disabled={saving || address.trim() === ""}
        >
          <Plus size={15} />
          {t("servers.add")}
        </Button>
      </div>
    </Modal>
  );
}

type Removal =
  | { kind: "favorite"; id: string; name: string; address: string }
  | { kind: "game"; index: number; name: string; address: string };

export function ServersPage() {
  const { t } = useTranslation();
  const instances = useInstances((s) => s.instances);
  const inst = useInstances(selectedInstance);
  const select = useInstances((s) => s.select);
  const favorites = useServers((s) => s.favorites);
  const game = useServers((s) => (inst ? s.game[inst.id] : undefined));
  const loadFavorites = useServers((s) => s.loadFavorites);
  const loadGame = useServers((s) => s.loadGame);
  const ping = useServers((s) => s.ping);
  const addFavorite = useServers((s) => s.addFavorite);
  const removeFavorite = useServers((s) => s.removeFavorite);
  const removeFromGame = useServers((s) => s.removeFromGame);
  const running = useTasks((s) => activeTaskFor(s.tasks, inst?.id ?? null) !== null);
  const [adding, setAdding] = useState(false);
  const [removal, setRemoval] = useState<Removal | null>(null);

  useEffect(() => {
    void loadFavorites();
  }, [loadFavorites]);

  useEffect(() => {
    if (inst) void loadGame(inst.id);
  }, [inst, loadGame]);

  // Ping everything that is listed (cached for 30 s).
  const addresses = [...favorites.map((f) => f.address), ...(game ?? []).map((g) => g.address)];
  const addressList = addresses.join("\n");
  useEffect(() => {
    for (const a of addressList.split("\n")) if (a) void ping(a);
  }, [addressList, ping]);

  const refresh = () => {
    void loadFavorites();
    if (inst) void loadGame(inst.id);
    for (const a of addresses) void ping(a, true);
  };

  const toggleFavorite = (name: string, address: string) => {
    const fav = favorites.find((f) => addressKey(f.address) === addressKey(address));
    if (fav) void removeFavorite(fav.id);
    else void addFavorite(name, address).then((e) => e && useApp.setState({ notice: e }));
  };

  return (
    <div className="flex flex-col gap-5">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h1 className="font-display text-3xl font-bold tracking-wide">{t("servers.title")}</h1>
          <p className="mt-1 text-sm text-muted">{t("servers.subtitle")}</p>
        </div>
        <div className="flex items-center gap-2">
          {instances.length > 0 && (
            <label className="flex items-center gap-2 text-sm">
              <span className="text-muted">{t("servers.profile")}</span>
              <select
                value={inst?.id ?? ""}
                onChange={(e) => void select(e.target.value)}
                className="rounded-md border border-line bg-surface-2 px-2 py-1.5 text-sm focus:border-accent focus:outline-none"
              >
                {instances.map((i) => (
                  <option key={i.id} value={i.id}>
                    {i.name} ({i.mcVersion})
                  </option>
                ))}
              </select>
            </label>
          )}
          <IconButton label={t("servers.refresh")} onClick={refresh}>
            <RefreshCw size={16} />
          </IconButton>
          <Button variant="primary" onClick={() => setAdding(true)}>
            <Plus size={16} />
            {t("servers.add")}
          </Button>
        </div>
      </div>

      {instances.length === 0 && <Badge tone="warn">{t("servers.noInstances")}</Badge>}

      <section className="flex flex-col gap-2">
        <h2 className="font-display text-lg font-semibold tracking-wide">
          {t("servers.favorites")}
        </h2>
        {favorites.length === 0 ? (
          <EmptyState icon={<Star size={30} />} title={t("servers.favorites")}>
            {t("servers.favoritesEmpty")}
          </EmptyState>
        ) : (
          <ul className="flex flex-col gap-2">
            {favorites.map((f) => (
              <ServerRow
                key={f.id}
                name={f.name}
                address={f.address}
                inst={inst}
                favorite
                onToggleFavorite={() => toggleFavorite(f.name, f.address)}
                onRemove={() =>
                  setRemoval({ kind: "favorite", id: f.id, name: f.name, address: f.address })
                }
              />
            ))}
          </ul>
        )}
      </section>

      {inst && (
        <section className="flex flex-col gap-2">
          <div className="flex flex-wrap items-baseline gap-2">
            <h2 className="font-display text-lg font-semibold tracking-wide">
              {t("servers.gameList")}
            </h2>
            <span className="text-xs text-muted">{t("servers.gameListHint")}</span>
            {running && <Badge tone="warn">{t("servers.lockedWhileRunning")}</Badge>}
          </div>
          {game && game.length === 0 ? (
            <EmptyState icon={<Server size={30} />} title={t("servers.gameListEmpty")} />
          ) : (
            <ul className="flex flex-col gap-2">
              {(game ?? []).map((g) => (
                <ServerRow
                  key={`${g.index}-${g.address}`}
                  name={g.name || g.address}
                  address={g.address}
                  storedIcon={g.icon}
                  inst={inst}
                  favorite={isFavorite(favorites, g.address)}
                  onToggleFavorite={() => toggleFavorite(g.name || g.address, g.address)}
                  onRemove={
                    running
                      ? undefined
                      : () =>
                          setRemoval({
                            kind: "game",
                            index: g.index,
                            name: g.name || g.address,
                            address: g.address,
                          })
                  }
                />
              ))}
            </ul>
          )}
        </section>
      )}

      <AddServerDialog open={adding} onClose={() => setAdding(false)} inst={inst} />

      <ConfirmDialog
        open={removal !== null}
        danger
        title={
          removal?.kind === "game" ? t("servers.removeGameTitle") : t("servers.removeFavoriteTitle")
        }
        message={t("servers.removeMessage", {
          name: removal?.name ?? "",
          address: removal?.address ?? "",
        })}
        confirmLabel={t("common.delete")}
        onClose={() => setRemoval(null)}
        onConfirm={() => {
          if (!removal) return;
          if (removal.kind === "favorite") void removeFavorite(removal.id);
          else if (inst)
            void removeFromGame(inst.id, {
              index: removal.index,
              name: removal.name,
              address: removal.address,
              icon: null,
            });
        }}
      />
    </div>
  );
}
