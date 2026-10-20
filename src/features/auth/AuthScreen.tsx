import {
  KeyRound,
  Loader2,
  LogIn,
  LogOut,
  MailCheck,
  RefreshCw,
  ShieldBan,
  UserPlus,
  WifiOff,
} from "lucide-react";
import { useState, type FormEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { Logo } from "../../components/Logo";
import { Button, Field, Modal, TextInput } from "../../components/ui";
import { ipc, toErrorPayload } from "../../lib/ipc";
import type { AuthStatus } from "../../lib/ipc/bindings/AuthStatus";
import type { CodePurpose } from "../../lib/ipc/bindings/CodePurpose";
import type { ErrorPayload } from "../../lib/ipc/bindings/ErrorPayload";
import { formatIso } from "../../lib/format";
import { useAuth } from "../../stores/auth";

type Mode =
  | { kind: "signIn" }
  | { kind: "signUp" }
  | { kind: "forgot" }
  | { kind: "code"; purpose: CodePurpose };

const MIN_PASSWORD = 8;

/** What a local-only account cannot do (K82), in the order shown. */
const LOCAL_RULES = ["demo", "browser", "library", "share", "skinmc", "admin", "friends"] as const;

function Card({ title, icon, children }: { title: string; icon: ReactNode; children: ReactNode }) {
  return (
    <div className="grid flex-1 place-items-center overflow-y-auto p-6">
      <div className="w-full max-w-md rounded-xl border border-line bg-surface-1/90 p-6 shadow-xl backdrop-blur">
        <div className="mb-5 flex flex-col items-center gap-3 text-center">
          <Logo className="h-14 w-14 rounded-xl" />
          <h1 className="flex items-center gap-2 font-display text-2xl font-bold tracking-wide">
            {icon}
            {title}
          </h1>
        </div>
        {children}
      </div>
    </div>
  );
}

function ErrorLine({ error }: { error: ErrorPayload | null }) {
  const { t } = useTranslation();
  if (!error) return null;
  return (
    <p role="alert" className="text-sm text-danger">
      {t(`errors.${error.code}`, { ...error.params, defaultValue: error.detail })}
    </p>
  );
}

/** Sign in / create an account / reset the password (K73). */
export function AuthScreen({ status }: { status: AuthStatus }) {
  const { t } = useTranslation();
  const load = useAuth((s) => s.load);
  const [mode, setMode] = useState<Mode>(
    status.anonymousIdentity ? { kind: "signUp" } : { kind: "signIn" },
  );
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [password2, setPassword2] = useState("");
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ErrorPayload | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const [localOpen, setLocalOpen] = useState(false);
  const [google, setGoogle] = useState(false);

  const go = (m: Mode) => {
    setMode(m);
    setError(null);
    setInfo(null);
    setCode("");
    if (m.kind !== "code") setPassword2("");
  };

  const run = async (op: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await op();
    } catch (e) {
      setError(toErrorPayload(e));
    } finally {
      setBusy(false);
    }
  };

  const mismatch = password2 !== "" && password !== password2;
  const passwordOk = password.length >= MIN_PASSWORD && password === password2;

  const signIn = (e: FormEvent) => {
    e.preventDefault();
    void run(async () => {
      await ipc.authSignIn(email, password);
      setPassword("");
      await load(true);
    });
  };

  const signUp = (e: FormEvent) => {
    e.preventDefault();
    void run(async () => {
      const r = await ipc.authSignUp(email, password);
      if (r.kind === "signedIn") {
        setPassword("");
        await load(true);
      } else {
        go({ kind: "code", purpose: r.purpose });
      }
    });
  };

  const forgot = (e: FormEvent) => {
    e.preventDefault();
    void run(async () => {
      await ipc.authRequestReset(email);
      setPassword("");
      go({ kind: "code", purpose: "recovery" });
    });
  };

  const verify = (e: FormEvent) => {
    e.preventDefault();
    if (mode.kind !== "code") return;
    void run(async () => {
      await ipc.authVerify(email, code, mode.purpose, password);
      setPassword("");
      setPassword2("");
      await load(true);
    });
  };

  // Google (K83): the browser signs in; cancelling is not an error.
  const googleSignIn = async () => {
    setGoogle(true);
    setError(null);
    try {
      await ipc.authGoogle();
      await load(true);
    } catch (e) {
      const p = toErrorPayload(e);
      if (p.code !== "auth.googleCancelled") setError(p);
    } finally {
      setGoogle(false);
    }
  };

  // Local-only account (K82): offered on the sign-up screen only.
  const createLocal = () => {
    void run(async () => {
      await ipc.authCreateLocal();
      setLocalOpen(false);
      await load(false);
    });
  };

  const resend = () => {
    if (mode.kind !== "code") return;
    void run(async () => {
      await ipc.authResend(email, mode.purpose);
      setInfo(t("auth.code.resent"));
    });
  };

  const emailField = (
    <Field label={t("auth.email")}>
      <TextInput
        type="email"
        autoComplete="email"
        value={email}
        maxLength={254}
        onChange={(e) => setEmail(e.target.value)}
      />
    </Field>
  );
  const passwordFields = (confirm: boolean, label = t("auth.password")) => (
    <>
      <Field label={label}>
        <TextInput
          type="password"
          autoComplete={confirm ? "new-password" : "current-password"}
          value={password}
          maxLength={72}
          onChange={(e) => setPassword(e.target.value)}
        />
      </Field>
      {confirm && (
        <>
          <Field label={t("auth.passwordAgain")}>
            <TextInput
              type="password"
              autoComplete="new-password"
              value={password2}
              maxLength={72}
              onChange={(e) => setPassword2(e.target.value)}
            />
          </Field>
          <p className={`text-xs ${mismatch ? "text-danger" : "text-muted"}`}>
            {mismatch ? t("auth.mismatch") : t("auth.passwordRule", { min: MIN_PASSWORD })}
          </p>
        </>
      )}
    </>
  );
  const link = (label: string, m: Mode) => (
    <button
      type="button"
      onClick={() => go(m)}
      className="text-sm text-accent underline-offset-2 hover:underline"
    >
      {label}
    </button>
  );
  const spinner = busy && <Loader2 size={14} className="animate-spin" />;

  if (mode.kind === "code") {
    const needsPassword = mode.purpose === "recovery";
    return (
      <Card title={t("auth.code.title")} icon={<MailCheck size={22} className="text-accent" />}>
        <form onSubmit={verify} className="flex flex-col gap-4">
          <p className="text-sm text-muted">{t("auth.code.sent", { email })}</p>
          <Field label={t("auth.code.label")}>
            <TextInput
              inputMode="numeric"
              autoComplete="one-time-code"
              value={code}
              maxLength={12}
              onChange={(e) => setCode(e.target.value)}
              className="text-center font-mono text-lg tracking-[0.4em]"
            />
          </Field>
          {needsPassword && passwordFields(true, t("auth.newPassword"))}
          {info && <p className="text-sm text-success">{info}</p>}
          <ErrorLine error={error} />
          <Button
            type="submit"
            variant="primary"
            disabled={busy || code.trim().length < 6 || (needsPassword && !passwordOk)}
          >
            {spinner}
            {t("auth.code.submit")}
          </Button>
          <div className="flex justify-between">
            <button
              type="button"
              disabled={busy}
              onClick={resend}
              className="text-sm text-accent underline-offset-2 hover:underline"
            >
              {t("auth.code.resend")}
            </button>
            {link(
              t("common.back"),
              mode.purpose === "recovery" ? { kind: "forgot" } : { kind: "signUp" },
            )}
          </div>
          <p className="text-xs text-muted">{t("auth.code.spam")}</p>
        </form>
      </Card>
    );
  }

  if (mode.kind === "forgot") {
    return (
      <Card title={t("auth.forgot.title")} icon={<KeyRound size={22} className="text-accent" />}>
        <form onSubmit={forgot} className="flex flex-col gap-4">
          <p className="text-sm text-muted">{t("auth.forgot.hint")}</p>
          {emailField}
          <ErrorLine error={error} />
          <Button type="submit" variant="primary" disabled={busy || email.trim() === ""}>
            {spinner}
            {t("auth.forgot.submit")}
          </Button>
          <div className="flex justify-center">{link(t("auth.toSignIn"), { kind: "signIn" })}</div>
        </form>
      </Card>
    );
  }

  if (mode.kind === "signUp") {
    return (
      <Card title={t("auth.signUp.title")} icon={<UserPlus size={22} className="text-accent" />}>
        <form onSubmit={signUp} className="flex flex-col gap-4">
          <p className="text-sm text-muted">
            {status.anonymousIdentity ? t("auth.signUp.upgrade") : t("auth.signUp.hint")}
          </p>
          {emailField}
          {passwordFields(true)}
          <ErrorLine error={error} />
          <Button
            type="submit"
            variant="primary"
            disabled={busy || email.trim() === "" || !passwordOk}
          >
            {spinner}
            {t("auth.signUp.submit")}
          </Button>
          <div className="flex justify-center">
            {link(t("auth.haveAccount"), { kind: "signIn" })}
          </div>
        </form>
        <div className="mt-5 flex flex-col gap-2 border-t border-line pt-4">
          <Button variant="ghost" disabled={busy} onClick={() => setLocalOpen(true)}>
            <WifiOff size={14} />
            {t("auth.local.button")}
          </Button>
          <p className="text-center text-xs text-muted">{t("auth.local.hint")}</p>
        </div>
        <Modal
          open={localOpen}
          onClose={() => setLocalOpen(false)}
          title={t("auth.local.title")}
          footer={
            <>
              <Button variant="ghost" onClick={() => setLocalOpen(false)}>
                {t("common.cancel")}
              </Button>
              <Button variant="primary" disabled={busy} onClick={createLocal}>
                {spinner}
                {t("auth.local.confirm")}
              </Button>
            </>
          }
        >
          <div className="flex flex-col gap-3 text-sm">
            <p>{t("auth.local.intro")}</p>
            <ul className="flex list-disc flex-col gap-1 pl-5 text-muted">
              {LOCAL_RULES.map((k) => (
                <li key={k}>{t(`auth.local.rules.${k}`)}</li>
              ))}
            </ul>
            <p className="text-xs text-muted">{t("auth.local.later")}</p>
            <ErrorLine error={error} />
          </div>
        </Modal>
      </Card>
    );
  }

  return (
    <Card title={t("auth.signIn.title")} icon={<LogIn size={22} className="text-accent" />}>
      <form onSubmit={signIn} className="flex flex-col gap-4">
        <p className="text-sm text-muted">{t("auth.signIn.hint")}</p>
        {status.anonymousIdentity && (
          <p className="rounded-md border border-warn/40 bg-warn/10 px-3 py-2 text-xs text-warn">
            {t("auth.signIn.anonymousWarning")}
          </p>
        )}
        {emailField}
        {passwordFields(false)}
        <ErrorLine error={error} />
        <Button
          type="submit"
          variant="primary"
          disabled={busy || email.trim() === "" || password === ""}
        >
          {spinner}
          {t("auth.signIn.submit")}
        </Button>
        <div className="flex justify-between">
          {link(t("auth.forgot.link"), { kind: "forgot" })}
          {link(t("auth.createAccount"), { kind: "signUp" })}
        </div>
      </form>
      <div className="mt-5 flex flex-col gap-2 border-t border-line pt-4">
        {google ? (
          <>
            <p role="status" className="flex items-center justify-center gap-2 text-sm text-muted">
              <Loader2 size={14} className="animate-spin text-accent" />
              {t("auth.google.waiting")}
            </p>
            <Button variant="ghost" onClick={() => void ipc.authGoogleCancel()}>
              {t("common.cancel")}
            </Button>
          </>
        ) : (
          <Button disabled={busy} onClick={() => void googleSignIn()}>
            <GoogleIcon />
            {t("auth.google.button")}
          </Button>
        )}
      </div>
    </Card>
  );
}

