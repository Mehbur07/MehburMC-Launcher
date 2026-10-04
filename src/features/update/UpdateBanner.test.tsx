import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({ installAppUpdate: vi.fn(() => new Promise(() => {})) }));
vi.mock("../../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../../lib/ipc")>()),
  ipc: ipcMock,
  onUpdateProgress: vi.fn(async () => () => {}),
}));

import "../../i18n";
import { applyLanguage } from "../../i18n";
import { useUpdate } from "../../stores/update";
import { UpdateBanner } from "./UpdateBanner";

describe("UpdateBanner", () => {
  beforeEach(() => {
    applyLanguage("en");
    useUpdate.setState({ info: null, installing: null, dismissed: false, checking: false });
  });

  it("stays hidden unless an update is available", () => {
    useUpdate.setState({
      info: { status: "unavailable", current: "0.1.0", version: null, notes: null },
    });
    const { container } = render(<UpdateBanner />);
    expect(container).toBeEmptyDOMElement();
  });

  it("installs on request and can be dismissed", async () => {
    useUpdate.setState({
      info: { status: "available", current: "0.1.0", version: "0.2.0", notes: null },
    });
    render(<UpdateBanner />);
    expect(screen.getByText(/0\.2\.0/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Update and restart/ }));
    await vi.waitFor(() => expect(ipcMock.installAppUpdate).toHaveBeenCalled());
    expect(await screen.findByRole("progressbar")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(useUpdate.getState().dismissed).toBe(true);
  });
});
