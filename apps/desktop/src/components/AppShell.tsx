import { Outlet } from "react-router";
import { Header } from "./Header";
import { Sidebar } from "./Sidebar";
import { TerminalDrawer } from "./TerminalDrawer";
import { useGlobalShortcuts } from "./useGlobalShortcuts";

export function AppShell() {
  useGlobalShortcuts();
  return (
    <div className="flex h-full">
      <Sidebar />
      <div className="flex min-w-0 flex-1 flex-col">
        <Header />
        <main className="min-h-0 flex-1 overflow-auto">
          <Outlet />
        </main>
        <TerminalDrawer />
      </div>
    </div>
  );
}