/** Google's "G" mark for the sign-in button. */
function GoogleIcon() {
  return (
    <svg viewBox="0 0 48 48" width="16" height="16" aria-hidden="true">
      <path
        fill="#EA4335"
        d="M24 9.5c3.54 0 6.71 1.22 9.21 3.6l6.85-6.85C35.9 2.38 30.47 0 24 0 14.62 0 6.51 5.38 2.56 13.22l7.98 6.19C12.43 13.72 17.74 9.5 24 9.5z"
      />
      <path
        fill="#4285F4"
        d="M46.98 24.55c0-1.57-.15-3.09-.38-4.55H24v9.02h12.94c-.58 2.96-2.26 5.48-4.78 7.18l7.73 6c4.51-4.18 7.09-10.36 7.09-17.65z"
      />
      <path
        fill="#FBBC05"
        d="M10.53 28.59c-.48-1.45-.76-2.99-.76-4.59s.27-3.14.76-4.59l-7.98-6.19C.92 16.46 0 20.12 0 24c0 3.88.92 7.54 2.56 10.78l7.97-6.19z"
      />
      <path
        fill="#34A853"
        d="M24 48c6.48 0 11.93-2.13 15.89-5.81l-7.73-6c-2.15 1.45-4.92 2.3-8.16 2.3-6.26 0-11.57-4.22-13.47-9.91l-7.98 6.19C6.51 42.62 14.62 48 24 48z"
      />
    </svg>
  );
}

