import { NavLink } from "react-router";
import { type MessageKey, t } from "../i18n";

const ITEMS: { to: string; key: MessageKey }[] = [
  { to: "/", key: "nav.home" },
  { to: "/search", key: "nav.search" },
  { to: "/updates", key: "nav.updates" },
  { to: "/installed", key: "nav.installed" },
  { to: "/maintenance", key: "nav.maintenance" },
  { to: "/settings", key: "nav.settings" },
];

export function Sidebar() {
  return (
    <nav
      aria-label={t("nav.label")}
      className="flex w-52 shrink-0 flex-col gap-1 border-r border-border bg-sidebar p-3"
    >
      <div className="px-2 pb-3 pt-1 text-lg font-bold">{t("app.name")}</div>
      {ITEMS.map((item) => (
        <NavLink
          key={item.to}
          to={item.to}
          end={item.to === "/"}
          className={({ isActive }) =>
            `rounded-ui-sm px-3 py-2 font-medium hover:bg-surface-2 ${
              isActive ? "bg-surface-2 text-accent" : "text-fg"
            }`
          }
        >
          {t(item.key)}
        </NavLink>
      ))}
    </nav>
  );
}
