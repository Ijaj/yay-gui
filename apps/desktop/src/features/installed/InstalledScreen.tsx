import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useMemo, useRef, useState } from "react";
import { api, type InstalledPackage } from "../../api";
import { SourceBadge } from "../../components/SourceBadge";
import { t } from "../../i18n";
import { ORPHANS_KEY, removalKey, useQueue } from "../../state/queue";
import { formatDate, formatSize } from "../search/formatters";

export const INSTALLED_KEY = ["installed"] as const;
const ROW_HEIGHT = 88;

export type InstalledFilter = "all" | "explicit" | "deps" | "foreign" | "orphans";
export type InstalledSort = "name" | "size" | "date";

const FILTERS: InstalledFilter[] = ["all", "explicit", "deps", "foreign", "orphans"];
const SORTS: InstalledSort[] = ["name", "size", "date"];

const BUTTON =
  "rounded-ui-sm border border-border bg-surface px-3 py-1.5 font-medium hover:bg-surface-2 disabled:cursor-not-allowed disabled:opacity-50";

export function matchesFilter(p: InstalledPackage, f: InstalledFilter): boolean {
  switch (f) {
    case "all":
      return true;
    case "explicit":
      return p.explicit;
    case "deps":
      return !p.explicit;
    case "foreign":
      return p.repo === null;
    case "orphans":
      return p.orphan;
  }
}

/** Filter by category and text, then sort (largest and newest first). */
export function selectPackages(
  pkgs: InstalledPackage[],
  filter: InstalledFilter,
  text: string,
  sort: InstalledSort,
): InstalledPackage[] {
  const needle = text.trim().toLowerCase();
  const out = pkgs.filter(
    (p) =>
      matchesFilter(p, filter) &&
      (needle === "" ||
        p.name.toLowerCase().includes(needle) ||
        p.description.toLowerCase().includes(needle)),
  );
  const byName = (a: InstalledPackage, b: InstalledPackage) => a.name.localeCompare(b.name);
  const cmp: Record<InstalledSort, (a: InstalledPackage, b: InstalledPackage) => number> = {
    name: byName,
    size: (a, b) => b.installedSize - a.installedSize || byName(a, b),
    date: (a, b) => (b.installDate ?? 0) - (a.installDate ?? 0) || byName(a, b),
  };
  return out.sort(cmp[sort]);
}

function nameList(names: string[]): string {
  const shown = names.slice(0, 3).join(", ");
  return names.length > 3 ? t("installed.more", { names: shown, count: names.length - 3 }) : shown;
}

function Pill({ className, children }: { className: string; children: React.ReactNode }) {
  return (
    <span className={`rounded-full px-2 py-0.5 text-xs font-semibold ${className}`}>
      {children}
    </span>
  );
}

function InstalledRow({ pkg }: { pkg: InstalledPackage }) {
  const key = removalKey(pkg.name);
  const queued = useQueue((s) => s.items.some((i) => i.key === key));
  const addRemoval = useQueue((s) => s.addRemoval);
  const remove = useQueue((s) => s.remove);
  const required = pkg.requiredBy.length > 0;

  const details = [
    formatSize(pkg.installedSize),
    pkg.installDate !== null
      ? t("installed.installedOn", { date: formatDate(pkg.installDate) })
      : null,
  ].filter(Boolean);

  return (
    <div className="flex h-full items-center gap-3 border-b border-border px-4">
      <div className="flex min-w-0 flex-1 flex-col gap-0.5 py-2">
        <span className="flex w-full items-center gap-2">
          <span className="truncate font-semibold">{pkg.name}</span>
          <span className="text-muted">{pkg.version}</span>
          {pkg.repo !== null ? (
            <SourceBadge source={{ kind: "repo", repo: pkg.repo }} />
          ) : (
            <span
              data-testid="source-badge"
              className="inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-semibold"
              style={{ color: "var(--badge-aur)", borderColor: "var(--badge-aur)" }}
            >
              {t("installed.foreign")}
            </span>
          )}
          {!pkg.explicit && (
            <Pill className="bg-surface-2 text-muted">{t("installed.dependency")}</Pill>
          )}
          {pkg.orphan && (
            <Pill className="bg-warning-bg text-warning">{t("installed.orphan")}</Pill>
          )}
        </span>
        <span className="w-full truncate text-muted">{pkg.description}</span>
        <span className="w-full truncate text-xs text-muted">
          {details.join(" · ")}
          {required && (
            <>
              {" · "}
              <span className={queued ? "text-warning" : undefined}>
                {t("installed.requiredBy", { names: nameList(pkg.requiredBy) })}
              </span>
            </>
          )}
          {!required && pkg.optionalFor.length > 0 && (
            <>
              {" · "}
              {t("installed.optionalFor", { names: nameList(pkg.optionalFor) })}
            </>
          )}
        </span>
      </div>
      <button
        type="button"
        aria-pressed={queued}
        aria-label={
          queued
            ? t("installed.unqueueLabel", { name: pkg.name })
            : t("installed.removeLabel", { name: pkg.name })
        }
        title={required ? t("installed.requiredWarning", { name: pkg.name }) : undefined}
        onClick={() => (queued ? remove(key) : addRemoval(pkg))}
        className={`shrink-0 rounded-ui-sm px-3 py-1.5 font-medium ${
          queued
            ? "border border-border bg-surface hover:bg-surface-2"
            : "border border-danger text-danger hover:bg-danger-bg"
        }`}
      >
        {queued ? t("installed.queuedRemoval") : t("installed.remove")}
      </button>
    </div>
  );
}