/** Shown instead of the launcher while the account is banned. */
export function BanScreen({ status }: { status: AuthStatus }) {
  const { t, i18n } = useTranslation();
  const load = useAuth((s) => s.load);
  const signOut = useAuth((s) => s.signOut);
  const [busy, setBusy] = useState(false);
  const ban = status.ban;
  return (
    <Card title={t("auth.ban.title")} icon={<ShieldBan size={22} className="text-danger" />}>
      <div className="flex flex-col gap-3 text-sm">
        <p>
          {ban?.until
            ? t("auth.ban.until", { date: formatIso(ban.until, i18n.language) })
            : t("auth.ban.permanent")}
        </p>
        {ban?.reason && (
          <p className="text-muted">{t("auth.ban.reason", { reason: ban.reason })}</p>
        )}
        <p className="text-xs text-muted">{status.email}</p>
        <div className="mt-2 flex justify-between gap-2">
          <Button
            variant="ghost"
            disabled={busy}
            onClick={() => {
              setBusy(true);
              void load(true).finally(() => setBusy(false));
            }}
          >
            <RefreshCw size={14} className={busy ? "animate-spin" : ""} />
            {t("auth.ban.recheck")}
          </Button>
          <Button variant="danger" onClick={() => void signOut()}>
            <LogOut size={14} />
            {t("auth.signOut")}
          </Button>
        </div>
      </div>
    </Card>
  );
}
