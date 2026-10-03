import { Plus } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { INSTANCE_ICONS, InstanceIcon } from "../../components/InstanceIcon";
import { Badge, Button, Field, Modal, TextInput } from "../../components/ui";
import type { LoaderKind } from "../../lib/ipc/bindings/LoaderKind";
import { useApp } from "../../stores/app";
import { useInstances } from "../../stores/instances";
import { InstanceOptionsFields, type InstanceOptions } from "./InstanceOptionsFields";
import { VersionPicker } from "./VersionPicker";

/** Only vanilla is installable until phase 4. */
const LOADERS: { kind: LoaderKind; label: string; ready: boolean }[] = [
  { kind: "vanilla", label: "Vanilla", ready: true },
  { kind: "fabric", label: "Fabric", ready: false },
  { kind: "quilt", label: "Quilt", ready: false },
  { kind: "forge", label: "Forge", ready: false },
  { kind: "neoForge", label: "NeoForge", ready: false },
  { kind: "legacyFabric", label: "Legacy Fabric", ready: false },
  { kind: "optifine", label: "OptiFine", ready: false },
];

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
  const [loader, setLoader] = useState<LoaderKind>("vanilla");
  const [options, setOptions] = useState<InstanceOptions>(EMPTY);
  const [busy, setBusy] = useState(false);

  const close = () => {
    setWizard(false);
    setName("");
    setIcon("grass");
    setOptions(EMPTY);
  };

  const finalName = name.trim() || (version ? `Minecraft ${version}` : "");
  const valid = !!version && finalName.length > 0 && finalName.length <= 64;

  const submit = async () => {
    if (!valid || !version) return;
    setBusy(true);
    const inst = await create({
      name: finalName,
      icon,
      mcVersion: version,
      loader: { kind: loader },
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
                  placeholder={version ? `Minecraft ${version}` : ""}
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

          <Field label={t("wizard.loader")}>
            <div
              className="grid grid-cols-4 gap-2"
              role="radiogroup"
              aria-label={t("wizard.loader")}
            >
              {LOADERS.map((l) => (
                <button
                  key={l.kind}
                  type="button"
                  role="radio"
                  aria-checked={loader === l.kind}
                  disabled={!l.ready}
                  onClick={() => setLoader(l.kind)}
                  title={l.ready ? l.label : t("wizard.loaderSoon")}
                  className={`flex flex-col items-start gap-1 rounded-md border px-3 py-2 text-left text-sm transition-colors disabled:cursor-not-allowed disabled:opacity-45 ${
                    loader === l.kind
                      ? "border-accent bg-accent/10 text-accent"
                      : "border-line hover:border-accent/40"
                  }`}
                >
                  <span className="font-semibold">{l.label}</span>
                  {!l.ready && <Badge>{t("wizard.loaderSoon")}</Badge>}
                </button>
              ))}
            </div>
          </Field>

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
          <VersionPicker value={version} onChange={setVersion} />
        </div>
      </div>
    </Modal>
  );
}
