import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { AppRoutes } from "../App";
import { useQueue } from "../state/queue";
import { useUi } from "../state/ui";

export function resetStores() {
  useQueue.setState({ items: [] });
  useUi.setState({
    terminalOpen: false,
    query: "",
    sourceFilter: "all",
    installedOnly: false,
  });
}

export function renderApp(initialPath = "/search") {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={[initialPath]}>
        <AppRoutes />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}
