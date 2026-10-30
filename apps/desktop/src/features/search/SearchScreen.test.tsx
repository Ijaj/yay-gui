import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { renderApp, resetStores } from "../../test/utils";

beforeEach(resetStores);

/** Wait for the virtualised list to have rendered rows. */
async function rows() {
  const list = await screen.findByRole("list", { name: /search results/i });
  await waitFor(() => expect(within(list).getAllByRole("listitem").length).toBeGreaterThan(0));
  return list;
}

async function search(text: string) {
  const user = userEvent.setup();
  await user.type(screen.getByRole("searchbox", { name: /search packages/i }), text);
  return user;
}

describe("search screen", () => {
  it("shows the initial empty state", () => {
    renderApp();
    expect(screen.getByText("Search for packages")).toBeInTheDocument();
  });

  it("renders mock results, repo first then AUR", async () => {
    renderApp();
    await search("firefox");
    const list = await rows();
    const items = within(list).getAllByRole("listitem");
    const badges = items.map((li) => within(li).getByTestId("source-badge").textContent);
    expect(badges).toEqual(["extra", "extra", "AUR", "AUR", "AUR"]);
    expect(within(items[0]!).getByText("Installed")).toBeInTheDocument();
    expect(within(items[3]!).getByText("Out of date")).toBeInTheDocument();
    expect(within(items[4]!).getByText("Orphaned")).toBeInTheDocument();
  });

  it("filters by source", async () => {
    renderApp();
    const user = await search("firefox");
    await rows();
    await user.click(screen.getByRole("radio", { name: "AUR" }));
    const list = screen.getByRole("list", { name: /search results/i });
    const badges = within(list)
      .getAllByTestId("source-badge")
      .map((b) => b.textContent);
    expect(badges).toEqual(["AUR", "AUR", "AUR"]);
    await user.click(screen.getByRole("radio", { name: "Repos" }));
    expect(
      within(screen.getByRole("list", { name: /search results/i }))
        .getAllByTestId("source-badge")
        .map((b) => b.textContent),
    ).toEqual(["extra", "extra"]);
  });

  it("filters installed only", async () => {
    renderApp();
    const user = await search("firefox");
    await rows();
    await user.click(screen.getByRole("checkbox", { name: /installed only/i }));
    const items = within(screen.getByRole("list", { name: /search results/i })).getAllByRole(
      "listitem",
    );
    expect(items).toHaveLength(1);
  });

  it("shows the empty state when nothing matches", async () => {
    renderApp();
    await search("zzzznothing");
    expect(await screen.findByText("No packages found")).toBeInTheDocument();
  });

  it("shows a non-blocking banner on AUR error and still lists repo results", async () => {
    renderApp();
    await search("aurfail");
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(/AUR results are unavailable/);
    expect(await rows()).toBeInTheDocument();
  });

  it("adds a package to the queue from a row", async () => {
    renderApp();
    const user = await search("yay");
    await rows();
    await user.click(screen.getByRole("button", { name: /add to queue: yay/i }));
    expect(screen.getByTestId("queue-count")).toHaveTextContent("1");
  });

  it("opens the package detail when a row is clicked", async () => {
    renderApp();
    const user = await search("yay");
    await rows();
    await user.click(screen.getByRole("button", { name: /^yay/ }));
    await waitFor(() => expect(screen.getByRole("heading", { name: "yay" })).toBeInTheDocument());
    expect(screen.getByText("jguer")).toBeInTheDocument();
  });
});
