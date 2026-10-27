import { Layers, ListChecks, Pencil, Plus, Search, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  Badge,
  Button,
  ConfirmDialog,
  Field,
  IconButton,
  Modal,
  TextInput,
  Toggle,
} from "../../components/ui";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { InstalledItem } from "../../lib/ipc/bindings/InstalledItem";
import type { ModGroup } from "../../lib/ipc/bindings/ModGroup";
import {
  filesToSwitch,
  groupItems,
  groupState,
  isMember,
  memberFromItem,
  modLabel,
  newGroupId,
  refreshMembers,
} from "../../lib/modGroups";
import { useApp } from "../../stores/app";

const MAX_NAME = 40;
const notify = (e: unknown) => useApp.setState({ notice: toErrorPayload(e) });

/** Name and mods of a new or existing group. */
function GroupEditor({
  group,
  items,
  onClose,
  onSave,
}: {
  /** `null` = closed; a group without mods and name = new. */
  group: ModGroup | null;
  items: InstalledItem[];
  onClose: () => void;
  onSave: (g: ModGroup) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState("");
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);

  // Filled when a group opens; a rescan meanwhile keeps the user's picks.
  const [openedId, setOpenedId] = useState<string | null>(null);
  if (group && group.id !== openedId) {
    setOpenedId(group.id);
    setName(group.name);
    setPicked(new Set(groupItems(group, items).map((i) => i.fileName)));
    setQuery("");
  } else if (!group && openedId !== null) {
    setOpenedId(null);
  }

  const sorted = useMemo(() => {
    const q = query.trim().toLocaleLowerCase();
    return items
      .filter((i) => !q || modLabel(i).toLocaleLowerCase().includes(q))
      .sort((a, b) => modLabel(a).localeCompare(modLabel(b)));
  }, [items, query]);
  // Members whose mod is not installed now stay in the group.
  const missing = group ? group.mods.filter((m) => !items.some((i) => isMember(m, i))) : [];

  const trimmed = name.trim();
  const save = async () => {
    if (!group) return;
    setBusy(true);
    try {
      await onSave({
        ...group,
        name: trimmed,
        mods: [...items.filter((i) => picked.has(i.fileName)).map(memberFromItem), ...missing],
      });
      onClose();
    } finally {
      setBusy(false);
    }
  };

  const flip = (file: string) =>
    setPicked((s) => {
      const next = new Set(s);
      if (next.has(file)) next.delete(file);
      else next.add(file);
      return next;
    });

  return (
    <Modal
      open={group !== null}
      onClose={onClose}
      wide
      title={group?.name ? t("modGroups.editTitle") : t("modGroups.newTitle")}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button
            variant="primary"
            disabled={busy || trimmed === "" || trimmed.length > MAX_NAME}
            onClick={() => void save()}
          >
            {t("common.save")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-3">
        <Field label={t("modGroups.name")}>
          <TextInput
            value={name}
            maxLength={MAX_NAME}
            placeholder={t("modGroups.namePlaceholder")}
            onChange={(e) => setName(e.target.value)}
          />
        </Field>
        <div className="flex items-center justify-between gap-2">
          <p className="text-xs font-semibold tracking-wide text-muted uppercase">
            {t("modGroups.mods", { count: picked.size })}
          </p>
          <div className="relative w-56">
            <Search
              size={14}
              className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-muted"
            />
            <TextInput
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t("modToggle.search")}
              aria-label={t("modToggle.search")}
              className="h-8 pl-8 text-xs"
            />
          </div>
        </div>
        <ul className="flex max-h-80 flex-col gap-0.5 overflow-y-auto rounded-md border border-line p-1">
          {sorted.map((i) => (
            <li key={i.fileName}>
              <label className="flex cursor-pointer items-center gap-2 rounded px-2 py-1 text-sm hover:bg-surface-2">
                <input
                  type="checkbox"
                  checked={picked.has(i.fileName)}
                  onChange={() => flip(i.fileName)}
                  className="accent-[rgb(var(--mc-accent-rgb))]"
                />
                <span className="min-w-0 flex-1 truncate">{modLabel(i)}</span>
                {!i.enabled && <span className="text-[11px] text-muted">{t("modToggle.off")}</span>}
              </label>
            </li>
          ))}
        </ul>
        {missing.length > 0 && (
          <p className="text-xs text-muted">
            {t("modGroups.missing", {
              names: missing.map((m) => m.title ?? m.file).join(", "),
            })}
          </p>
        )}
      </div>
    </Modal>
  );
}

function RenameDialog({
  group,
  onClose,
  onSave,
}: {
  group: ModGroup | null;
  onClose: () => void;
  onSave: (g: ModGroup) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState("");
  useEffect(() => {
    if (group) setName(group.name);
  }, [group]);
  const trimmed = name.trim();
  const ok = trimmed !== "" && trimmed.length <= MAX_NAME;
  const save = async () => {
    if (!group || !ok) return;
    await onSave({ ...group, name: trimmed });
    onClose();
  };
  return (
    <Modal
      open={group !== null}
      onClose={onClose}
      title={t("modGroups.renameTitle")}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button variant="primary" disabled={!ok} onClick={() => void save()}>
            {t("common.save")}
          </Button>
        </>
      }
    >
      <Field label={t("modGroups.name")}>
        <TextInput
          value={name}
          maxLength={MAX_NAME}
          autoFocus
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void save();
          }}
        />
      </Field>
    </Modal>
  );
}

