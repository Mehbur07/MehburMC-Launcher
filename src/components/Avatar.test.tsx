import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Avatar } from "./Avatar";

describe("Avatar", () => {
  it("prefers the photo, then the skin head, then an icon", () => {
    const photo = render(<Avatar photo="data:image/png;base64,QQ" skin="data:x" />);
    expect(photo.container.querySelector("img")).toHaveAttribute("src", "data:image/png;base64,QQ");
    expect(photo.container.querySelector("canvas")).toBeNull();

    const head = render(<Avatar photo={null} skin="data:image/png;base64,QQ" />);
    expect(head.container.querySelector("img")).toBeNull();
    expect(head.container.querySelector("canvas")).not.toBeNull();

    const none = render(<Avatar photo={undefined} />);
    expect(none.container.querySelector("canvas, img")).toBeNull();
    expect(none.container.querySelector("svg")).not.toBeNull();
  });
});
