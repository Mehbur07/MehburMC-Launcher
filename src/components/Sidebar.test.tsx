import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import "../i18n";
import { applyLanguage } from "../i18n";
import { useApp } from "../stores/app";
import { useFriends } from "../stores/friends";
import { useTasks } from "../stores/tasks";
import { task } from "../test/fixtures";
import { Sidebar } from "./Sidebar";

describe("Sidebar", () => {
  beforeEach(() => {
    applyLanguage("en");
    useApp.setState({ view: "home" });
    useTasks.setState({ tasks: {} });
  });

  it("navigates and marks the current page", () => {
    render(<Sidebar />);
    fireEvent.click(screen.getByRole("button", { name: /Settings/ }));
    expect(useApp.getState().view).toBe("settings");
    expect(screen.getByRole("button", { name: /Settings/ })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("treats the instance detail page as part of Instances", () => {
    useApp.setState({ view: "instance" });
    render(<Sidebar />);
    expect(screen.getByRole("button", { name: /Instances/ })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("counts running downloads", () => {
    useTasks.setState({
      tasks: {
        a: task({ id: "a" }),
        b: task({ id: "b" }),
        c: task({ id: "c", status: "completed" }),
      },
    });
    render(<Sidebar />);
    expect(screen.getByRole("button", { name: /Downloads/ })).toHaveTextContent("2");
  });

  it("shows unread messages on Friends", () => {
    useFriends.setState({
      friends: [
        {
          id: "f1",
          friendCode: "MEHBUR-AAAA",
          displayName: "Ali",
          status: "accepted",
          incoming: false,
          requestId: 1,
          unread: 3,
          avatar: null,
        },
      ],
    });
    render(<Sidebar />);
    expect(screen.getByRole("button", { name: /Friends/ })).toHaveTextContent("3");
  });
});
