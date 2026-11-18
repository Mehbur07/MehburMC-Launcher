import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  installPerformanceMods: vi.fn(),
  installShaderSupport: vi.fn(),
}));
vi.mock("../../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../../lib/ipc")>()),
  ipc: ipcMock,
}));

import "../../i18n";
import { applyLanguage } from "../../i18n";
import { instance } from "../../test/fixtures";
import { GeneralTab } from "./GeneralTab";

describe("GeneralTab performance mods (K111)", () => {
  beforeEach(() => {
    applyLanguage("en");
    vi.clearAllMocks();
  });

  it("installs them and reports installed, present and unavailable mods", async () => {
    ipcMock.installPerformanceMods.mockResolvedValue({
      installed: ["modernfix-5.0.jar"],
      alreadyPresent: [],
      unavailable: ["FerriteCore"],
      incompatible: [],
    });
    render(<GeneralTab inst={instance()} />);
    expect(screen.getByText("Performance mods")).toBeTruthy();
    const buttons = screen.getAllByRole("button", { name: /One-click install/ });
    fireEvent.click(buttons[buttons.length - 1]!);
    await waitFor(() => expect(ipcMock.installPerformanceMods).toHaveBeenCalledWith("pack-1"));
    expect(
      await screen.findByText(
        "Installed: modernfix-5.0.jar · Not available for this version: FerriteCore",
      ),
    ).toBeTruthy();
  });

  it("is not offered without a mod loader", () => {
    render(<GeneralTab inst={instance({ loader: { kind: "vanilla" } })} />);
    expect(screen.queryByText("Performance mods")).toBeNull();
  });
});
