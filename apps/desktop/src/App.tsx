import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { HashRouter, Route, Routes } from "react-router";
import { AppShell } from "./components/AppShell";
import {
  HomeScreen,
  InstalledScreen,
  MaintenanceScreen,
  SettingsScreen,
  UpdatesScreen,
} from "./features/placeholders/screens";
import { PackageDetail } from "./features/search/PackageDetail";
import { SearchScreen } from "./features/search/SearchScreen";

export function AppRoutes() {
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route index element={<HomeScreen />} />
        <Route path="search" element={<SearchScreen />} />
        <Route path="package/:source/:name" element={<PackageDetail />} />
        <Route path="updates" element={<UpdatesScreen />} />
        <Route path="installed" element={<InstalledScreen />} />
        <Route path="maintenance" element={<MaintenanceScreen />} />
        <Route path="settings" element={<SettingsScreen />} />
      </Route>
    </Routes>
  );
}

export function createQueryClient() {
  return new QueryClient({ defaultOptions: { queries: { refetchOnWindowFocus: false } } });
}

export function App() {
  return (
    <QueryClientProvider client={createQueryClient()}>
      <HashRouter>
        <AppRoutes />
      </HashRouter>
    </QueryClientProvider>
  );
}
