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
import { TitleBar } from "./TitleBar";

describe("TitleBar", () => {
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
