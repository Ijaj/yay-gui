import { useState } from "react";
import { Link, useNavigate } from "react-router";
import type { PlanResponse } from "../../api";
import { ReviewStatusBadge } from "../../components/StatusBadge";
import { t } from "../../i18n";
import { UPGRADE_KEY, useQueue } from "../../state/queue";
import { isActive, useTransaction } from "../../state/transaction";
import { commandLine } from "../transaction/log";
import { usePlan } from "./usePlan";

const BUTTON =
  "rounded-ui-sm border border-border bg-surface px-3 py-1.5 font-medium hover:bg-surface-2";
const PRIMARY =
  "rounded-ui-sm bg-accent px-4 py-1.5 font-semibold text-accent-contrast hover:bg-accent-hover disabled:cursor-not-allowed disabled:opacity-50";

function UpgradeToggle() {
  const upgrade = useQueue((s) => s.items.find((i) => i.key === UPGRADE_KEY));
  const setUpgrade = useQueue((s) => s.setUpgrade);
  const devel = upgrade?.action === "system_upgrade" && upgrade.devel;
  return (
    <div className="rounded-ui border border-border bg-surface p-3">
      <label className="flex cursor-pointer items-center gap-2 font-medium">
        <input
          type="checkbox"
          checked={upgrade !== undefined}
          onChange={(e) => setUpgrade(e.target.checked, devel)}
        />
        {t("queue.upgrade")}
      </label>
      <p className="ml-6 mt-1 text-muted">{t("queue.upgradeHint")}</p>
      {upgrade && (
        <label className="ml-6 mt-2 flex cursor-pointer items-center gap-2">
          <input
            type="checkbox"
            checked={devel}
            onChange={(e) => setUpgrade(true, e.target.checked)}
          />
          {t("queue.devel")}
        </label>
      )}
    </div>
  );
}

function QueueList() {
  const all = useQueue((s) => s.items);
  const items = all.filter((i) => i.action === "install");
  const remove = useQueue((s) => s.remove);
  if (items.length === 0) return null;
  return (
    <ul aria-label={t("queue.items")} className="m-0 list-none divide-y divide-border p-0">
      {items.map(
        (i) =>
          i.action === "install" && (
            <li key={i.key} className="flex items-center gap-3 py-2">
              <span className="text-muted">{t("queue.action.install")}</span>
              <span className="font-semibold">{i.name}</span>
              <span className="text-muted">{i.version}</span>
              <span className="text-xs text-muted">{i.sourceLabel}</span>
              <button
                type="button"
                className={`ml-auto ${BUTTON}`}
                aria-label={t("queue.remove", { name: i.name })}
                onClick={() => remove(i.key)}
              >
                ✕
              </button>
            </li>
          ),
      )}
    </ul>
  );
}

function PlanError({ plan }: { plan: PlanResponse }) {
  const setUpgrade = useQueue((s) => s.setUpgrade);
  const e = plan.error;
  if (!e) return null;
  const text =
    e.kind === "partial_upgrade"
      ? t("queue.error.partial_upgrade")
      : e.kind === "unreviewed"
        ? t("queue.error.unreviewed")
        : e.message;
  return (
    <div role="alert" className="rounded-ui border border-danger bg-danger-bg p-3 text-danger">
      <p className="m-0">{text}</p>
      {e.kind === "partial_upgrade" && (
        <button type="button" className={`mt-2 ${BUTTON}`} onClick={() => setUpgrade(true)}>
          {t("queue.fix.addUpgrade")}
        </button>
      )}
    </div>
  );
}

function Reviews({ plan }: { plan: PlanResponse }) {
  if (plan.reviews.length === 0) return null;
  return (
    <section aria-labelledby="reviews-heading" className="mt-6">
      <h2 id="reviews-heading" className="text-lg font-semibold">
        {t("queue.reviews")}
      </h2>
      <p className="mt-1 text-muted">{t("queue.reviewsHint")}</p>
      <ul className="m-0 mt-2 list-none divide-y divide-border p-0">
        {plan.reviews.map((r) => (
          <li key={r.name} className="flex flex-wrap items-center gap-3 py-2">
            <span className="font-semibold">{r.name}</span>
            <span className="text-muted">{r.version}</span>
            {r.dependencyOf && (
              <span className="text-xs text-muted">
                {t("queue.dependencyOf", { name: r.dependencyOf })}
              </span>
            )}
            <ReviewStatusBadge status={r.status} />
            <Link
              to={`/review/${encodeURIComponent(r.name)}`}
              className={`ml-auto ${r.status === "approved" ? BUTTON : PRIMARY}`}
            >
              {r.status === "approved"
                ? t("queue.reviewAgain", { name: r.name })
                : t("queue.review", { name: r.name })}
            </Link>
          </li>
        ))}
      </ul>
    </section>
  );
}

