import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { api, type PackageSummary } from "../../api";
import { useUi } from "../../state/ui";

export const SEARCH_DEBOUNCE_MS = 200;

export function useDebounced<T>(value: T, ms: number): T {
  const [v, setV] = useState(value);
  useEffect(() => {
    const id = setTimeout(() => setV(value), ms);
    return () => clearTimeout(id);
  }, [value, ms]);
  return v;
}

export function useSearch() {
  const rawQuery = useUi((s) => s.query);
  const sourceFilter = useUi((s) => s.sourceFilter);
  const installedOnly = useUi((s) => s.installedOnly);
  const query = useDebounced(rawQuery.trim(), SEARCH_DEBOUNCE_MS);

  const result = useQuery({
    queryKey: ["search", query],
    queryFn: () => api.searchPackages(query),
    enabled: query.length >= 1,
    placeholderData: keepPreviousData,
    staleTime: 30_000,
    retry: false,
  });

  const items = useMemo<PackageSummary[]>(() => {
    const data = result.data;
    if (!data) return [];
    const merged = [...data.repo, ...data.aur];
    return merged.filter((p) => {
      if (sourceFilter === "repo" && p.source.kind !== "repo") return false;
      if (sourceFilter === "aur" && p.source.kind !== "aur") return false;
      if (installedOnly && !p.installed) return false;
      return true;
    });
  }, [result.data, sourceFilter, installedOnly]);

  return {
    query,
    pending: rawQuery.trim() !== query,
    items,
    isLoading: result.isLoading || (result.isPlaceholderData && result.isFetching),
    isError: result.isError,
    refetch: result.refetch,
    aurError: result.data?.aurError,
  };
}
