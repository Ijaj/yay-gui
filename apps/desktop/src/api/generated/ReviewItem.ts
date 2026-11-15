// This file is generated from the Rust types by ts-rs. Do not edit it.
// Regenerate with: cargo test -p aurdeck export_bindings -- --ignored

import type { ReviewState } from "./ReviewState";

export type ReviewItem = { name: string, pkgbase: string, version: string, status: ReviewState, approvedCommit?: string, 
/**
 * Set for AUR dependencies that were not queued themselves.
 */
dependencyOf?: string, maintainer: string | null, lastModified: number, };
