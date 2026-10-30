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
