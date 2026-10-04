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
import { UpdateButton } from "./UpdateButton";

describe("UpdateButton", () => {
  beforeEach(() => {
    applyLanguage("en");
    useUpdate.setState({ info: null, installing: null, checking: false });
  });

  it("stays hidden unless an update is available", () => {
    useUpdate.setState({
      info: { status: "unavailable", current: "0.1.0", version: null, notes: null },
    });
    const { container } = render(<UpdateButton />);
    expect(container).toBeEmptyDOMElement();
  });

  it("installs on click and shows progress", async () => {
    useUpdate.setState({
      info: { status: "available", current: "0.1.0", version: "0.2.0", notes: null },
    });
    render(<UpdateButton />);
    const button = screen.getByRole("button", { name: /Update to 0\.2\.0/ });
    fireEvent.click(button);
    await vi.waitFor(() => expect(ipcMock.installAppUpdate).toHaveBeenCalled());
    expect(await screen.findByText(/Downloading 0%/)).toBeInTheDocument();
    expect(button).toBeDisabled();
  });
});
