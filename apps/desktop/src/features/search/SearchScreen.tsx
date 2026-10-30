import { useVirtualizer } from "@tanstack/react-virtual";
import { useRef, useState } from "react";
import { useNavigate } from "react-router";
import type { PackageSummary } from "../../api";
import { t } from "../../i18n";
import { type SourceFilter, useUi } from "../../state/ui";
import { PackageRow } from "./PackageRow";
import { useSearch } from "./useSearch";

const ROW_HEIGHT = 84;

const SOURCES: {
  value: SourceFilter;
  label: "search.source.all" | "search.source.repo" | "search.source.aur";
}[] = [
  { value: "all", label: "search.source.all" },
  { value: "repo", label: "search.source.repo" },
  { value: "aur", label: "search.source.aur" },
];

export function packagePath(p: PackageSummary): string {
  return `/package/${p.source.kind === "repo" ? p.source.repo : "aur"}/${encodeURIComponent(p.name)}`;
}

function Filters() {
  const sourceFilter = useUi((s) => s.sourceFilter);
  const setSourceFilter = useUi((s) => s.setSourceFilter);
  const installedOnly = useUi((s) => s.installedOnly);
  const setInstalledOnly = useUi((s) => s.setInstalledOnly);
  return (
    <div className="flex flex-wrap items-center gap-4">
      <fieldset className="flex items-center gap-1 border-0 p-0">
        <legend className="sr-only">{t("search.filterSource")}</legend>
        <span aria-hidden className="mr-1 text-muted">
          {t("search.filterSource")}
        </span>
        {SOURCES.map((s) => (
          <label
            key={s.value}
            className={`cursor-pointer rounded-full border px-3 py-1 has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-2 has-[:focus-visible]:outline-[var(--focus)] ${
              sourceFilter === s.value
                ? "border-accent bg-accent text-accent-contrast"
                : "border-border bg-surface hover:bg-surface-2"
            }`}
          >
            <input
              type="radio"
              name="source-filter"
              value={s.value}
              checked={sourceFilter === s.value}
              onChange={() => setSourceFilter(s.value)}
              className="sr-only"
            />
            {t(s.label)}
          </label>
        ))}
      </fieldset>
      <label className="flex cursor-pointer items-center gap-2">
        <input
          type="checkbox"
          checked={installedOnly}
          onChange={(e) => setInstalledOnly(e.target.checked)}
        />
        {t("search.installedOnly")}
      </label>
    </div>
  );
}

function StateMessage({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children?: React.ReactNode;
}) {
  return (
    <div className="mx-auto mt-16 max-w-md text-center">
      <h2 className="text-lg font-semibold">{title}</h2>
      <p className="mt-1 text-muted">{description}</p>
      {children}
    </div>
  );
}

function ResultList({ items }: { items: PackageSummary[] }) {
  const navigate = useNavigate();
  const parentRef = useRef<HTMLDivElement>(null);
  const [active, setActive] = useState(0);
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 6,
  });
  const activeIndex = Math.min(active, items.length - 1);

  const focusRow = (i: number) => {
    setActive(i);
    virtualizer.scrollToIndex(i);
    requestAnimationFrame(() => {
      parentRef.current?.querySelector<HTMLElement>(`[data-open][data-row-index="${i}"]`)?.focus();
    });
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    const target = e.target as HTMLElement;
    if (!target.matches("[data-open]")) return;
    const map: Record<string, number> = {
      ArrowDown: activeIndex + 1,
      ArrowUp: activeIndex - 1,
      Home: 0,
      End: items.length - 1,
      PageDown: activeIndex + 8,
      PageUp: activeIndex - 8,
    };
    const next = map[e.key];
    if (next === undefined) return;
    e.preventDefault();
    focusRow(Math.max(0, Math.min(items.length - 1, next)));
  };

  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: key handler delegates arrow navigation to focusable row buttons
    <div
      ref={parentRef}
      onKeyDown={onKeyDown}
      className="min-h-0 flex-1 overflow-auto rounded-ui border border-border bg-surface"
    >
      <ul
        aria-label={t("search.results")}
        className="relative m-0 list-none p-0"
        style={{ height: virtualizer.getTotalSize() }}
      >
        {virtualizer.getVirtualItems().map((v) => {
          const pkg = items[v.index];
          if (!pkg) return null;
          return (
            <li
              key={`${pkg.source.kind}:${pkg.name}`}
              className="absolute left-0 top-0 w-full"
              style={{ height: v.size, transform: `translateY(${v.start}px)` }}
            >
              <PackageRow
                pkg={pkg}
                index={v.index}
                tabbable={v.index === activeIndex}
                onOpen={(p) => navigate(packagePath(p))}
              />
            </li>
          );
        })}
      </ul>
    </div>
  );
}

export function SearchScreen() {
  const rawQuery = useUi((s) => s.query);
  const { query, pending, items, isLoading, isError, refetch, aurError } = useSearch();
  const trimmed = rawQuery.trim();

  let body: React.ReactNode;
  if (trimmed === "") {
    body = (
      <StateMessage
        title={t("search.emptyInitial.title")}
        description={t("search.emptyInitial.desc")}
      />
    );
  } else if (isError) {
    body = (
      <StateMessage title={t("search.error.title")} description={t("search.error.desc")}>
        <button
          type="button"
          onClick={() => refetch()}
          className="mt-3 rounded-ui-sm bg-accent px-3 py-1.5 font-medium text-accent-contrast hover:bg-accent-hover"
        >
          {t("search.retry")}
        </button>
      </StateMessage>
    );
  } else if (isLoading || pending) {
    body = (
      <p role="status" className="mt-16 text-center text-muted">
        {t("search.loading")}
      </p>
    );
  } else if (items.length === 0) {
    body = (
      <StateMessage
        title={t("search.empty.title")}
        description={t("search.empty.desc", { query })}
      />
    );
  } else {
    body = <ResultList items={items} />;
  }

  return (
    <section className="flex h-full flex-col gap-3 p-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-2xl font-semibold">{t("search.title")}</h1>
        {items.length > 0 && !isLoading && (
          <span className="text-muted" aria-live="polite">
            {t("search.count", { count: items.length })}
          </span>
        )}
      </div>
      <Filters />
      {aurError && (
        <div role="alert" className="rounded-ui-sm bg-warning-bg px-3 py-2 text-warning">
          {t("search.aurError", { message: aurError })}
        </div>
      )}
      {trimmed.length === 1 && <p className="text-muted">{t("search.aurShort")}</p>}
      {body}
    </section>
  );
}
