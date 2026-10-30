import { useNavigate, useParams } from "react-router";
import type { PackageSummary } from "../../api";
import { SourceBadge } from "../../components/SourceBadge";
import { t } from "../../i18n";
import { packageKey, useQueue } from "../../state/queue";
import { formatDate, formatPopularity } from "./formatters";
import { useSearch } from "./useSearch";

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[10rem_1fr] gap-2 border-b border-border py-2">
      <dt className="text-muted">{label}</dt>
      <dd className="m-0">{children}</dd>
    </div>
  );
}

function DetailBody({ pkg }: { pkg: PackageSummary }) {
  const key = packageKey(pkg);
  const queued = useQueue((s) => s.items.some((i) => i.key === key));
  const add = useQueue((s) => s.add);
  const remove = useQueue((s) => s.remove);
  const isAur = pkg.source.kind === "aur";
  return (
    <>
      <div className="flex flex-wrap items-center gap-3">
        <h1 className="text-2xl font-semibold">{pkg.name}</h1>
        <SourceBadge source={pkg.source} />
        {pkg.installed && (
          <span className="rounded-full bg-success-bg px-2 py-0.5 text-xs font-semibold text-success">
            {pkg.installedVersion
              ? t("pkg.installedVersion", { version: pkg.installedVersion })
              : t("pkg.installed")}
          </span>
        )}
        <button
          type="button"
          onClick={() => (queued ? remove(key) : add(pkg))}
          aria-pressed={queued}
          className={`ml-auto rounded-ui-sm px-3 py-1.5 font-medium ${
            queued
              ? "border border-border bg-surface hover:bg-surface-2"
              : "bg-accent text-accent-contrast hover:bg-accent-hover"
          }`}
        >
          {queued ? t("pkg.removeFromQueue") : t("pkg.addToQueue")}
        </button>
      </div>
      <dl className="m-0 mt-4">
        <Field label={t("pkg.field.version")}>{pkg.version}</Field>
        <Field label={t("pkg.field.description")}>{pkg.description}</Field>
        <Field label={t("pkg.field.source")}>
          {pkg.source.kind === "repo" ? pkg.source.repo : t("pkg.source.aur")}
        </Field>
        {isAur && (
          <>
            <Field label={t("pkg.field.maintainer")}>{pkg.maintainer ?? t("pkg.orphaned")}</Field>
            <Field label={t("pkg.votes", { count: pkg.votes ?? 0 })}>
              {t("pkg.popularity", { value: formatPopularity(pkg.popularity ?? 0) })}
            </Field>
            {pkg.lastModified !== undefined && (
              <Field label={t("pkg.field.lastModified")}>{formatDate(pkg.lastModified)}</Field>
            )}
            {pkg.outOfDate != null && (
              <Field label={t("pkg.field.outOfDate")}>{formatDate(pkg.outOfDate)}</Field>
            )}
          </>
        )}
      </dl>
      <p className="mt-6 rounded-ui border border-dashed border-border p-4 text-muted">
        {t("pkg.tabs.coming")}
      </p>
    </>
  );
}

export function PackageDetail() {
  const { source, name } = useParams();
  const navigate = useNavigate();
  const { items, isLoading } = useSearch();
  const pkg = items.find(
    (p) => p.name === name && (p.source.kind === "repo" ? p.source.repo : "aur") === source,
  );
  return (
    <section className="p-6">
      <button
        type="button"
        onClick={() => navigate("/search")}
        className="mb-4 rounded-ui-sm px-2 py-1 text-accent hover:bg-surface-2"
      >
        {t("pkg.back")}
      </button>
      {pkg ? (
        <DetailBody pkg={pkg} />
      ) : (
        <p className="text-muted">{isLoading ? t("search.loading") : t("pkg.notFound")}</p>
      )}
    </section>
  );
}
