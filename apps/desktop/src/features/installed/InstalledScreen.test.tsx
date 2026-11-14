import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { MOCK_INSTALLED } from "../../api/mock";
import { useQueue } from "../../state/queue";
import { renderApp, resetStores } from "../../test/utils";
import { type InstalledFilter, type InstalledSort, selectPackages } from "./InstalledScreen";

beforeEach(resetStores);

async function rows() {
  const list = await screen.findByRole("list", { name: "Installed packages" });
  await waitFor(() => expect(within(list).getAllByRole("listitem").length).toBeGreaterThan(0));
  return within(list).getAllByRole("listitem");
}

const names = (items: HTMLElement[]) =>
  items.map((li) => li.querySelector(".font-semibold")?.textContent);

describe("selectPackages", () => {
  it("filters by category and text, and sorts", () => {
    const pick = (f: InstalledFilter, text: string, sort: InstalledSort) =>
      selectPackages(MOCK_INSTALLED, f, text, sort).map((p) => p.name);
    expect(pick("orphans", "", "name")).toEqual(["old-codec", "python-requests"]);
    expect(pick("foreign", "", "name")).toEqual(["old-codec", "yay"]);
    expect(pick("explicit", "", "name")).toEqual(["base", "firefox", "linux", "yay"]);
    expect(pick("all", "PYTHON", "name")).toEqual(["python", "python-requests"]);
    expect(pick("all", "aur helper", "name")).toEqual(["yay"]);
    expect(pick("all", "", "size")[0]).toBe("firefox");
    expect(pick("all", "", "date")[0]).toBe("firefox");
  });
});

describe("installed screen", () => {
  it("lists every installed package with its source and status", async () => {
    renderApp("/installed");
    const items = await rows();
    expect(items).toHaveLength(MOCK_INSTALLED.length);
    expect(screen.getByText(/9 of 9 packages/)).toBeInTheDocument();

    const yay = items.find((li) => within(li).queryByText("yay"));
    expect(within(yay!).getByTestId("source-badge")).toHaveTextContent("AUR / local");
    const glibc = items.find((li) => within(li).queryByText("glibc"));
    expect(within(glibc!).getByTestId("source-badge")).toHaveTextContent("core");
    expect(within(glibc!).getByText("Dependency")).toBeInTheDocument();
    expect(
      within(glibc!).getByText(/Required by base, firefox, linux and 2 more/),
    ).toBeInTheDocument();
  });

  it("filters and searches", async () => {
    renderApp("/installed");
    const user = userEvent.setup();
    await rows();
    await user.click(screen.getByRole("radio", { name: /Orphans \(2\)/ }));
    expect(names(await rows())).toEqual(["old-codec", "python-requests"]);

    await user.click(screen.getByRole("radio", { name: /All/ }));
    await user.type(screen.getByRole("searchbox", { name: "Filter installed packages" }), "font");
    expect(names(await rows())).toEqual(["ttf-dejavu"]);

    await user.type(screen.getByRole("searchbox", { name: "Filter installed packages" }), "zzz");
    expect(await screen.findByText("No packages match")).toBeInTheDocument();
  });

  it("queues removal of all orphans", async () => {
    renderApp("/installed");
    const user = userEvent.setup();
    await rows();
    await user.click(screen.getByRole("radio", { name: /Orphans/ }));
    await user.click(screen.getByRole("button", { name: "Queue removal of all 2 orphans" }));
    expect(useQueue.getState().items.map((i) => i.action)).toEqual(["clean_orphans"]);
    expect(
      screen.getByRole("button", { name: "All orphans are queued for removal" }),
    ).toHaveAttribute("aria-pressed", "true");
  });

  it("removes a package through the queue and updates the list", async () => {
    renderApp("/installed");
    const user = userEvent.setup();
    await rows();
    await user.click(screen.getByRole("button", { name: "Queue removal of yay" }));
    expect(
      screen.getByRole("button", { name: "Remove yay from the removal queue" }),
    ).toHaveTextContent("Queued for removal");
    expect(screen.getByTestId("queue-count")).toHaveTextContent("1");

    await user.click(screen.getByRole("button", { name: /Transaction queue/ }));
    const summary = await screen.findByRole("region", { name: "Summary" });
    expect(await within(summary).findByText("yay -Rs -- yay")).toBeInTheDocument();
    await user.selectOptions(screen.getByRole("combobox", { name: "How to remove yay" }), "plain");
    expect(await screen.findByText("yay -R -- yay")).toBeInTheDocument();
    const summary2 = await screen.findByRole("region", { name: "Summary" });

    const run = within(summary2).getByRole("button", { name: "Run" });
    await waitFor(() => expect(run).toBeEnabled());
    await user.click(run);
    await user.type(await screen.findByLabelText("Password"), "pw{Enter}");
    const dialog = await screen.findByRole("dialog", { name: "Question from yay" });
    expect(dialog).toHaveTextContent("Do you want to remove these packages?");
    await user.click(within(dialog).getByRole("button", { name: "Yes" }));
    expect(await screen.findByText(/all done/i)).toBeInTheDocument();

    await user.click(screen.getByRole("link", { name: "Installed" }));
    await waitFor(() => expect(screen.getByText(/8 of 8 packages/)).toBeInTheDocument());
    expect(names(await rows())).not.toContain("yay");
  });

  it("reports an unreadable database", async () => {
    const { mockBackend } = await import("../../api/mock");
    mockBackend.installed = async () => ({ packages: [], error: "no such file: /var/lib/pacman" });
    renderApp("/installed");
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("The package database could not be read");
    expect(alert).toHaveTextContent("/var/lib/pacman");
  });
});
