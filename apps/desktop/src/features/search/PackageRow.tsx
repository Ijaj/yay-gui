import type { PackageSummary } from "../../api";
import { SourceBadge } from "../../components/SourceBadge";
import { t } from "../../i18n";
import { packageKey, useQueue } from "../../state/queue";
import { formatPopularity } from "./formatters";

export function PackageRow({
  pkg,
  onOpen,
  tabbable,
  index,
}: {
  pkg: PackageSummary;
  onOpen: (pkg: PackageSummary) => void;
  tabbable: boolean;
  index: number;
}) {
  const key = packageKey(pkg);
  const queued = useQueue((s) => s.items.some((i) => i.key === key));
  const add = useQueue((s) => s.add);
  const remove = useQueue((s) => s.remove);
  const isAur = pkg.source.kind === "aur";
  const orphaned = isAur && pkg.maintainer === null;
  const flagged = pkg.outOfDate != null;

  return (
    <div className="flex h-full items-center gap-3 border-b border-border px-4">
      <button
        type="button"
        data-open
        data-row-index={index}
        tabIndex={tabbable ? 0 : -1}
        onClick={() => onOpen(pkg)}
        className="flex min-w-0 flex-1 flex-col items-start gap-0.5 rounded-ui-sm py-2 text-left"
      >
        <span className="flex w-full items-center gap-2">
          <span className="truncate font-semibold">{pkg.name}</span>
          <span className="text-muted">{pkg.version}</span>
          <SourceBadge source={pkg.source} />
          {pkg.installed && (
            <span className="rounded-full bg-success-bg px-2 py-0.5 text-xs font-semibold text-success">
              {t("pkg.installed")}
            </span>
          )}
          {flagged && (
            <span className="rounded-full bg-warning-bg px-2 py-0.5 text-xs font-semibold text-warning">
              {t("pkg.outOfDate")}
            </span>
          )}
          {orphaned && (
            <span className="rounded-full bg-danger-bg px-2 py-0.5 text-xs font-semibold text-danger">
              {t("pkg.orphaned")}
            </span>
          )}
        </span>
        <span className="w-full truncate text-muted">{pkg.description}</span>
        {isAur && (
          <span className="text-xs text-muted">
            {t("pkg.votes", { count: pkg.votes ?? 0 })}
            {" · "}
            {t("pkg.popularity", { value: formatPopularity(pkg.popularity ?? 0) })}
          </span>
        )}
      </button>
      <button
        type="button"
        aria-label={`${queued ? t("pkg.removeFromQueue") : t("pkg.addToQueue")}: ${pkg.name}`}
        aria-pressed={queued}
        tabIndex={tabbable ? 0 : -1}
        onClick={() => (queued ? remove(key) : add(pkg))}
        className={`shrink-0 rounded-ui-sm px-3 py-1.5 font-medium ${
          queued
            ? "border border-border bg-surface hover:bg-surface-2"
            : "bg-accent text-accent-contrast hover:bg-accent-hover"
        }`}
      >
        {queued ? t("pkg.inQueue") : t("pkg.addToQueue")}
      </button>
    </div>
  );
}