function SkipReview() {
  const skip = useQueue((s) => s.skipReview);
  const setSkip = useQueue((s) => s.setSkipReview);
  return (
    <div className="mt-6">
      <label className="flex cursor-pointer items-center gap-2">
        <input type="checkbox" checked={skip} onChange={(e) => setSkip(e.target.checked)} />
        {t("queue.skipReview")}
      </label>
      {skip && <p className="ml-6 mt-1 font-medium text-danger">{t("queue.skipReviewWarning")}</p>}
    </div>
  );
}

function Summary() {
  const { data: plan, isLoading, isError, error, refetch, request, skipReview } = usePlan();
  const start = useTransaction((s) => s.start);
  const active = useTransaction((s) => isActive(s.state));
  const navigate = useNavigate();
  const [startError, setStartError] = useState<string | null>(null);

  if (request.length === 0) return null;
  if (isLoading) return <p className="mt-6 text-muted">{t("queue.planning")}</p>;
  if (isError || !plan) {
    return (
      <p role="alert" className="mt-6 text-danger">
        {t("queue.planFailed", { message: String(error) })}
      </p>
    );
  }

  const run = async () => {
    setStartError(null);
    if (await start(request, skipReview)) navigate("/transaction");
    else setStartError(useTransaction.getState().startError);
  };

  return (
    <section aria-labelledby="summary-heading" className="mt-8">
      <h2 id="summary-heading" className="text-lg font-semibold">
        {t("queue.summary")}
      </h2>
      <div className="mt-3 flex flex-col gap-3">
        {plan.lockHeld && (
          <div
            role="alert"
            className="rounded-ui border border-warning bg-warning-bg p-3 text-warning"
          >
            <p className="m-0">{t("queue.lockHeld")}</p>
            <button type="button" className={`mt-2 ${BUTTON}`} onClick={() => refetch()}>
              {t("queue.recheck")}
            </button>
          </div>
        )}
        <PlanError plan={plan} />
      </div>
      <h3 className="mt-4 font-semibold">{t("queue.commands")}</h3>
      <p className="mt-1 text-muted">{t("queue.commandsHint")}</p>
      {plan.commands.length === 0 ? (
        <p className="text-muted">{t("queue.noCommands")}</p>
      ) : (
        <ol className="mt-2 flex flex-col gap-2 pl-5">
          {plan.commands.map((c) => (
            <li key={c.argv.join(" ")}>
              <div>{c.description}</div>
              <code className="mt-1 block overflow-x-auto rounded-ui-sm bg-surface-2 px-2 py-1 font-mono text-sm">
                {commandLine(c.argv)}
              </code>
            </li>
          ))}
        </ol>
      )}
      <Reviews plan={plan} />
      <SkipReview />
      <div className="mt-6 flex items-center gap-3">
        <button type="button" className={PRIMARY} disabled={!plan.canRun || active} onClick={run}>
          {t("queue.run")}
        </button>
        <span className="text-muted">{t("queue.runHint")}</span>
      </div>
      {startError && (
        <p role="alert" className="mt-2 text-danger">
          {t("queue.startFailed", { message: startError })}
        </p>
      )}
    </section>
  );
}

export function QueueScreen() {
  const count = useQueue((s) => s.items.length);
  return (
    <section className="mx-auto max-w-4xl p-6">
      <h1 className="text-2xl font-semibold">{t("queue.title")}</h1>
      {count === 0 && (
        <div className="mt-4">
          <p className="font-medium">{t("queue.empty.title")}</p>
          <p className="text-muted">{t("queue.empty.desc")}</p>
          <Link to="/search" className="mt-2 inline-block text-accent">
            {t("queue.goSearch")}
          </Link>
        </div>
      )}
      <div className="mt-4 flex flex-col gap-4">
        <UpgradeToggle />
        <QueueList />
      </div>
      <Summary />
    </section>
  );
}
