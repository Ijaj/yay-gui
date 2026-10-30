import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useNavigate, useParams } from "react-router";
import { api, type RepoFile, type ReviewBundle } from "../../api";
import { CodeViewer, languageForPath } from "../../components/CodeViewer";
import { ReviewStatusBadge } from "../../components/StatusBadge";
import { t } from "../../i18n";
import { useQueue } from "../../state/queue";
import { PLAN_KEY } from "../queue/usePlan";
import { formatDate, formatPopularity } from "../search/formatters";

const BUTTON =
  "rounded-ui-sm border border-border bg-surface px-3 py-1.5 font-medium hover:bg-surface-2";
const PRIMARY =
  "rounded-ui-sm bg-accent px-4 py-1.5 font-semibold text-accent-contrast hover:bg-accent-hover disabled:opacity-50";

const short = (c: string) => c.slice(0, 10);

type Tab = { id: string; label: string; file?: RepoFile };

function FileBody({ file }: { file: RepoFile }) {
  if (file.symlink) {
    return <p className="text-warning">{t("review.symlink", { target: file.content ?? "?" })}</p>;
  }
  if (file.content === null)
    return <p className="text-muted">{t("review.binary", { size: file.size })}</p>;
  return <CodeViewer text={file.content} language={languageForPath(file.path)} label={file.path} />;
}

function Meta({ b }: { b: ReviewBundle }) {
  const rows: [string, React.ReactNode][] = [
    [t("pkg.field.maintainer"), b.maintainer ?? t("pkg.orphaned")],
    [t("review.submitter"), b.submitter ?? "—"],
    [
      t("pkg.votes", { count: b.votes }),
      t("pkg.popularity", { value: formatPopularity(b.popularity) }),
    ],
    [t("review.firstSubmitted"), formatDate(b.firstSubmitted)],
    [t("pkg.field.lastModified"), formatDate(b.lastModified)],
    [t("review.commit"), <code key="c">{b.commit}</code>],
  ];
  if (b.outOfDate !== null) rows.push([t("pkg.field.outOfDate"), formatDate(b.outOfDate)]);
  if (b.url) rows.push([t("review.upstream"), b.url]);
  return (
    <dl className="m-0 mt-4 grid grid-cols-[10rem_1fr] gap-x-2 gap-y-1">
      {rows.map(([k, v]) => (
        <div key={k} className="contents">
          <dt className="text-muted">{k}</dt>
          <dd className="m-0 break-all">{v}</dd>
        </div>
      ))}
    </dl>
  );
}

function ReviewBody({ b }: { b: ReviewBundle }) {
  const tabs: Tab[] = [
    ...(b.diff !== undefined ? [{ id: "diff", label: t("review.diffTab") }] : []),
    ...b.files.map((f) => ({ id: `f:${f.path}`, label: f.path, file: f })),
  ];
  const [active, setActive] = useState(tabs[0]?.id ?? "");
  const current = tabs.find((x) => x.id === active) ?? tabs[0];
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const queued = useQueue((s) => s.items.find((i) => i.action === "install" && i.name === b.name));
  const remove = useQueue((s) => s.remove);
  const approve = useMutation({
    mutationFn: () => api.approveReview(b.pkgbase, b.commit),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: [PLAN_KEY] });
      await queryClient.invalidateQueries({ queryKey: ["review", b.name] });
      navigate("/queue");
    },
  });

  return (
    <>
      <div className="flex flex-wrap items-center gap-3">
        <h1 className="text-2xl font-semibold">{t("review.title", { name: b.name })}</h1>
        <span className="text-muted">{b.version}</span>
        <ReviewStatusBadge status={b.status} />
      </div>
      <p className="mt-3 rounded-ui border border-warning bg-warning-bg p-3 text-warning">
        {t("review.warning")}
      </p>
      {b.previous && (
        <p className="mt-3">
          {t(
            b.diff !== undefined || b.status === "approved"
              ? "review.previous"
              : "review.previousGone",
            {
              commit: short(b.previous.commit),
              date: formatDate(b.previous.approvedAt),
            },
          )}
        </p>
      )}
      <Meta b={b} />

      <div
        role="tablist"
        aria-label={t("review.files")}
        className="mt-6 flex flex-wrap gap-1 border-b border-border"
      >
        {tabs.map((tab) => (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={tab.id === current?.id}
            onClick={() => setActive(tab.id)}
            className={`rounded-t-ui-sm px-3 py-1.5 font-mono text-sm ${
              tab.id === current?.id
                ? "bg-surface-2 font-semibold text-accent"
                : "hover:bg-surface-2"
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>
      <div role="tabpanel" aria-label={current?.label} className="mt-3">
        {current?.file ? (
          <FileBody file={current.file} />
        ) : (
          <CodeViewer text={b.diff ?? ""} language="diff" label={t("review.diffTab")} />
        )}
      </div>
      {b.truncated && <p className="mt-2 text-warning">{t("review.truncated")}</p>}

      <div className="mt-6 flex flex-wrap items-center gap-3">
        {b.status === "approved" ? (
          <span className="font-medium text-success">{t("review.approved")}</span>
        ) : (
          <button
            type="button"
            className={PRIMARY}
            disabled={approve.isPending}
            onClick={() => approve.mutate()}
          >
            {t("review.approve", { commit: short(b.commit) })}
          </button>
        )}
        {queued && (
          <button
            type="button"
            className={BUTTON}
            onClick={() => {
              remove(queued.key);
              navigate("/queue");
            }}
          >
            {t("review.removeFromQueue")}
          </button>
        )}
      </div>
      <p className="mt-2 text-muted">{t("review.approveHint")}</p>
      {approve.isError && (
        <p role="alert" className="mt-2 text-danger">
          {t("review.approveFailed", { message: String(approve.error) })}
        </p>
      )}
    </>
  );
}

export function ReviewScreen() {
  const { name = "" } = useParams();
  const navigate = useNavigate();
  const q = useQuery({
    queryKey: ["review", name],
    queryFn: () => api.getReview(name),
    retry: false,
    staleTime: 0,
  });
  return (
    <section className="mx-auto max-w-5xl p-6">
      <button
        type="button"
        onClick={() => navigate("/queue")}
        className="mb-4 rounded-ui-sm px-2 py-1 text-accent hover:bg-surface-2"
      >
        {t("review.back")}
      </button>
      {q.isLoading && <p className="text-muted">{t("review.loading")}</p>}
      {q.isError && (
        <div role="alert" className="text-danger">
          <p>{t("review.failed", { message: String(q.error) })}</p>
          <button type="button" className={BUTTON} onClick={() => q.refetch()}>
            {t("review.retry")}
          </button>
        </div>
      )}
      {q.data && <ReviewBody key={q.data.commit} b={q.data} />}
    </section>
  );
}