function PackageList({ items }: { items: InstalledPackage[] }) {
  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 6,
  });
  return (
    <div
      ref={parentRef}
      className="min-h-0 flex-1 overflow-auto rounded-ui border border-border bg-surface"
    >
      <ul
        aria-label={t("installed.list")}
        className="relative m-0 list-none p-0"
        style={{ height: virtualizer.getTotalSize() }}
      >
        {virtualizer.getVirtualItems().map((v) => {
          const pkg = items[v.index];
          if (!pkg) return null;
          return (
            <li
              key={pkg.name}
              className="absolute left-0 top-0 w-full"
              style={{ height: v.size, transform: `translateY(${v.start}px)` }}
            >
              <InstalledRow pkg={pkg} />
            </li>
          );
        })}
      </ul>
    </div>
  );
}

function OrphansBanner({ count }: { count: number }) {
  const queued = useQueue((s) => s.items.some((i) => i.key === ORPHANS_KEY));
  const setCleanOrphans = useQueue((s) => s.setCleanOrphans);
  if (count === 0) return <p className="m-0 text-muted">{t("installed.noOrphans")}</p>;
  return (
    <div className="flex flex-wrap items-center gap-3 rounded-ui border border-border bg-surface p-3">
      <p className="m-0 text-muted">{t("installed.orphansHint")}</p>
      <button
        type="button"
        className={`ml-auto ${BUTTON}`}
        aria-pressed={queued}
        onClick={() => setCleanOrphans(!queued)}
      >
        {queued ? t("installed.orphansQueued") : t("installed.cleanOrphans", { count })}
      </button>
    </div>
  );
}

export function InstalledScreen() {
  const client = useQueryClient();
  const { data, isLoading, isError, error } = useQuery({
    queryKey: INSTALLED_KEY,
    queryFn: () => api.listInstalled(false),
    retry: false,
  });
  const [filter, setFilter] = useState<InstalledFilter>("all");
  const [text, setText] = useState("");
  const [sort, setSort] = useState<InstalledSort>("name");
  const [refreshing, setRefreshing] = useState(false);

  const all = data?.packages ?? [];
  const items = useMemo(() => selectPackages(all, filter, text, sort), [all, filter, text, sort]);
  const counts = useMemo(
    () =>
      Object.fromEntries(FILTERS.map((f) => [f, all.filter((p) => matchesFilter(p, f)).length])),
    [all],
  ) as Record<InstalledFilter, number>;
  const size = items.reduce((n, p) => n + p.installedSize, 0);

  const refresh = async () => {
    setRefreshing(true);
    try {
      client.setQueryData(INSTALLED_KEY, await api.listInstalled(true));
    } catch {
      await client.invalidateQueries({ queryKey: INSTALLED_KEY });
    } finally {
      setRefreshing(false);
    }
  };

  const dbError = isError ? String(error) : data?.error;
  let body: React.ReactNode;
  if (isLoading) {
    body = (
      <p role="status" className="mt-16 text-center text-muted">
        {t("installed.loading")}
      </p>
    );
  } else if (dbError) {
    body = (
      <div role="alert" className="mx-auto mt-16 max-w-md text-center">
        <h2 className="text-lg font-semibold">{t("installed.error.title")}</h2>
        <p className="mt-1 text-muted">{dbError}</p>
      </div>
    );
  } else if (items.length === 0) {
    body =
      filter === "orphans" && text.trim() === "" ? null : (
        <div className="mx-auto mt-16 max-w-md text-center">
          <h2 className="text-lg font-semibold">{t("installed.empty.title")}</h2>
          <p className="mt-1 text-muted">{t("installed.empty.desc")}</p>
        </div>
      );
  } else {
    body = <PackageList items={items} />;
  }

  return (
    <section className="flex h-full flex-col gap-3 p-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-2xl font-semibold">{t("installed.title")}</h1>
        <div className="flex items-center gap-3">
          {data && !dbError && (
            <span className="text-muted" aria-live="polite">
              {t("installed.count", {
                shown: items.length,
                total: all.length,
                size: formatSize(size),
              })}
            </span>
          )}
          <button type="button" className={BUTTON} onClick={refresh} disabled={refreshing}>
            {refreshing ? t("installed.refreshing") : t("installed.refresh")}
          </button>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-4">
        <fieldset className="flex flex-wrap items-center gap-1 border-0 p-0">
          <legend className="sr-only">{t("installed.filterLabel")}</legend>
          {FILTERS.map((f) => (
            <label
              key={f}
              className={`cursor-pointer rounded-full border px-3 py-1 has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-2 has-[:focus-visible]:outline-[var(--focus)] ${
                filter === f
                  ? "border-accent bg-accent text-accent-contrast"
                  : "border-border bg-surface hover:bg-surface-2"
              }`}
            >
              <input
                type="radio"
                name="installed-filter"
                value={f}
                checked={filter === f}
                onChange={() => setFilter(f)}
                className="sr-only"
              />
              {t(`installed.filter.${f}` as const)} ({counts[f] ?? 0})
            </label>
          ))}
        </fieldset>
        <input
          type="search"
          aria-label={t("installed.searchLabel")}
          placeholder={t("installed.searchPlaceholder")}
          value={text}
          onChange={(e) => setText(e.target.value)}
          className="min-w-48 flex-1 rounded-ui-sm border border-border bg-surface px-3 py-1.5"
        />
        <label className="flex items-center gap-2">
          <span className="text-muted">{t("installed.sortLabel")}</span>
          <select
            value={sort}
            onChange={(e) => setSort(e.target.value as InstalledSort)}
            className="rounded-ui-sm border border-border bg-surface px-2 py-1"
          >
            {SORTS.map((s) => (
              <option key={s} value={s}>
                {t(`installed.sort.${s}` as const)}
              </option>
            ))}
          </select>
        </label>
      </div>

      {filter === "orphans" && data && !dbError && <OrphansBanner count={counts.orphans} />}
      {body}
    </section>
  );
}
