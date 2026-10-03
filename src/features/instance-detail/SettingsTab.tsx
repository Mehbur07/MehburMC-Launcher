import { Save } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { INSTANCE_ICONS, InstanceIcon } from "../../components/InstanceIcon";
import { Button, Field, TextInput } from "../../components/ui";
import type { Instance } from "../../lib/ipc/bindings/Instance";
import { useApp } from "../../stores/app";
import { useInstances } from "../../stores/instances";
import { InstanceOptionsFields, type InstanceOptions } from "../instances/InstanceOptionsFields";

export function SettingsTab({ inst }: { inst: Instance }) {
  const { t } = useTranslation();
  const update = useInstances((s) => s.update);
  const defaultMemory = useApp((s) => s.settings?.defaultMemoryMb ?? 4096);
  const [name, setName] = useState(inst.name);
  const [icon, setIcon] = useState(inst.icon);
  const [options, setOptions] = useState<InstanceOptions>({
    javaPath: inst.javaPath,
    memoryMb: inst.memoryMb,
    jvmArgs: inst.jvmArgs,
    resolution: inst.resolution,
    fullscreen: inst.fullscreen,
  });
  const [saved, setSaved] = useState(false);

  const save = async () => {
    const res = await update(inst.id, {
      name,
      icon,
      javaPath: options.javaPath ?? undefined,
      clearJavaPath: options.javaPath === null,
      memoryMb: options.memoryMb ?? undefined,
      clearMemory: options.memoryMb === null,
      jvmArgs: options.jvmArgs,
      resolution: options.resolution ?? undefined,
      clearResolution: options.resolution === null,
      fullscreen: options.fullscreen,
    });
    if (res) {
      setSaved(true);
      setTimeout(() => setSaved(false), 1500);
    }
  };

  return (
    <div className="flex max-w-2xl flex-col gap-5">
      <Field label={t("wizard.name")}>
        <TextInput value={name} maxLength={64} onChange={(e) => setName(e.target.value)} />
      </Field>
      <Field label={t("wizard.icon")}>
        <div className="flex flex-wrap gap-2">
          {Object.keys(INSTANCE_ICONS).map((k) => (
            <button
              key={k}
              type="button"
              aria-pressed={icon === k}
              aria-label={k}
              onClick={() => setIcon(k)}
              className={`rounded-lg ${icon === k ? "neon-ring" : "opacity-70 hover:opacity-100"}`}
            >
              <InstanceIcon icon={k} size={38} />
            </button>
          ))}
        </div>
      </Field>
      <InstanceOptionsFields
        value={options}
        onChange={setOptions}
        defaultMemoryMb={defaultMemory}
      />
      <div>
        <Button variant="primary" disabled={!name.trim()} onClick={() => void save()}>
          <Save size={15} />
          {saved ? t("common.saved") : t("common.save")}
        </Button>
      </div>
    </div>
  );
}
