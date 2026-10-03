import {
  DndContext,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import { SortableContext, arrayMove, rectSortingStrategy, useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import {
  Boxes,
  Copy,
  Download,
  FolderOpen,
  GripVertical,
  Info,
  MoreVertical,
  Play,
  Plus,
  Square,
  Trash2,
  Upload,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { InstanceIcon } from "../../components/InstanceIcon";
import {
  Badge,
  Button,
  ConfirmDialog,
  EmptyState,
  IconButton,
  Modal,
  TextInput,
} from "../../components/ui";
import { openFolder, play, stop } from "../../lib/actions";
import { formatDuration } from "../../lib/format";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { Instance } from "../../lib/ipc/bindings/Instance";
import { useApp } from "../../stores/app";
import { useInstances } from "../../stores/instances";
import { activeTaskFor, useTasks } from "../../stores/tasks";

export function loaderLabel(kind: Instance["loader"]["kind"]) {
  return {
    vanilla: "Vanilla",
    fabric: "Fabric",
    quilt: "Quilt",
    legacyFabric: "Legacy Fabric",
    forge: "Forge",
    neoForge: "NeoForge",
    optifine: "OptiFine",
  }[kind];
}

export function usePlayTimeUnits() {
  const { t } = useTranslation();
  return { h: t("units.h"), m: t("units.m"), s: t("units.s") };
}

export function InstancesPage() {
  const { t } = useTranslation();
  const instances = useInstances((s) => s.instances);
  const reorder = useInstances((s) => s.reorder);
  const load = useInstances((s) => s.load);
  const setWizard = useApp((s) => s.setWizard);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 6 } }));

  const onDragEnd = (e: DragEndEvent) => {
    if (!e.over || e.active.id === e.over.id) return;
    const ids = instances.map((i) => i.id);
    const from = ids.indexOf(String(e.active.id));
    const to = ids.indexOf(String(e.over.id));
    void reorder(arrayMove(ids, from, to));
  };

  const importZip = async () => {
    const src = await openDialog({
      multiple: false,
      filters: [{ name: "Zip", extensions: ["zip"] }],
    });
    if (typeof src !== "string") return;
    try {
      await ipc.importInstance(src);
      await load();
    } catch (e) {
      useApp.setState({ notice: toErrorPayload(e) });
    }
  };

  return (
    <div className="flex flex-col gap-5 pb-6">
      <div className="flex items-center justify-between">
        <h1 className="font-display text-3xl font-bold tracking-wide">{t("nav.instances")}</h1>
        <div className="flex gap-2">
          <Button onClick={() => void importZip()}>
            <Upload size={15} />
            {t("instances.import")}
          </Button>
          <Button variant="primary" onClick={() => setWizard(true)}>
            <Plus size={16} />
            {t("instances.create")}
          </Button>
        </div>
      </div>

      {instances.length === 0 ? (
        <EmptyState icon={<Boxes size={36} />} title={t("instances.emptyTitle")}>
          <p>{t("instances.emptyBody")}</p>
          <Button variant="primary" className="mt-4" onClick={() => setWizard(true)}>
            <Plus size={16} />
            {t("instances.create")}
          </Button>
        </EmptyState>
      ) : (
        <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
          <SortableContext items={instances.map((i) => i.id)} strategy={rectSortingStrategy}>
            <div className="grid grid-cols-[repeat(auto-fill,minmax(250px,1fr))] gap-4">
              {instances.map((i) => (
                <InstanceCard key={i.id} inst={i} />
              ))}
            </div>
          </SortableContext>
        </DndContext>
      )}
    </div>
  );
}

