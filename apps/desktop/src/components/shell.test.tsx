import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { renderApp, resetStores } from "../test/utils";

beforeEach(resetStores);

describe("app shell", () => {
  it("focuses the global search with /", async () => {
    renderApp("/");
    const user = userEvent.setup();
    const input = screen.getByRole("searchbox", { name: /search packages/i });
    expect(input).not.toHaveFocus();
    await user.keyboard("/");
    expect(input).toHaveFocus();
    // typing "/" inside the box must not be swallowed
    await user.keyboard("a/b");
    expect(input).toHaveValue("a/b");
  });

  it("toggles the terminal drawer with Ctrl+J", async () => {
    renderApp("/");
    const user = userEvent.setup();
    expect(screen.queryByText("No transaction running")).not.toBeInTheDocument();
    await user.keyboard("{Control>}j{/Control}");
    expect(screen.getByText("No transaction running")).toBeInTheDocument();
    await user.keyboard("{Control>}j{/Control}");
    expect(screen.queryByText("No transaction running")).not.toBeInTheDocument();
  });

  it("lists the sidebar screens and renders placeholders", async () => {
    renderApp("/");
    const user = userEvent.setup();
    const nav = screen.getByRole("navigation", { name: /main navigation/i });
    for (const name of ["Home", "Search", "Updates", "Installed", "Maintenance", "Settings"]) {
      expect(nav).toHaveTextContent(name);
    }
    await user.click(screen.getByRole("link", { name: "Updates" }));
    expect(screen.getByRole("heading", { name: "Updates" })).toBeInTheDocument();
  });
});
