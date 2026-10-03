import { Copy, ExternalLink, Loader2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button, Modal } from "../../components/ui";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { ErrorPayload } from "../../lib/ipc/bindings/ErrorPayload";
import type { LoginPrompt } from "../../lib/ipc/bindings/LoginPrompt";
import { useAccounts } from "../../stores/accounts";

type Phase = "starting" | "waiting" | "done" | "failed";

/** Device code sign-in: shows the code, opens the browser, waits. */
export function MicrosoftLogin({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { t } = useTranslation();
  const reload = useAccounts((s) => s.load);
  const [phase, setPhase] = useState<Phase>("starting");
  const [prompt, setPrompt] = useState<LoginPrompt | null>(null);
  const [error, setError] = useState<ErrorPayload | null>(null);
  const [name, setName] = useState("");
  const [copied, setCopied] = useState(false);
  const current = useRef<string | null>(null);

  useEffect(() => {
    if (!open) return;
    let alive = true;
    setPhase("starting");
    setPrompt(null);
    setError(null);
    (async () => {
      try {
        const p = await ipc.beginMicrosoftLogin();
        if (!alive) {
          void ipc.cancelMicrosoftLogin(p.loginId);
          return;
        }
        current.current = p.loginId;
        setPrompt(p);
        setPhase("waiting");
        void ipc.openMicrosoftLogin(p.loginId).catch(() => {});
        const account = await ipc.finishMicrosoftLogin(p.loginId);
        current.current = null;
        if (!alive) return;
        setName(account.name);
        setPhase("done");
        await reload();
      } catch (e) {
        current.current = null;
        if (!alive) return;
        const err = toErrorPayload(e);
        if (err.code === "task.cancelled") return;
        setError(err);
        setPhase("failed");
      }
    })();
    return () => {
      alive = false;
    };
  }, [open, reload]);

  const close = () => {
    if (current.current) void ipc.cancelMicrosoftLogin(current.current);
    current.current = null;
    onClose();
  };

  const copy = async () => {
    if (!prompt) return;
    try {
      await navigator.clipboard.writeText(prompt.userCode);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard may be unavailable; the code is visible anyway.
    }
  };

  return (
    <Modal
      open={open}
      onClose={close}
      title={t("msa.title")}
      footer={
        <Button variant={phase === "done" ? "primary" : "ghost"} onClick={close}>
          {phase === "done" ? t("common.close") : t("common.cancel")}
        </Button>
      }
    >
      <div className="flex min-h-48 flex-col items-center justify-center gap-4 text-center">
        {phase === "starting" && (
          <Loader2
            size={28}
            className="animate-spin text-accent"
            aria-label={t("common.loading")}
          />
        )}

        {phase === "waiting" && prompt && (
          <>
            <p className="text-sm text-muted">{t("msa.step1")}</p>
            <div
              data-selectable
              className="rounded-lg border border-accent/50 bg-accent/10 px-6 py-3 font-mono text-3xl font-bold tracking-[0.3em] text-accent neon-ring"
            >
              {prompt.userCode}
            </div>
            <div className="flex gap-2">
              <Button size="sm" onClick={() => void copy()}>
                <Copy size={13} />
                {copied ? t("msa.copied") : t("msa.copy")}
              </Button>
              <Button
                size="sm"
                variant="primary"
                onClick={() => void ipc.openMicrosoftLogin(prompt.loginId)}
              >
                <ExternalLink size={13} />
                {t("msa.openBrowser")}
              </Button>
            </div>
            <p className="text-xs text-muted">{t("msa.step2", { url: prompt.verificationUri })}</p>
            <p className="flex items-center gap-2 text-xs text-muted">
              <Loader2 size={12} className="animate-spin" />
              {t("msa.waiting")}
            </p>
          </>
        )}

        {phase === "done" && (
          <p className="text-base font-semibold text-success">{t("msa.done", { name })}</p>
        )}

        {phase === "failed" && error && (
          <p className="text-sm text-danger">
            {t(`errors.${error.code}`, { ...error.params, defaultValue: error.detail })}
          </p>
        )}
      </div>
    </Modal>
  );
}
