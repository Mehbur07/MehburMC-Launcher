import { Check, Layers, Loader2, Pencil, Play, Plus, Save, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
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
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { InstalledItem } from "../../lib/ipc/bindings/InstalledItem";
import type { ModGroup } from "../../lib/ipc/bindings/ModGroup";
import {
  applyPlan,
  groupCounts,
  groupState,
  newGroupId,
  refreshMembers,
  snapshot,
} from "../../lib/modGroups";
import { useApp } from "../../stores/app";

const MAX_NAME = 40;
const notify = (e: unknown) => useApp.setState({ notice: toErrorPayload(e) });

/** Name of a new group (which records the current state) or a renamed one. */
function NameDialog({
  open,
  title,
  initial,
  hint,
  onClose,
  onSave,
}: {
  open: boolean;
  title: string;
  initial: string;
  hint?: string;
  onClose: () => void;
  onSave: (name: string) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (open) setName(initial);
  }, [open, initial]);
  const trimmed = name.trim();
  const ok = trimmed !== "" && trimmed.length <= MAX_NAME && !busy;
  const save = async () => {
    if (!ok) return;
    setBusy(true);
    try {
      await onSave(trimmed);
      onClose();
    } catch {
      // Already reported; the dialog stays open.
    } finally {
      setBusy(false);
    }
  };
  return (
    <Modal
      open={open}
      onClose={onClose}
      title={title}
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
      <div className="flex flex-col gap-3">
        <Field label={t("modGroups.name")}>
          <TextInput
            value={name}
            maxLength={MAX_NAME}
            autoFocus
            placeholder={t("modGroups.namePlaceholder")}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void save();
            }}
          />
        </Field>
        {hint && <p className="text-xs text-muted">{hint}</p>}
      </div>
    </Modal>
  );
}

/**
 * "Mod groups" of the Mod Toggle page (K91, K92): a new group records which
 * mods are on and off right now; applying it brings that state back.
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
  const [creating, setCreating] = useState(false);
  const [renaming, setRenaming] = useState<ModGroup | null>(null);
  const [overwriting, setOverwriting] = useState<ModGroup | null>(null);
  const [deleting, setDeleting] = useState<ModGroup | null>(null);
  const [applying, setApplying] = useState<string | null>(null);

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
  const replace = (g: ModGroup) => persist(groups.map((x) => (x.id === g.id ? g : x)));

  const on = items.filter((i) => i.enabled).length;
  const createHint = t("modGroups.createHint", { on, off: items.length - on });

  const apply = async (g: ModGroup) => {
    const plan = applyPlan(g, items);
    setApplying(g.id);
    try {
      if (plan.enable.length > 0) await ipc.setContentEnabled(instId, "mods", plan.enable, true);
      if (plan.disable.length > 0) await ipc.setContentEnabled(instId, "mods", plan.disable, false);
      // Remember renamed files (updates) so the group keeps matching them.
      const fresh = refreshMembers(g, items);
      if (JSON.stringify(fresh) !== JSON.stringify(g)) await replace(fresh).catch(() => undefined);
    } catch (e) {
      notify(e);
    } finally {
      await onSwitched();
      setApplying(null);
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
          onClick={() => setCreating(true)}
        >
          <Plus size={14} />
          {t("modGroups.new")}
        </Button>
      </div>
      {groups.length > 0 && (
        <ul className="flex flex-wrap gap-2">
          {groups.map((g) => {
            const state = groupState(g, items);
            const counts = groupCounts(g);
            const active = state === "active";
            return (
              <li
                key={g.id}
                className={`flex min-w-64 items-center gap-2 rounded-lg border bg-surface-1/85 px-3 py-2 ${
                  active ? "border-accent/60 neon-ring" : "border-line"
                }`}
              >
                <div className="min-w-0 flex-1">
                  <p
                    className="flex items-center gap-1.5 truncate text-sm font-semibold"
                    title={g.name}
                  >
                    <span className="truncate">{g.name}</span>
                    {active && (
                      <Badge tone="success">
                        <Check size={10} />
                        {t("modGroups.active")}
                      </Badge>
                    )}
                  </p>
                  <p className="text-[11px] text-muted">{t("modGroups.counts", counts)}</p>
                </div>
                <IconButton
                  label={t("modGroups.rename", { name: g.name })}
                  onClick={() => setRenaming(g)}
                  className="h-7 w-7"
                >
                  <Pencil size={13} />
                </IconButton>
                <IconButton
                  label={t("modGroups.overwrite", { name: g.name })}
                  onClick={() => setOverwriting(g)}
                  className="h-7 w-7"
                >
                  <Save size={13} />
                </IconButton>
                <IconButton
                  label={t("modGroups.delete", { name: g.name })}
                  onClick={() => setDeleting(g)}
                  className="h-7 w-7 hover:text-danger"
                >
                  <Trash2 size={13} />
                </IconButton>
                <Button
                  size="sm"
                  variant={active ? "ghost" : "primary"}
                  disabled={disabled || applying !== null || active || state === "empty"}
                  onClick={() => void apply(g)}
                  aria-label={t("modGroups.applyNamed", { name: g.name })}
                >
                  {applying === g.id ? (
                    <Loader2 size={13} className="animate-spin" />
                  ) : (
                    <Play size={13} />
                  )}
                  {t("modGroups.apply")}
                </Button>
              </li>
            );
          })}
        </ul>
      )}

      <NameDialog
        open={creating}
        title={t("modGroups.newTitle")}
        initial=""
        hint={createHint}
        onClose={() => setCreating(false)}
        onSave={(name) => persist([...groups, { id: newGroupId(), name, mods: snapshot(items) }])}
      />
      <NameDialog
        open={renaming !== null}
        title={t("modGroups.renameTitle")}
        initial={renaming?.name ?? ""}
        onClose={() => setRenaming(null)}
        onSave={(name) => (renaming ? replace({ ...renaming, name }) : Promise.resolve())}
      />
      <ConfirmDialog
        open={overwriting !== null}
        title={t("modGroups.overwriteTitle", { name: overwriting?.name ?? "" })}
        message={createHint}
        confirmLabel={t("modGroups.overwriteConfirm")}
        onClose={() => setOverwriting(null)}
        onConfirm={() =>
          overwriting &&
          void replace({ ...overwriting, mods: snapshot(items) }).catch(() => undefined)
        }
      />
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
