import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { api } from "../../api";
import { toRequest, useQueue } from "../../state/queue";

export const PLAN_KEY = "plan";

/** Plans the current queue; re-runs whenever the queue or the review opt-in changes. */
export function usePlan() {
  const items = useQueue((s) => s.items);
  const skipReview = useQueue((s) => s.skipReview);
  const request = useMemo(() => toRequest(items), [items]);
  const query = useQuery({
    queryKey: [PLAN_KEY, request, skipReview],
    queryFn: () => api.planTransaction(request, skipReview),
    enabled: request.length > 0,
    retry: false,
  });
  return { ...query, request, skipReview };
}
