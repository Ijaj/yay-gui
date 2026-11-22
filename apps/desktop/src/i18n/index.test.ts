import { describe, expect, it } from "vitest";
import { catalogs, type MessageKey, t } from "./index";

// Every catalog file on disk, so a file missing from `catalogs` fails a test.
const files = import.meta.glob<Record<string, string>>("./en/*.json", {
  eager: true,
  import: "default",
});

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

  it("every catalog file is registered in catalogs", () => {
    const prefixes = Object.keys(files).map((f) => f.replace(/^\.\/en\/(.*)\.json$/, "$1"));
    expect(prefixes.length).toBeGreaterThan(0);
    expect(Object.keys(catalogs).sort()).toEqual(prefixes.sort());
  });

  it("t() resolves every key of every file to its own value", () => {
    for (const [file, catalog] of Object.entries(files)) {
      for (const [key, value] of Object.entries(catalog)) {
        expect(t(key as MessageKey), `${key} from ${file}`).toBe(value);
      }
    }
  });
});