/**
 * "Mod groups" of the Mod Toggle page (K91): named sets of mods switched on
 * or off with one click, like the Switch mod.
 */
export function ModGroups({
  instId,
  items,
  disabled,
  onSwitched,
}: {
  instId: string;
  items: InstalledItem[];
  /** Game running or another switch in progress. */
  disabled: boolean;
  /** After a group switched mods: rescan. */
  onSwitched: () => Promise<void>;
}) {
  const { t } = useTranslation();
  const [groups, setGroups] = useState<ModGroup[]>([]);
  const [editing, setEditing] = useState<ModGroup | null>(null);
  const [renaming, setRenaming] = useState<ModGroup | null>(null);
  const [deleting, setDeleting] = useState<ModGroup | null>(null);
  const [switching, setSwitching] = useState<string | null>(null);

  useEffect(() => {
    let gone = false;
    setGroups([]);
    ipc
      .modGroups(instId)
      .then((g) => !gone && setGroups(g))
      .catch(notify);
    return () => {
      gone = true;
    };
  }, [instId]);

  const persist = async (next: ModGroup[]) => {
    try {
      setGroups(await ipc.saveModGroups(instId, next));
    } catch (e) {
      notify(e);
      throw e;
    }
  };
  const upsert = (g: ModGroup) =>
    persist(
      groups.some((x) => x.id === g.id)
        ? groups.map((x) => (x.id === g.id ? g : x))
        : [...groups, g],
    );

  const flip = async (g: ModGroup, enable: boolean) => {
    const files = filesToSwitch(g, groups, items, enable);
    setSwitching(g.id);
    try {
      if (files.length > 0) await ipc.setContentEnabled(instId, "mods", files, enable);
      // Remember renamed files (updates) so the group keeps matching them.
      const fresh = refreshMembers(g, items);
      if (JSON.stringify(fresh) !== JSON.stringify(g)) await upsert(fresh).catch(() => undefined);
    } catch (e) {
      notify(e);
    } finally {
      await onSwitched();
      setSwitching(null);
    }
  };

  return (
    <section aria-label={t("modGroups.title")} className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <Layers size={15} className="text-accent" />
        <h2 className="text-sm font-semibold">{t("modGroups.title")}</h2>
        <span className="text-xs text-muted">{t("modGroups.hint")}</span>
        <Button
          size="sm"
          className="ml-auto"
          disabled={groups.length >= 50}
          onClick={() => setEditing({ id: newGroupId(), name: "", mods: [] })}
        >
          <Plus size={14} />
          {t("modGroups.new")}
        </Button>
      </div>
      {groups.length > 0 && (
        <ul className="flex flex-wrap gap-2">
          {groups.map((g) => {
            const state = groupState(g, items);
            const count = groupItems(g, items).length;
            return (
              <li
                key={g.id}
                className={`flex min-w-56 items-center gap-2 rounded-lg border bg-surface-1/85 px-3 py-2 ${
                  state === "on" ? "border-accent/50" : "border-line"
                }`}
              >
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-semibold" title={g.name}>
                    {g.name}
                  </p>
                  <p className="flex items-center gap-1.5 text-[11px] text-muted">
                    {t("modGroups.count", { count })}
                    {state === "mixed" && <Badge tone="warn">{t("modGroups.mixed")}</Badge>}
                  </p>
                </div>
                <IconButton
                  label={t("modGroups.rename", { name: g.name })}
                  onClick={() => setRenaming(g)}
                  className="h-7 w-7"
                >
                  <Pencil size={13} />
                </IconButton>
                <IconButton
                  label={t("modGroups.edit", { name: g.name })}
                  onClick={() => setEditing(g)}
                  className="h-7 w-7"
                >
                  <ListChecks size={13} />
                </IconButton>
                <IconButton
                  label={t("modGroups.delete", { name: g.name })}
                  onClick={() => setDeleting(g)}
                  className="h-7 w-7 hover:text-danger"
                >
                  <Trash2 size={13} />
                </IconButton>
                <Toggle
                  checked={state === "on"}
                  disabled={disabled || switching !== null || state === "empty"}
                  onChange={() => void flip(g, state !== "on")}
                  label={t("modGroups.switch", { name: g.name })}
                />
              </li>
            );
          })}
        </ul>
      )}

      <GroupEditor group={editing} items={items} onClose={() => setEditing(null)} onSave={upsert} />
      <RenameDialog group={renaming} onClose={() => setRenaming(null)} onSave={upsert} />
      <ConfirmDialog
        open={deleting !== null}
        danger
        title={t("modGroups.deleteTitle", { name: deleting?.name ?? "" })}
        message={t("modGroups.deleteMessage")}
        confirmLabel={t("common.delete")}
        onClose={() => setDeleting(null)}
        onConfirm={() =>
          deleting &&
          void persist(groups.filter((g) => g.id !== deleting.id)).catch(() => undefined)
        }
      />
    </section>
  );
}
