import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const win = vi.hoisted(() => ({
  isMaximized: vi.fn(async () => false),
  onResized: vi.fn(async () => () => {}),
  minimize: vi.fn(async () => {}),
  toggleMaximize: vi.fn(async () => {}),
  close: vi.fn(async () => {}),
}));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => win }));

import "../i18n";
import { applyLanguage } from "../i18n";
import { useApp } from "../stores/app";
import { useUpdate } from "../stores/update";
import { TitleBar } from "./TitleBar";

describe("TitleBar", () => {
  it("shows the installed version with the update button right after it", () => {
    applyLanguage("en");
    useApp.setState({
      boot: {
        appName: "MehburMC Launcher",
        version: "0.2.1",
        paths: null,
        settings: useApp.getState().settings!,
        startupError: null,
        updatedFrom: null,
      },
    });
    useUpdate.setState({
      info: { status: "available", current: "0.2.1", version: "0.3.0", notes: null },
      installing: null,
    });
    render(<TitleBar />);
    const version = screen.getByText("v0.2.1");
    const update = screen.getByRole("button", { name: /Update to 0\.3\.0/ });
    expect(version.nextElementSibling).toBe(update);
    useApp.setState({ boot: null });
    useUpdate.setState({ info: null });
  });

  it("shows the app name, is draggable and wires window controls", () => {
    const { container } = render(<TitleBar />);
    expect(container.querySelector("[data-tauri-drag-region]")).not.toBeNull();
    expect(screen.getByText("Launcher")).toBeInTheDocument();

    const buttons = screen.getAllByRole("button");
    expect(buttons).toHaveLength(3);
    buttons.forEach((b) => fireEvent.click(b));
    expect(win.minimize).toHaveBeenCalled();
    expect(win.toggleMaximize).toHaveBeenCalled();
    expect(win.close).toHaveBeenCalled();
  });
});
