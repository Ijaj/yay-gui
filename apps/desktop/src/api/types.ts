// Contract with the Rust backend (serde `rename_all = "camelCase"`).
// Keep in sync with the Tauri commands in apps/desktop/src-tauri.

export type PackageSource = { kind: "repo"; repo: string } | { kind: "aur" };

export interface PackageSummary {
  name: string;
  version: string;
  description: string;
  source: PackageSource;
  installed: boolean;
  installedVersion?: string;
  /** AUR only */
  votes?: number;
  /** AUR only */
  popularity?: number;
  /** Unix seconds when flagged out of date (AUR), null/undefined if not flagged. */
  outOfDate?: number | null;
  /** AUR maintainer; null means orphaned. */
  maintainer?: string | null;
  /** Unix seconds (AUR last modified). */
  lastModified?: number;
}

export interface SearchRequest {
  query: string;
}

export interface SearchResponse {
  repo: PackageSummary[];
  aur: PackageSummary[];
  /** Set when the AUR lookup failed; repo results are still valid. */
  aurError?: string;
}

// ---- Queue → review → run (plan.rs, review.rs, tx.rs) ----

export type RemoveMode = "plain" | "with_deps" | "with_deps_and_config";

export type QueueItemRequest =
  | { kind: "install"; name: string; source: PackageSource }
  | { kind: "remove"; name: string; mode: RemoveMode }
  | { kind: "system_upgrade"; devel: boolean }
  | { kind: "clean_orphans" };

export type PlanErrorKind =
  | "invalid"
  | "not_in_repos"
  | "not_in_aur"
  | "aur_unavailable"
  | "partial_upgrade"
  | "unreviewed"
  | "conflict"
  | "not_installable"
  | "too_many_aur_packages";

export interface PlanError {
  kind: PlanErrorKind;
  message: string;
  packages: string[];
}

export interface PlannedCommand {
  description: string;
  /** Program followed by its arguments, exactly as spawned. */
  argv: string[];
}

export type ReviewState = "not_reviewed" | "approved" | "changed_since_review";

export interface ReviewItem {
  name: string;
  pkgbase: string;
  version: string;
  status: ReviewState;
  approvedCommit?: string;
  /** Set for AUR dependencies that were not queued themselves. */
  dependencyOf?: string;
  maintainer: string | null;
  lastModified: number;
}

export interface PlanResponse {
  commands: PlannedCommand[];
  reviews: ReviewItem[];
  error?: PlanError;
  lockHeld: boolean;
  skipReview: boolean;
  canRun: boolean;
}

export interface RepoFile {
  path: string;
  size: number;
  /** null when binary or too large. */
  content: string | null;
  symlink: boolean;
}

export interface Approval {
  commit: string;
  approvedAt: number;
  maintainer: string | null;
}

export interface ReviewBundle {
  name: string;
  pkgbase: string;
  version: string;
  commit: string;
  files: RepoFile[];
  truncated: boolean;
  status: ReviewState;
  previous?: Approval;
  /** Unified diff since the previous approval. */
  diff?: string;
  maintainer: string | null;
  submitter: string | null;
  url: string | null;
  votes: number;
  popularity: number;
  outOfDate: number | null;
  firstSubmitted: number;
  lastModified: number;
}

export type TransactionState =
  | { state: "planned" }
  | { state: "reviewing" }
  | { state: "approved" }
  | { state: "running" }
  | { state: "succeeded" }
  | { state: "failed"; reason: string }
  | { state: "cancelled" };

export type PromptErrorKind =
  | "db_lock"
  | "target_not_found"
  | "commit_failed"
  | "conflicting_files"
  | "build_failed"
  | "auth_failed";

export type Prompt =
  | { kind: "sudo_password"; user: string }
  | { kind: "yes_no"; question: string; defaultYes: boolean }
  | { kind: "menu"; text: string }
  | { kind: "error"; errorKind: PromptErrorKind; line: string };

export type TxEvent =
  | ({ type: "state" } & TransactionState)
  | { type: "command_started"; index: number; total: number; description: string; argv: string[] }
  | { type: "output"; text: string }
  | { type: "prompt"; prompt: Prompt }
  | { type: "command_finished"; index: number; code: number; signal?: string };

export interface TxRecord {
  state: TransactionState;
  commands: PlannedCommand[];
  startedAt: number;
  updatedAt: number;
  /** The app stopped while this transaction was running. */
  interrupted: boolean;
}
