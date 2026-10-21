import { Mic, Square } from "lucide-react";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { Button, Toggle } from "../../components/ui";
import { watchLevel } from "../../lib/voice/audio";
import { useApp } from "../../stores/app";

function Row({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-6 py-3">
      <div>
        <div className="text-sm font-medium">{label}</div>
        {hint && <div className="mt-0.5 text-xs text-muted">{hint}</div>}
      </div>
      {children}
    </div>
  );
}

async function microphones(): Promise<MediaDeviceInfo[]> {
  try {
    const all = (await navigator.mediaDevices?.enumerateDevices?.()) ?? [];
    return all.filter((d) => d.kind === "audioinput" && d.deviceId !== "default");
  } catch {
    return [];
  }
}

/** Settings → Voice chat: microphone, mic test, "hide my IP" (K86). */
export function VoiceSection() {
  const { t } = useTranslation();
  const settings = useApp((s) => s.settings);
  const update = useApp((s) => s.updateSettings);
  const [devices, setDevices] = useState<MediaDeviceInfo[]>([]);
  const [level, setLevel] = useState<number | null>(null);
  const [testError, setTestError] = useState<string | null>(null);
  const stopTest = useRef<(() => void) | null>(null);

  const refresh = useCallback(() => void microphones().then(setDevices), []);
  useEffect(() => {
    refresh();
    const md = navigator.mediaDevices;
    md?.addEventListener?.("devicechange", refresh);
    return () => {
      md?.removeEventListener?.("devicechange", refresh);
      stopTest.current?.();
    };
  }, [refresh]);

  if (!settings) return null;
  const device = settings.voiceInputDevice ?? "";

  const toggleTest = async () => {
    if (stopTest.current) {
      stopTest.current();
      stopTest.current = null;
      setLevel(null);
      return;
    }
    setTestError(null);
    try {
      const stream = await navigator.mediaDevices.getUserMedia({
        audio: device ? { deviceId: { ideal: device } } : true,
      });
      const unwatch = watchLevel(stream, setLevel);
      stopTest.current = () => {
        unwatch();
        stream.getTracks().forEach((tr) => tr.stop());
      };
      setLevel(0);
      // Device names are only visible once the microphone was allowed.
      refresh();
    } catch (e) {
      const name = e instanceof DOMException ? e.name : "";
      setTestError(name === "NotAllowedError" ? "calls.micDenied" : "calls.micMissing");
    }
  };

  return (
    <>
      <Row label={t("settings.voice.mic")} hint={t("settings.voice.micHint")}>
        <select
          value={devices.some((d) => d.deviceId === device) ? device : ""}
          onChange={(e) => {
            stopTest.current?.();
            stopTest.current = null;
            setLevel(null);
            void update({ voiceInputDevice: e.target.value || undefined });
          }}
          aria-label={t("settings.voice.mic")}
          className="max-w-60 rounded-md border border-line bg-surface-2 px-3 py-2 text-sm focus:border-accent"
        >
          <option value="">{t("settings.voice.micDefault")}</option>
          {devices.map((d, i) => (
            <option key={d.deviceId} value={d.deviceId}>
              {d.label || t("settings.voice.micUnnamed", { n: i + 1 })}
            </option>
          ))}
        </select>
      </Row>
      <Row label={t("settings.voice.test")} hint={t("settings.voice.testHint")}>
        <div className="flex items-center gap-3">
          {level !== null && (
            <div
              role="meter"
              aria-label={t("settings.voice.level")}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={Math.round(level * 100)}
              className="h-2 w-32 overflow-hidden rounded-full bg-surface-3"
            >
              <div
                className="h-full rounded-full bg-online transition-[width] duration-100"
                style={{ width: `${Math.round(level * 100)}%` }}
              />
            </div>
          )}
          <Button size="sm" onClick={() => void toggleTest()}>
            {level !== null ? <Square size={13} /> : <Mic size={13} />}
            {level !== null ? t("settings.voice.stopTest") : t("settings.voice.startTest")}
          </Button>
        </div>
      </Row>
      {testError && <p className="pb-3 text-xs text-warn">{t(`errors.${testError}`)}</p>}
      <Row label={t("settings.voice.hideIp")} hint={t("settings.voice.hideIpHint")}>
        <Toggle
          checked={settings.voiceHideIp}
          onChange={(v) => void update({ voiceHideIp: v })}
          label={t("settings.voice.hideIp")}
        />
      </Row>
    </>
  );
}
