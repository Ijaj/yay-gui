import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { MOCK_PACKAGES } from "../../api/mock";
import { useQueue } from "../../state/queue";
import { renderApp, resetStores } from "../../test/utils";

beforeEach(resetStores);

const pkg = (name: string) => {
  const p = MOCK_PACKAGES.find((x) => x.name === name);
  if (!p) throw new Error(name);
  return p;
};

function queue(...names: string[]) {
  act(() => {
    for (const n of names) useQueue.getState().add(pkg(n));
  });
}

async function summary() {
  return screen.findByRole("region", { name: "Summary" });
}

async function approve(user: ReturnType<typeof userEvent.setup>, name: string) {
  await user.click(await screen.findByRole("link", { name: `Review ${name}` }));
  await screen.findByRole("heading", { name: `Review ${name}` });
  await user.click(screen.getByRole("button", { name: /^Approve commit/ }));
  await screen.findByRole("heading", { name: "Queue" });
}

describe("queue → review → run", () => {
  it("requires review, then runs with password and confirmation prompts", async () => {
    queue("firefox-nightly-bin");
    renderApp("/queue");
    const user = userEvent.setup();

    const s = await summary();
    expect(await within(s).findByRole("alert")).toHaveTextContent(/review every aur package/i);
    expect(within(s).getByRole("button", { name: "Run" })).toBeDisabled();
    expect(within(s).getByText("Not reviewed")).toBeInTheDocument();
    expect(
      within(s).getByText(
        "yay -S --needed --answerdiff None --answeredit None -- firefox-nightly-bin",
      ),
    ).toBeInTheDocument();

    // Review screen: PKGBUILD tab first, binary file not shown.
    await user.click(screen.getByRole("link", { name: "Review firefox-nightly-bin" }));
    await screen.findByRole("heading", { name: "Review firefox-nightly-bin" });
    expect(screen.getByRole("tab", { name: "PKGBUILD" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByTestId("code-viewer").dataset.text).toContain("pkgname=firefox-nightly-bin");
    await user.click(screen.getByRole("tab", { name: "icon.png" }));
    expect(screen.getByText(/binary or large file/i)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /^Approve commit/ }));

    const s2 = await summary();
    await within(s2).findByText("Reviewed");
    const run = within(s2).getByRole("button", { name: "Run" });
    await waitFor(() => expect(run).toBeEnabled());
    await user.click(run);

    // Wrong password first.
    await screen.findByRole("heading", { name: "Transaction" });
    await user.type(await screen.findByLabelText("Password"), "wrong{Enter}");
    await screen.findByRole("dialog", { name: "Password for user" });
    await waitFor(() => expect(screen.getByLabelText("Password")).toHaveValue(""));
    await user.type(screen.getByLabelText("Password"), "hunter2{Enter}");

    const dialog = await screen.findByRole("dialog", { name: "Question from yay" });
    expect(dialog).toHaveTextContent("Proceed with installation?");
    await user.click(within(dialog).getByRole("button", { name: "Yes" }));

    expect(await screen.findByText(/all done/i)).toBeInTheDocument();
    expect(screen.getByRole("log", { name: "Output" })).toHaveTextContent("installing... done");
    expect(screen.getByRole("log", { name: "Output" })).not.toHaveTextContent("hunter2");
    expect(useQueue.getState().items).toHaveLength(0);
  });

  it("explains partial upgrades and fixes them with one click", async () => {
    queue("firefox");
    renderApp("/queue");
    const user = userEvent.setup();
    const s = await summary();
    expect(await within(s).findByRole("alert")).toHaveTextContent(/partial upgrade/i);
    await user.click(within(s).getByRole("button", { name: "Add full system upgrade" }));
    expect(screen.getByRole("checkbox", { name: "Full system upgrade" })).toBeChecked();
    expect(await screen.findByText("yay -Syu --needed -- firefox")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("button", { name: "Run" })).toBeEnabled());
  });

  it("shows the diff since the last approval first", async () => {
    queue("firefox-esr-bin");
    renderApp("/queue");
    const user = userEvent.setup();
    const s = await summary();
    expect(await within(s).findByText("Changed since review")).toBeInTheDocument();
    await user.click(within(s).getByRole("link", { name: "Review firefox-esr-bin" }));
    const tab = await screen.findByRole("tab", { name: "Changes since review" });
    expect(tab).toHaveAttribute("aria-selected", "true");
    expect(screen.getByTestId("code-viewer").dataset.text).toContain("+pkgrel=2");
    expect(screen.getByText(/you approved commit/i)).toBeInTheDocument();
  });

  it("requires reviewing AUR dependencies and reports build failures", async () => {
    queue("firefox-legacy-git");
    renderApp("/queue");
    const user = userEvent.setup();
    const s = await summary();
    expect(await within(s).findByText("dependency of firefox-legacy-git")).toBeInTheDocument();
    await approve(user, "firefox-legacy-git");
    expect(await within(await summary()).findByRole("alert")).toBeInTheDocument();
    await approve(user, "legacy-ui-lib");
    const run = within(await summary()).getByRole("button", { name: "Run" });
    await waitFor(() => expect(run).toBeEnabled());
    await user.click(run);
    await user.type(await screen.findByLabelText("Password"), "pw{Enter}");
    await user.click(await screen.findByRole("button", { name: "Yes" }));
    expect(await screen.findByText(/the transaction failed/i)).toBeInTheDocument();
    expect(screen.getByText(/failed to build/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
    expect(useQueue.getState().items).toHaveLength(1);
  });

  it("can be cancelled while waiting for the password", async () => {
    queue("firefox");
    act(() => useQueue.getState().setUpgrade(true));
    renderApp("/queue");
    const user = userEvent.setup();
    const run = within(await summary()).getByRole("button", { name: "Run" });
    await waitFor(() => expect(run).toBeEnabled());
    await user.click(run);
    await screen.findByLabelText("Password");
    expect(screen.getByRole("link", { name: /transaction running/i })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(await screen.findByText("The transaction was cancelled.")).toBeInTheDocument();
  });

  it("opens the queue with Ctrl+Enter", async () => {
    renderApp("/");
    const user = userEvent.setup();
    await user.keyboard("{Control>}{Enter}{/Control}");
    expect(screen.getByRole("heading", { name: "Queue" })).toBeInTheDocument();
    expect(screen.getByText("The queue is empty")).toBeInTheDocument();
  });

  it("opens the queue from the header", async () => {
    renderApp("/search");
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /transaction queue/i }));
    expect(screen.getByRole("heading", { name: "Queue" })).toBeInTheDocument();
  });
});
