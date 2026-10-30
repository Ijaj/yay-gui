import { LogView } from "../features/transaction/TransactionScreen";
import { t } from "../i18n";
import { useTransaction } from "../state/transaction";
import { useUi } from "../state/ui";

/** Plain-text transaction output; xterm.js and direct keyboard input come later. */
export function TerminalDrawer() {
  const open = useUi((s) => s.terminalOpen);
  const setOpen = useUi((s) => s.setTerminalOpen);
  const ran = useTransaction((s) => s.state !== null);
  if (!open) return null;
  return (
    <section
      aria-label={t("terminal.title")}
      className="flex h-56 shrink-0 flex-col border-t border-border bg-surface-2"
    >
      <div className="flex items-center justify-between border-b border-border px-4 py-1.5">
        <h2 className="text-sm font-semibold">{t("terminal.title")}</h2>
        <button
          type="button"
          onClick={() => setOpen(false)}
          className="rounded-ui-sm px-2 py-0.5 text-muted hover:bg-surface"
        >
          {t("terminal.close")}
        </button>
      </div>
      {ran ? (
        <LogView className="min-h-0 flex-1" />
      ) : (
        <p className="p-4 font-mono text-muted">{t("terminal.idle")}</p>
      )}
    </section>
  );
}
