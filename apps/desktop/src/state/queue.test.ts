import { beforeEach, describe, expect, it } from "vitest";
import { MOCK_PACKAGES } from "../api/mock";
import { packageKey, useQueue } from "./queue";

const [firefox, , , , nightly] = MOCK_PACKAGES;

describe("queue store", () => {
  beforeEach(() => useQueue.setState({ items: [] }));

  it("adds items and ignores duplicates", () => {
    useQueue.getState().add(firefox!);
    useQueue.getState().add(firefox!);
    useQueue.getState().add(nightly!);
    expect(useQueue.getState().items.map((i) => i.key)).toEqual([
      "extra/firefox",
      "aur/firefox-nightly-bin",
    ]);
  });

  it("removes by key and clears", () => {
    useQueue.getState().add(firefox!);
    useQueue.getState().add(nightly!);
    useQueue.getState().remove(packageKey(firefox!));
    expect(useQueue.getState().items).toHaveLength(1);
    useQueue.getState().clear();
    expect(useQueue.getState().items).toHaveLength(0);
  });
});
