import { FolderSearch } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button, Field, TextInput, Toggle } from "../../components/ui";
import { ipc } from "../../lib/ipc";
import type { JavaInstall } from "../../lib/ipc/bindings/JavaInstall";
import type { Resolution } from "../../lib/ipc/bindings/Resolution";

export interface InstanceOptions {
  javaPath: string | null;
  memoryMb: number | null;
  jvmArgs: string;
  resolution: Resolution | null;
  fullscreen: boolean;
}

const MEM_MIN = 1024;
const MEM_MAX = 16384;

let javaCache: Promise<JavaInstall[]> | null = null;
const loadJava = () => (javaCache ??= ipc.listJava().catch(() => []));

export function InstanceOptionsFields({
  value,
  onChange,
  defaultMemoryMb,
}: {
  value: InstanceOptions;
  onChange: (v: InstanceOptions) => void;
  defaultMemoryMb: number;
}) {
  const { t } = useTranslation();
  const [javas, setJavas] = useState<JavaInstall[]>([]);
  useEffect(() => {
    void loadJava().then(setJavas);
  }, []);
  const set = (patch: Partial<InstanceOptions>) => onChange({ ...value, ...patch });

  const browse = async () => {
    const picked = await ipc.pickPath("java").catch(() => null);
    if (picked) set({ javaPath: picked });
  };

  const mem = value.memoryMb ?? defaultMemoryMb;

  return (
    <div className="flex flex-col gap-5">
      <Field label={t("options.java")} hint={t("options.javaHint")}>
        <div className="flex gap-2">
          <select
            value={value.javaPath ?? ""}
            onChange={(e) => set({ javaPath: e.target.value || null })}
            className="min-w-0 flex-1 rounded-md border border-line bg-surface-2 px-3 py-2 text-sm focus:border-accent"
          >
            <option value="">{t("options.javaAuto")}</option>
            {javas.map((j) => (
              <option key={j.executable} value={j.executable}>
                Java {j.major} · {j.version} ·{" "}
                {j.managed ? t("options.javaManaged") : (j.vendor ?? t("options.javaSystem"))}
              </option>
            ))}
            {value.javaPath && !javas.some((j) => j.executable === value.javaPath) && (
              <option value={value.javaPath}>{value.javaPath}</option>
            )}
          </select>
          <Button onClick={() => void browse()}>
            <FolderSearch size={15} />
            {t("options.browse")}
          </Button>
        </div>
      </Field>

      <Field label={t("options.memory")}>
        <div className="flex items-center gap-3">
          <input
            type="range"
            min={MEM_MIN}
            max={MEM_MAX}
            step={512}
            value={mem}
            disabled={value.memoryMb === null}
            onChange={(e) => set({ memoryMb: Number(e.target.value) })}
            className="flex-1 accent-[var(--mc-accent)] disabled:opacity-40"
            aria-label={t("options.memory")}
          />
          <span className="w-16 text-right text-sm font-semibold tabular-nums">
            {(mem / 1024).toFixed(1)} GB
          </span>
        </div>
        <label className="mt-1 flex items-center gap-2 text-xs text-muted">
          <input
            type="checkbox"
            checked={value.memoryMb === null}
            onChange={(e) => set({ memoryMb: e.target.checked ? null : defaultMemoryMb })}
            className="accent-[var(--mc-accent)]"
          />
          {t("options.memoryDefault", { gb: (defaultMemoryMb / 1024).toFixed(1) })}
        </label>
      </Field>

      <Field label={t("options.jvmArgs")} hint={t("options.jvmArgsHint")}>
        <TextInput
          value={value.jvmArgs}
          onChange={(e) => set({ jvmArgs: e.target.value })}
          placeholder="-XX:+UseG1GC"
          spellCheck={false}
          className="font-mono text-xs"
        />
      </Field>

      <div className="grid grid-cols-[1fr_auto] items-end gap-4">
        <Field label={t("options.resolution")}>
          <div className="flex items-center gap-2">
            <Toggle
              checked={value.resolution !== null}
              onChange={(on) => set({ resolution: on ? { width: 1280, height: 720 } : null })}
              label={t("options.resolution")}
            />
            {value.resolution && (
              <>
                <TextInput
                  type="number"
                  min={320}
                  max={7680}
                  value={value.resolution.width}
                  onChange={(e) =>
                    set({
                      resolution: { ...value.resolution!, width: Number(e.target.value) || 0 },
                    })
                  }
                  className="w-24"
                  aria-label={t("options.width")}
                />
                <span className="text-muted">×</span>
                <TextInput
                  type="number"
                  min={240}
                  max={4320}
                  value={value.resolution.height}
                  onChange={(e) =>
                    set({
                      resolution: { ...value.resolution!, height: Number(e.target.value) || 0 },
                    })
                  }
                  className="w-24"
                  aria-label={t("options.height")}
                />
              </>
            )}
          </div>
        </Field>
        <Field label={t("options.fullscreen")}>
          <Toggle
            checked={value.fullscreen}
            onChange={(v) => set({ fullscreen: v })}
            label={t("options.fullscreen")}
          />
        </Field>
      </div>
    </div>
  );
}
