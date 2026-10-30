import { t } from "../../i18n";
import { useQueue } from "../../state/queue";

export function QueueIndicator() {
  const count = useQueue((s) => s.items.length);
  return (
    <button
      type="button"
      aria-label={t("header.queueLabel", { count })}
      className="inline-flex items-center gap-2 rounded-ui-sm border border-border bg-surface px-3 py-1.5 font-medium hover:bg-surface-2"
    >
      {t("header.queue")}
      <span
        data-testid="queue-count"
        className="min-w-6 rounded-full bg-accent px-1.5 text-center text-xs font-bold text-accent-contrast"
      >
        {count}
      </span>
    </button>
  );
}
