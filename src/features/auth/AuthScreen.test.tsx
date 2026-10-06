import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  authStatus: vi.fn(),
  authSignIn: vi.fn(),
  authSignUp: vi.fn(),
  authVerify: vi.fn(),
  authResend: vi.fn(),
  authRequestReset: vi.fn(),
  authSignOut: vi.fn(),
}));
vi.mock("../../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../../lib/ipc")>()),
  ipc: ipcMock,
}));

import "../../i18n";
import { applyLanguage } from "../../i18n";
import type { AuthStatus } from "../../lib/ipc/bindings/AuthStatus";
import { useAuth } from "../../stores/auth";
import { AuthScreen, BanScreen } from "./AuthScreen";

const status = (over: Partial<AuthStatus> = {}): AuthStatus => ({
  signedIn: false,
  email: null,
  anonymousIdentity: false,
  rank: 0,
  ban: null,
  offline: false,
  ...over,
});

const type = (label: string, value: string) =>
  fireEvent.change(screen.getByLabelText(label), { target: { value } });

describe("AuthScreen", () => {
  beforeEach(() => {
    applyLanguage("en");
    vi.clearAllMocks();
    ipcMock.authStatus.mockResolvedValue(status({ signedIn: true, email: "a@b.co" }));
    useAuth.setState({ status: null });
  });

  it("signs in and reloads the account state", async () => {
    ipcMock.authSignIn.mockResolvedValue(undefined);
    render(<AuthScreen status={status()} />);
    const submit = screen.getByRole("button", { name: "Sign in" });
    expect(submit).toBeDisabled();
    type("Email", "a@b.co");
    type("Password", "secretpass");
    await act(async () => fireEvent.click(submit));
    expect(ipcMock.authSignIn).toHaveBeenCalledWith("a@b.co", "secretpass");
    expect(ipcMock.authStatus).toHaveBeenCalledWith(true);
    expect(useAuth.getState().status?.signedIn).toBe(true);
  });

  it("shows server errors", async () => {
    ipcMock.authSignIn.mockRejectedValue({
      code: "auth.invalidCredentials",
      params: {},
      detail: "",
    });
    render(<AuthScreen status={status()} />);
    type("Email", "a@b.co");
    type("Password", "wrong");
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Sign in" })));
    expect(screen.getByRole("alert")).toHaveTextContent("Wrong email or password");
  });

  it("upgrades an anonymous identity with an emailed code", async () => {
    ipcMock.authSignUp.mockResolvedValue({ kind: "codeSent", purpose: "upgrade" });
    ipcMock.authVerify.mockResolvedValue(undefined);
    render(<AuthScreen status={status({ anonymousIdentity: true })} />);
    // Anonymous identities start on sign-up with the "your data moves" note.
    expect(screen.getByText(/move to the new account/)).toBeInTheDocument();
    type("Email", "a@b.co");
    type("Password", "longpassword");
    type("Password (again)", "longpasswor");
    expect(screen.getByText("The passwords do not match.")).toBeInTheDocument();
    const submit = screen.getByRole("button", { name: "Create account" });
    expect(submit).toBeDisabled();
    type("Password (again)", "longpassword");
    await act(async () => fireEvent.click(submit));
    expect(ipcMock.authSignUp).toHaveBeenCalledWith("a@b.co", "longpassword");

    expect(screen.getByText("We sent a 6-digit code to a@b.co.")).toBeInTheDocument();
    type("Verification code", "123456");
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Verify" })));
    expect(ipcMock.authVerify).toHaveBeenCalledWith("a@b.co", "123456", "upgrade", "longpassword");
  });

  it("resets a forgotten password with a code and a new password", async () => {
    ipcMock.authRequestReset.mockResolvedValue(undefined);
    ipcMock.authVerify.mockResolvedValue(undefined);
    ipcMock.authResend.mockResolvedValue(undefined);
    render(<AuthScreen status={status()} />);
    fireEvent.click(screen.getByRole("button", { name: "Forgot password" }));
    type("Email", "a@b.co");
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Send code" })));
    expect(ipcMock.authRequestReset).toHaveBeenCalledWith("a@b.co");

    await act(async () =>
      fireEvent.click(screen.getByRole("button", { name: "Send the code again" })),
    );
    expect(ipcMock.authResend).toHaveBeenCalledWith("a@b.co", "recovery");
    expect(screen.getByText("A new code is on its way.")).toBeInTheDocument();

    type("Verification code", "654321");
    type("New password", "brandnewpass");
    const verify = screen.getByRole("button", { name: "Verify" });
    expect(verify).toBeDisabled();
    type("Password (again)", "brandnewpass");
    await act(async () => fireEvent.click(verify));
    expect(ipcMock.authVerify).toHaveBeenCalledWith("a@b.co", "654321", "recovery", "brandnewpass");
  });

  it("warns before signing in over an anonymous identity", () => {
    render(<AuthScreen status={status({ anonymousIdentity: true })} />);
    fireEvent.click(screen.getByRole("button", { name: "I already have an account" }));
    expect(screen.getByText(/deletes its friends and reserved names/)).toBeInTheDocument();
  });

  it("explains a ban and can sign out", async () => {
    ipcMock.authSignOut.mockResolvedValue(undefined);
    ipcMock.authStatus.mockResolvedValue(status());
    render(
      <BanScreen
        status={status({
          signedIn: true,
          email: "a@b.co",
          ban: { until: null, reason: "cheating" },
        })}
      />,
    );
    expect(screen.getByText("This ban is permanent.")).toBeInTheDocument();
    expect(screen.getByText("Reason: cheating")).toBeInTheDocument();
    await act(async () => fireEvent.click(screen.getByRole("button", { name: /Sign out/ })));
    expect(ipcMock.authSignOut).toHaveBeenCalled();
    expect(useAuth.getState().status?.signedIn).toBe(false);
  });
});
