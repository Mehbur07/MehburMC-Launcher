import { act, render, waitFor } from "@testing-library/react";
import { useState } from "react";
import { beforeEach, describe, expect, it } from "vitest";

import { useApp } from "../stores/app";
import { useBackLayer, useMobileHistory } from "./mobileHistory";

function Pages() {
  useMobileHistory(true);
  return null;
}

let setLayer: (open: boolean) => void = () => {};
let setInner: (open: boolean) => void = () => {};

function Layers() {
  useMobileHistory(true);
  const [outer, setOuter] = useState(false);
  const [inner, setIn] = useState(false);
  setLayer = setOuter;
  setInner = setIn;
  useBackLayer(outer, () => setOuter(false), true);
  useBackLayer(inner, () => setIn(false), true);
  return (
    <p>
      {outer ? "outer" : ""} {inner ? "inner" : ""}
    </p>
  );
}

describe("mobile history (geri tuşu)", () => {
  beforeEach(() => {
    window.history.replaceState(null, "");
    useApp.setState({ view: "home" });
  });

  it("goes back to the previous page", async () => {
    render(<Pages />);
    act(() => useApp.getState().setView("instances"));
    act(() => useApp.getState().setView("settings"));
    act(() => window.history.back());
    await waitFor(() => expect(useApp.getState().view).toBe("instances"));
    act(() => window.history.back());
    await waitFor(() => expect(useApp.getState().view).toBe("home"));
  });

  it("closes only the top layer", async () => {
    const { container } = render(<Layers />);
    act(() => setLayer(true));
    act(() => setInner(true));
    expect(container.textContent).toContain("outer");
    expect(container.textContent).toContain("inner");
    act(() => window.history.back());
    await waitFor(() => expect(container.textContent).not.toContain("inner"));
    expect(container.textContent).toContain("outer");
    act(() => window.history.back());
    await waitFor(() => expect(container.textContent).not.toContain("outer"));
    expect(useApp.getState().view).toBe("home");
  });

  it("removes its history entry when closed by a button", async () => {
    render(<Layers />);
    act(() => setLayer(true));
    expect((window.history.state as { layer?: number }).layer).toBe(1);
    act(() => setLayer(false));
    await waitFor(() =>
      expect((window.history.state as { layer?: number } | null)?.layer).toBeUndefined(),
    );
  });
});
