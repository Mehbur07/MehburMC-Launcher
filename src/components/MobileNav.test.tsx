import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import "../i18n";
import { applyLanguage } from "../i18n";
import { useMobileHistory } from "../app/mobileHistory";
import { useApp } from "../stores/app";
import { useAuth } from "../stores/auth";
import { useFriends } from "../stores/friends";
import { useTasks } from "../stores/tasks";
import { task } from "../test/fixtures";
import { MobileNav } from "./Sidebar";

function Harness() {
  useMobileHistory(true);
  return <MobileNav />;
}

const tab = (name: RegExp) => within(screen.getByRole("navigation")).getByRole("button", { name });

describe("MobileNav (telefon menüsü)", () => {
  beforeEach(() => {
    applyLanguage("en");
    window.history.replaceState(null, "");
    useApp.setState({ view: "home" });
    useTasks.setState({ tasks: {} });
    useFriends.setState({ friends: [] });
    useAuth.setState({ status: null });
  });

  it("shows four tabs and a menu button", () => {
    render(<MobileNav />);
    for (const name of [/Play/, /Instances/, /Mod Browser/, /Friends/, /Menu/]) {
      expect(tab(name)).toBeInTheDocument();
    }
    expect(tab(/Play/)).toHaveAttribute("aria-current", "page");
    fireEvent.click(tab(/Instances/));
    expect(useApp.getState().view).toBe("instances");
  });

  it("uses tabs a local account can open", () => {
    useAuth.setState({
      status: {
        signedIn: false,
        email: null,
        anonymousIdentity: false,
        rank: 0,
        ban: null,
        offline: false,
        localAccount: true,
      },
    });
    render(<MobileNav />);
    expect(tab(/Skin & Cape/)).toBeInTheDocument();
    expect(
      within(screen.getByRole("navigation")).queryByRole("button", { name: /Friends/ }),
    ).toBeNull();
  });

  it("opens the drawer with every page and navigates from it", async () => {
    render(<Harness />);
    fireEvent.click(tab(/Menu/));
    const drawer = screen.getByRole("dialog", { name: "Menu" });
    expect(within(drawer).getByRole("button", { name: /Settings/ })).toBeInTheDocument();
    expect(within(drawer).getByRole("button", { name: /Console/ })).toBeInTheDocument();
    fireEvent.click(within(drawer).getByRole("button", { name: /Settings/ }));
    // Çekmecenin geçmiş kaydı geri alındıktan sonra sayfa açılır.
    await waitFor(() => expect(useApp.getState().view).toBe("settings"));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(tab(/Menu/).className).toContain("text-accent");
  });

  it("marks the menu button when a page inside it has news", () => {
    useTasks.setState({ tasks: { a: task({ id: "a" }) } });
    render(<MobileNav />);
    // İndirmeler menüde: Menü düğmesinde nokta görünür.
    expect(tab(/Menu/).querySelector(".rounded-full")).not.toBeNull();
  });

  it("closes the drawer on the back button", async () => {
    render(<Harness />);
    fireEvent.click(tab(/Menu/));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    act(() => window.history.back());
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(useApp.getState().view).toBe("home");
  });
});
