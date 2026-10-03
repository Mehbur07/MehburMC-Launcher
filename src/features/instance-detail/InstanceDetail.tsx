import { ArrowLeft } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { InstanceIcon } from "../../components/InstanceIcon";
import { Badge, EmptyState } from "../../components/ui";
import { useApp } from "../../stores/app";
import { useInstances } from "../../stores/instances";
import { loaderLabel } from "../instances/InstancesPage";
import { ContentTab } from "./ContentTab";
import { FilesTab } from "./FilesTab";
import { GeneralTab } from "./GeneralTab";
import { LogsTab } from "./LogsTab";
import { ScreenshotsTab } from "./ScreenshotsTab";
import { SettingsTab } from "./SettingsTab";

const TABS = [
  "general",
  "mods",
  "resourcePacks",
  "shaderPacks",
  "saves",
  "screenshots",
  "logs",
  "settings",
] as const;
type Tab = (typeof TABS)[number];

export function InstanceDetail() {
  const { t } = useTranslation();
  const id = useApp((s) => s.detailId);
  const setView = useApp((s) => s.setView);
  const inst = useInstances((s) => s.instances.find((i) => i.id === id) ?? null);
  const initialTab = useApp((s) => s.detailTab);
  const [tab, setTab] = useState<Tab>(() =>
    TABS.includes(initialTab as Tab) ? (initialTab as Tab) : "general",
  );

  if (!inst) {
    return <EmptyState icon={<ArrowLeft size={32} />} title={t("instance.missing")} />;
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <div className="flex items-center gap-4">
        <button
          type="button"
          onClick={() => setView("instances")}
          aria-label={t("common.back")}
          className="grid h-9 w-9 place-items-center rounded-md text-muted hover:bg-surface-3 hover:text-accent"
        >
          <ArrowLeft size={18} />
        </button>
        <InstanceIcon icon={inst.icon} size={48} />
        <div className="min-w-0">
          <h1 className="truncate font-display text-2xl font-bold tracking-wide">{inst.name}</h1>
          <div className="mt-1 flex gap-1.5">
            <Badge tone="accent">{inst.mcVersion}</Badge>
            <Badge>{loaderLabel(inst.loader.kind)}</Badge>
          </div>
        </div>
      </div>

      <div role="tablist" className="flex gap-1 overflow-x-auto border-b border-line">
        {TABS.map((k) => (
          <button
            key={k}
            type="button"
            role="tab"
            aria-selected={tab === k}
            onClick={() => setTab(k)}
            className={`-mb-px border-b-2 px-3 py-2 text-sm font-medium whitespace-nowrap transition-colors ${
              tab === k
                ? "border-accent text-accent"
                : "border-transparent text-muted hover:text-fg"
            }`}
          >
            {t(`instance.tabs.${k}`)}
          </button>
        ))}
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto pb-4">
        {tab === "general" && <GeneralTab inst={inst} />}
        {tab === "mods" && <ContentTab inst={inst} folder="mods" />}
        {tab === "resourcePacks" && <ContentTab inst={inst} folder="resourcePacks" />}
        {tab === "shaderPacks" && <ContentTab inst={inst} folder="shaderPacks" />}
        {tab === "saves" && <FilesTab inst={inst} folder="saves" />}
        {tab === "screenshots" && <ScreenshotsTab inst={inst} />}
        {tab === "logs" && <LogsTab inst={inst} />}
        {tab === "settings" && <SettingsTab inst={inst} />}
      </div>
    </div>
  );
}
