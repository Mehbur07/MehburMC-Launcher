import { AlertTriangle, Check, ClipboardCopy, X } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import type { ErrorPayload } from "../lib/ipc/bindings/ErrorPayload";

/** Translated error message plus a "Copy details" button. */
export function ErrorNotice({ error, onDismiss }: { error: ErrorPayload; onDismiss?: () => void }) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);

  const message = t(`errors.${error.code}`, {
    ...error.params,
    defaultValue: t("errors.unknown"),
  });

  const copy = async () => {
    const text = `[${error.code}] ${message}\n\n${error.detail}`;
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard can be unavailable; the details stay visible below anyway.
    }
  };

  return (
    <div role="alert" className="rounded-md border border-danger/50 bg-danger/10 p-4">
      <div className="flex items-start gap-3">
        <AlertTriangle className="mt-0.5 shrink-0 text-danger" size={18} />
        <div className="min-w-0 flex-1">
          <p className="font-medium">{message}</p>
          <pre
            data-selectable
            className="mt-2 max-h-40 overflow-auto rounded-sm bg-bg/60 p-2 text-xs whitespace-pre-wrap text-muted"
          >
            {error.detail}
          </pre>
          <button
            type="button"
            onClick={() => void copy()}
            className="mt-3 inline-flex items-center gap-1.5 rounded-sm border border-line px-2.5 py-1 text-xs text-muted hover:border-accent hover:text-accent"
          >
            {copied ? <Check size={14} /> : <ClipboardCopy size={14} />}
            {copied ? t("common.copied") : t("common.copyDetails")}
          </button>
        </div>
        {onDismiss && (
          <button
            type="button"
            onClick={onDismiss}
            aria-label={t("common.dismiss")}
            className="text-muted hover:text-fg"
          >
            <X size={16} />
          </button>
        )}
      </div>
    </div>
  );
}
