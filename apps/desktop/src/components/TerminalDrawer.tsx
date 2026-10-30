import { t } from "../i18n";
import { useUi } from "../state/ui";

/** Placeholder; xterm.js and live transaction output come later. */
export function TerminalDrawer() {
  const open = useUi((s) => s.terminalOpen);
  const setOpen = useUi((s) => s.setTerminalOpen);
  if (!open) return null;
  return (
    <section
      aria-label={t("terminal.title")}
      className="h-56 shrink-0 border-t border-border bg-surface-2"
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
      <p className="p-4 font-mono text-muted">{t("terminal.idle")}</p>
    </section>
  );
}
