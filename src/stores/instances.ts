import { create } from "zustand";

import { ipc, toErrorPayload } from "../lib/ipc";
import type { Instance } from "../lib/ipc/bindings/Instance";
import type { InstancePatch } from "../lib/ipc/bindings/InstancePatch";
import type { ManifestEntry } from "../lib/ipc/bindings/ManifestEntry";
import type { NewInstance } from "../lib/ipc/bindings/NewInstance";
import { useApp } from "./app";

interface InstancesStore {
  loaded: boolean;
  instances: Instance[];
  selected: string | null;
  versions: ManifestEntry[] | null;

  load: () => Promise<void>;
  loadVersions: () => Promise<ManifestEntry[]>;
  select: (id: string) => Promise<void>;
  create: (req: NewInstance) => Promise<Instance | null>;
  update: (id: string, patch: InstancePatch) => Promise<Instance | null>;
  remove: (id: string) => Promise<boolean>;
  copy: (id: string, name: string) => Promise<Instance | null>;
  reorder: (ids: string[]) => Promise<void>;
  /** Replaces one instance in the list (e.g. after play time changed). */
  patchLocal: (inst: Instance) => void;
}

const notify = (e: unknown) => useApp.setState({ notice: toErrorPayload(e) });

export const useInstances = create<InstancesStore>((set, get) => ({
  loaded: false,
  instances: [],
  selected: null,
  versions: null,

  load: async () => {
    try {
      const v = await ipc.listInstances();
      set({ instances: v.instances, selected: v.selected, loaded: true });
    } catch (e) {
      set({ loaded: true });
      notify(e);
    }
  },

  loadVersions: async () => {
    const cached = get().versions;
    if (cached) return cached;
    const versions = await ipc.listVersions();
    set({ versions });
    return versions;
  },

  select: async (id) => {
    set({ selected: id });
    try {
      await ipc.selectInstance(id);
    } catch (e) {
      notify(e);
    }
  },

  create: async (req) => {
    try {
      const inst = await ipc.createInstance(req);
      await get().load();
      set({ selected: inst.id });
      return inst;
    } catch (e) {
      notify(e);
      return null;
    }
  },

  update: async (id, patch) => {
    try {
      const inst = await ipc.updateInstance(id, patch);
      get().patchLocal(inst);
      return inst;
    } catch (e) {
      notify(e);
      return null;
    }
  },

  remove: async (id) => {
    try {
      const v = await ipc.deleteInstance(id);
      set({ instances: v.instances, selected: v.selected });
      return true;
    } catch (e) {
      notify(e);
      return false;
    }
  },

  copy: async (id, name) => {
    try {
      const inst = await ipc.copyInstance(id, name);
      await get().load();
      return inst;
    } catch (e) {
      notify(e);
      return null;
    }
  },

  // Optimistic reorder; reload on failure.
  reorder: async (ids) => {
    const byId = new Map(get().instances.map((i) => [i.id, i]));
    set({ instances: ids.map((id) => byId.get(id)).filter((i): i is Instance => !!i) });
    try {
      await ipc.reorderInstances(ids);
    } catch (e) {
      notify(e);
      await get().load();
    }
  },

  patchLocal: (inst) =>
    set((s) => ({ instances: s.instances.map((i) => (i.id === inst.id ? inst : i)) })),
}));

export const selectedInstance = (s: InstancesStore) =>
  s.instances.find((i) => i.id === s.selected) ?? null;
