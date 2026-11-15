// Types that cross the Tauri boundary are generated from the Rust types (ts-rs) into ./generated.
// Regenerate with: cargo test -p aurdeck export_bindings -- --ignored
// Only UI-only types are written by hand here.

export type {
  Approval,
  InstalledPackage,
  InstalledResponse,
  PackageSource,
  PackageSummary,
  PlanError,
  PlanErrorKind,
  PlannedCommand,
  PlanResponse,
  Prompt,
  PromptErrorKind,
  QueueItemRequest,
  RemoveMode,
  RepoFile,
  ReviewBundle,
  ReviewItem,
  ReviewState,
  SearchResponse,
  TransactionState,
  TxEvent,
  TxRecord,
} from "./generated";

export interface SearchRequest {
  query: string;
}
