import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router";
import { api, type TransactionState } from "../../api";
import { t } from "../../i18n";
import { isActive, useTransaction } from "../../state/transaction";
import { formatDate } from "../search/formatters";
import { commandLine, renderLog } from "./log";
import { PromptPanel } from "./PromptPanel";

const BUTTON =
  "rounded-ui-sm border border-border bg-surface px-3 py-1.5 font-medium hover:bg-surface-2";

const STATE_STYLE: Record<TransactionState["state"], string> = {
  planned: "bg-surface-2 text-fg",
  reviewing: "bg-surface-2 text-fg",
  approved: "bg-surface-2 text-fg",
  running: "bg-accent text-accent-contrast",
  succeeded: "bg-success-bg text-success",
  failed: "bg-danger-bg text-danger",
  cancelled: "bg-warning-bg text-warning",
};

export function StateBadge({ state }: { state: TransactionState }) {
  return (
    <span className={`rounded-full px-2 py-0.5 text-xs font-semibold ${STATE_STYLE[state.state]}`}>
      {t(`tx.state.${state.state}`)}
    </span>
  );
}

function LastTransaction() {
  const startError = useTransaction((s) => s.startError);
  const { data } = useQuery({ queryKey: ["last-transaction"], queryFn: api.lastTransaction });
  return (
    <div className="mt-4 flex flex-col gap-2">
      {startError && (
        <p role="alert" className="text-danger">
          {t("queue.startFailed", { message: startError })}
        </p>
      )}
      {data ? (
        <p>
          {t("tx.last", {
            state: t(`tx.state.${data.state.state}`),
            date: formatDate(data.startedAt),
          })}
        </p>
      ) : (
        <p className="text-muted">{t("tx.none")}</p>
      )}
      {data?.interrupted && (
        <p role="alert" className="rounded-ui border border-warning bg-warning-bg p-3 text-warning">
          {t("tx.interrupted")}
        </p>
      )}
    </div>
  );
}

function Result({ state, onRetry }: { state: TransactionState; onRetry: () => void }) {
  const errors = useTransaction((s) => s.errors);
  const log = useTransaction((s) => s.log);
  const navigate = useNavigate();
  const [copied, setCopied] = useState(false);
  const hints = [...new Set(errors.map((e) => e.errorKind))];
  const copy = async () => {
    await navigator.clipboard?.writeText(renderLog(log));
    setCopied(true);
  };
  return (
    <div
      role="status"
      className={`rounded-ui border p-4 ${
        state.state === "succeeded" ? "border-success bg-success-bg" : "border-danger bg-danger-bg"
      }`}
    >
      <p className="m-0 font-semibold">
        {state.state === "succeeded"
          ? t("tx.succeeded")
          : state.state === "failed"
            ? t("tx.failed", { reason: state.reason })
            : t("tx.cancelled")}
      </p>
      {state.state !== "succeeded" && hints.length > 0 && (
        <ul className="mt-2">
          {hints.map((h) => (
            <li key={h}>{t(`tx.hint.${h}`)}</li>
          ))}
        </ul>
      )}
      <div className="mt-3 flex flex-wrap gap-2">
        {state.state === "succeeded" ? (
          <button type="button" className={BUTTON} onClick={() => navigate("/search")}>
            {t("tx.backToSearch")}
          </button>
        ) : (
          <>
            <button type="button" className={BUTTON} onClick={onRetry}>
              {t("tx.retry")}
            </button>
            <button type="button" className={BUTTON} onClick={() => navigate("/queue")}>
              {t("tx.backToQueue")}
            </button>
          </>
        )}
        <button type="button" className={BUTTON} onClick={copy}>
          {copied ? t("tx.copied") : t("tx.copyLog")}
        </button>
      </div>
    </div>
  );
}

export function LogView({ className = "" }: { className?: string }) {
  const log = useTransaction((s) => s.log);
  const text = useMemo(() => renderLog(log), [log]);
  const ref = useRef<HTMLPreElement>(null);
  // biome-ignore lint/correctness/useExhaustiveDependencies: scroll whenever the text grows
  useEffect(() => {
    const el = ref.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [text]);
  return (
    <pre
      ref={ref}
      role="log"
      aria-label={t("tx.output")}
      className={`m-0 overflow-auto whitespace-pre-wrap break-all bg-surface-2 p-3 font-mono text-sm ${className}`}
    >
      {text || t("terminal.empty")}
    </pre>
  );
}

export function TransactionScreen() {
  const state = useTransaction((s) => s.state);
  const steps = useTransaction((s) => s.steps);
  const prompt = useTransaction((s) => s.prompt);
  const request = useTransaction((s) => s.request);
  const start = useTransaction((s) => s.start);
  const cancel = useTransaction((s) => s.cancel);
  const step = steps.at(-1);
  const active = isActive(state);

  return (
    <section className="mx-auto flex max-w-5xl flex-col gap-4 p-6">
      <div className="flex flex-wrap items-center gap-3">
        <h1 className="text-2xl font-semibold">{t("tx.title")}</h1>
        {state && <StateBadge state={state} />}
        {active && (
          <button
            type="button"
            className={`ml-auto ${BUTTON}`}
            title={t("tx.cancelHint")}
            onClick={() => cancel()}
          >
            {t("tx.cancel")}
          </button>
        )}
      </div>
      {!state && <LastTransaction />}
      {step && (
        <div>
          <p className="m-0 text-muted">
            {t("tx.step", { step: step.index + 1, total: step.total })}
          </p>
          <p className="m-0 font-medium">{step.description}</p>
          <code className="mt-1 block overflow-x-auto rounded-ui-sm bg-surface-2 px-2 py-1 font-mono text-sm">
            {commandLine(step.argv)}
          </code>
        </div>
      )}
      {prompt && active && <PromptPanel prompt={prompt} />}
      {state && !active && (
        <Result state={state} onRetry={() => request && start(request.items, request.skipReview)} />
      )}
      {state && <LogView className="h-96" />}
    </section>
  );
}
