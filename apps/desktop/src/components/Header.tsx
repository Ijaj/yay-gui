import { useLocation, useNavigate } from "react-router";
import { QueueIndicator } from "../features/queue/QueueIndicator";
import { t } from "../i18n";
import { useUi } from "../state/ui";

export const GLOBAL_SEARCH_ID = "global-search";

export function Header() {
  const query = useUi((s) => s.query);
  const setQuery = useUi((s) => s.setQuery);
  const terminalOpen = useUi((s) => s.terminalOpen);
  const toggleTerminal = useUi((s) => s.toggleTerminal);
  const navigate = useNavigate();
  const { pathname } = useLocation();

  return (
    <header className="flex items-center gap-3 border-b border-border bg-surface px-4 py-2">
      <div className="flex-1">
        <label htmlFor={GLOBAL_SEARCH_ID} className="sr-only">
          {t("header.searchLabel")}
        </label>
        <input
          id={GLOBAL_SEARCH_ID}
          type="search"
          value={query}
          autoComplete="off"
          spellCheck={false}
          placeholder={t("header.searchPlaceholder")}
          onChange={(e) => {
            setQuery(e.target.value);
            if (!pathname.startsWith("/search") && !pathname.startsWith("/package")) {
              navigate("/search");
            }
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") navigate("/search");
          }}
          className="w-full max-w-xl rounded-ui-sm border border-border bg-bg px-3 py-1.5 placeholder:text-muted"
        />
      </div>
      <QueueIndicator />
      <button
        type="button"
        aria-pressed={terminalOpen}
        onClick={toggleTerminal}
        className="rounded-ui-sm border border-border bg-surface px-3 py-1.5 font-medium hover:bg-surface-2"
      >
        {t("header.terminal")} <kbd className="text-xs text-muted">Ctrl+J</kbd>
      </button>
    </header>
  );
}
