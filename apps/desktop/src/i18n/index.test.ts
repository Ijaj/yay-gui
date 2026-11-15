import { describe, expect, it } from "vitest";
import { catalogs, t } from "./index";

const EXPECTED_KEY_COUNT = 200;

describe("i18n catalogs", () => {
  it("every key starts with its file's prefix", () => {
    for (const [prefix, catalog] of Object.entries(catalogs)) {
      for (const key of Object.keys(catalog)) {
        expect(key.startsWith(`${prefix}.`), `${key} in ${prefix}.json`).toBe(true);
      }
    }
  });

  it("no key appears in two files", () => {
    const seen = new Set<string>();
    for (const catalog of Object.values(catalogs)) {
      for (const key of Object.keys(catalog)) {
        expect(seen.has(key), `duplicate key ${key}`).toBe(false);
        seen.add(key);
      }
    }
  });

  it("the merged catalog has the expected number of keys", () => {
    // Checked once against the old single en.json (200 keys, same values).
    const total = Object.values(catalogs).reduce((n, c) => n + Object.keys(c).length, 0);
    expect(total).toBe(EXPECTED_KEY_COUNT);
    expect(t("queue.title")).toBe(catalogs.queue["queue.title"]);
  });
});
