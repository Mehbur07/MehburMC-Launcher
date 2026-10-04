import { Plus } from "lucide-react";
import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";

import { INSTANCE_ICONS, InstanceIcon } from "../../components/InstanceIcon";
import { Button, Field, Modal, TextInput } from "../../components/ui";
import type { LoaderSpec } from "../../lib/ipc/bindings/LoaderSpec";
import { useApp } from "../../stores/app";
import { useInstances } from "../../stores/instances";
import { InstanceOptionsFields, type InstanceOptions } from "./InstanceOptionsFields";
import { loaderLabel } from "./loaderLabels";
import { LoaderPicker } from "./LoaderPicker";
import { VersionPicker } from "./VersionPicker";

const EMPTY: InstanceOptions = {
  javaPath: null,
  memoryMb: null,
  jvmArgs: "",
  resolution: null,
  fullscreen: false,
};

export function CreateWizard() {
  const { t } = useTranslation();
  const open = useApp((s) => s.wizardOpen);
  const setWizard = useApp((s) => s.setWizard);
  const defaultMemory = useApp((s) => s.settings?.defaultMemoryMb ?? 4096);
  const create = useInstances((s) => s.create);

  const [name, setName] = useState("");
  const [icon, setIcon] = useState("grass");
  const [version, setVersion] = useState<string | null>(null);
  const [loader, setLoader] = useState<LoaderSpec>({ kind: "vanilla" });
  const [loaderReady, setLoaderReady] = useState(true);
  const onLoaderReady = useCallback((ok: boolean) => setLoaderReady(ok), []);
  const [options, setOptions] = useState<InstanceOptions>(EMPTY);
  const [busy, setBusy] = useState(false);

  const close = () => {
    setWizard(false);
    setName("");
    setIcon("grass");
    setLoader({ kind: "vanilla" });
    setOptions(EMPTY);
  };

  // A loader version belongs to one Minecraft version; reset it on change.
  const pickVersion = (v: string) => {
    setVersion(v);
    setLoader((l) => (l.kind === "optifine" || v === version ? l : { kind: l.kind }));
  };

  const suffix = loader.kind === "vanilla" ? "" : ` (${loaderLabel(loader.kind)})`;
  const finalName = name.trim() || (version ? `Minecraft ${version}${suffix}` : "");
  const valid = !!version && loaderReady && finalName.length > 0 && finalName.length <= 64;

  const submit = async () => {
    if (!valid || !version) return;
    setBusy(true);
    const inst = await create({
      name: finalName,
      icon,
      mcVersion: version,
      loader,
      javaPath: options.javaPath ?? undefined,
      memoryMb: options.memoryMb ?? undefined,
      jvmArgs: options.jvmArgs || undefined,
      resolution: options.resolution ?? undefined,
      fullscreen: options.fullscreen,
    });
    setBusy(false);
    if (inst) close();
  };

  return (
    <Modal
      open={open}
      onClose={close}
      title={t("wizard.title")}
      wide
      footer={
        <>
          <Button variant="ghost" onClick={close}>
            {t("common.cancel")}
          </Button>
          <Button variant="primary" disabled={!valid || busy} onClick={() => void submit()}>
            <Plus size={16} />
            {t("wizard.create")}
          </Button>
        </>
      }
    >
      <div
        className="grid grid-cols-[minmax(0,1fr)_320px] gap-6"
        style={{ height: "min(62vh, 560px)" }}
      >
        <div className="flex min-h-0 flex-col gap-5 overflow-y-auto pr-2">
          <div className="flex items-end gap-4">
            <InstanceIcon icon={icon} size={64} />
            <div className="flex-1">
              <Field label={t("wizard.name")}>
                <TextInput
                  value={name}
                  maxLength={64}
                  placeholder={version ? `Minecraft ${version}${suffix}` : ""}
                  onChange={(e) => setName(e.target.value)}
                  autoFocus
                />
              </Field>
            </div>
          </div>

          <Field label={t("wizard.icon")}>
            <div className="flex flex-wrap gap-2" role="radiogroup" aria-label={t("wizard.icon")}>
              {Object.keys(INSTANCE_ICONS).map((k) => (
                <button
                  key={k}
                  type="button"
                  role="radio"
                  aria-checked={icon === k}
                  aria-label={k}
                  onClick={() => setIcon(k)}
                  className={`rounded-lg transition-transform hover:scale-105 ${icon === k ? "neon-ring" : "opacity-70"}`}
                >
                  <InstanceIcon icon={k} size={40} />
                </button>
              ))}
            </div>
          </Field>

          <div className="flex flex-col gap-1.5">
            <span className="text-xs font-semibold tracking-wide text-muted uppercase">
              {t("wizard.loader")}
            </span>
            <LoaderPicker
              mc={version}
              value={loader}
              onChange={setLoader}
              onReady={onLoaderReady}
              onMcVersion={setVersion}
            />
          </div>

          <InstanceOptionsFields
            value={options}
            onChange={setOptions}
            defaultMemoryMb={defaultMemory}
          />
        </div>

        <div className="flex min-h-0 flex-col">
          <span className="mb-1.5 text-xs font-semibold tracking-wide text-muted uppercase">
            {t("wizard.version")}
          </span>
          <VersionPicker value={version} onChange={pickVersion} />
        </div>
      </div>
    </Modal>
  );
}
