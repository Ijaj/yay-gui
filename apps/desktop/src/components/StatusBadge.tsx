import type { ReviewState } from "../api";
import { t } from "../i18n";

const STYLE: Record<ReviewState, string> = {
  not_reviewed: "bg-warning-bg text-warning",
  approved: "bg-success-bg text-success",
  changed_since_review: "bg-danger-bg text-danger",
};

export function ReviewStatusBadge({ status }: { status: ReviewState }) {
  return (
    <span className={`rounded-full px-2 py-0.5 text-xs font-semibold ${STYLE[status]}`}>
      {t(`queue.status.${status}`)}
    </span>
  );
}