function InstanceCard({ inst }: { inst: Instance }) {
  const { t } = useTranslation();
  const units = usePlayTimeUnits();
  const openInstance = useApp((s) => s.openInstance);
  const selected = useInstances((s) => s.selected === inst.id);
  const select = useInstances((s) => s.select);
  const remove = useInstances((s) => s.remove);
  const copy = useInstances((s) => s.copy);
  const task = useTasks((s) => activeTaskFor(s.tasks, inst.id));
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: inst.id,
  });

  const [menu, setMenu] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [copyName, setCopyName] = useState<string | null>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!menu) return;
    const close = (e: MouseEvent) => {
      if (!menuRef.current?.contains(e.target as Node)) setMenu(false);
    };
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [menu]);

  const exportZip = async () => {
    const dest = await saveDialog({
      defaultPath: `${inst.name}.mehbur.zip`,
      filters: [{ name: "Zip", extensions: ["zip"] }],
    });
    if (!dest) return;
    try {
      await ipc.exportInstance(inst.id, dest);
    } catch (e) {
      useApp.setState({ notice: toErrorPayload(e) });
    }
  };

  const busy = !!task;
  const items: {
    icon: typeof Play;
    label: string;
    run: () => void;
    danger?: boolean;
    disabled?: boolean;
  }[] = [
    { icon: Info, label: t("instances.details"), run: () => openInstance(inst.id) },
    { icon: FolderOpen, label: t("instances.openFolder"), run: () => void openFolder(inst.id) },
    {
      icon: Copy,
      label: t("instances.copy"),
      run: () => setCopyName(t("instances.copyName", { name: inst.name })),
    },
    { icon: Download, label: t("instances.export"), run: () => void exportZip() },
    {
      icon: Trash2,
      label: t("common.delete"),
      run: () => setConfirmDelete(true),
      danger: true,
      disabled: busy,
    },
  ];

  return (
    <div
      ref={setNodeRef}
      style={{
        transform: CSS.Transform.toString(transform),
        transition,
        zIndex: isDragging ? 10 : undefined,
      }}
      className={`group relative flex flex-col gap-3 rounded-lg border bg-surface-1/85 p-4 backdrop-blur transition-colors ${
        selected ? "border-accent/60 neon-ring" : "border-line hover:border-accent/40"
      } ${isDragging ? "opacity-80" : ""}`}
      onClick={() => void select(inst.id)}
      onDoubleClick={() => openInstance(inst.id)}
    >
      <div className="flex items-start gap-3">
        <InstanceIcon icon={inst.icon} size={52} />
        <div className="min-w-0 flex-1">
          <h3 className="truncate font-display text-lg font-bold" title={inst.name}>
            {inst.name}
          </h3>
          <div className="mt-1 flex flex-wrap gap-1.5">
            <Badge tone="accent">{inst.mcVersion}</Badge>
            <Badge>{loaderLabel(inst.loader.kind)}</Badge>
          </div>
        </div>
        <button
          type="button"
          aria-label={t("instances.drag")}
          className="cursor-grab text-muted opacity-0 transition-opacity group-hover:opacity-100 active:cursor-grabbing"
          {...attributes}
          {...listeners}
          onClick={(e) => e.stopPropagation()}
        >
          <GripVertical size={16} />
        </button>
      </div>

      <div className="flex items-center justify-between text-xs text-muted">
        <span>
          {t("instances.playTime")}: {formatDuration(inst.playTimeSecs, units)}
        </span>
        <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
          {task ? (
            <IconButton
              label={t("home.stop")}
              onClick={() => void stop(inst.id)}
              className="text-danger"
            >
              <Square size={15} />
            </IconButton>
          ) : (
            <IconButton label={t("home.play")} onClick={() => void play(inst.id)}>
              <Play size={16} />
            </IconButton>
          )}
          <div className="relative" ref={menuRef}>
            <IconButton label={t("common.more")} onClick={() => setMenu((m) => !m)}>
              <MoreVertical size={16} />
            </IconButton>
            {menu && (
              <div className="absolute right-0 bottom-9 z-20 w-48 overflow-hidden rounded-md border border-line bg-surface-2 py-1 shadow-xl">
                {items.map(({ icon: Icon, label, run, danger, disabled }) => (
                  <button
                    key={label}
                    type="button"
                    disabled={disabled}
                    onClick={() => {
                      setMenu(false);
                      run();
                    }}
                    className={`flex w-full items-center gap-2 px-3 py-2 text-left text-sm disabled:opacity-40 ${
                      danger ? "text-danger hover:bg-danger/10" : "hover:bg-surface-3"
                    }`}
                  >
                    <Icon size={15} />
                    {label}
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>
      {task && (
        <span className="absolute top-2 right-9 h-2 w-2 animate-pulse rounded-full bg-success" />
      )}

      <ConfirmDialog
        open={confirmDelete}
        title={t("instances.deleteTitle")}
        message={t("instances.deleteBody", { name: inst.name })}
        confirmLabel={t("common.delete")}
        danger
        onConfirm={() => void remove(inst.id)}
        onClose={() => setConfirmDelete(false)}
      />
      <Modal
        open={copyName !== null}
        onClose={() => setCopyName(null)}
        title={t("instances.copy")}
        footer={
          <>
            <Button variant="ghost" onClick={() => setCopyName(null)}>
              {t("common.cancel")}
            </Button>
            <Button
              variant="primary"
              disabled={!copyName?.trim()}
              onClick={() => {
                if (copyName) void copy(inst.id, copyName);
                setCopyName(null);
              }}
            >
              {t("instances.copy")}
            </Button>
          </>
        }
      >
        <TextInput
          value={copyName ?? ""}
          maxLength={64}
          onChange={(e) => setCopyName(e.target.value)}
          autoFocus
        />
      </Modal>
    </div>
  );
}
