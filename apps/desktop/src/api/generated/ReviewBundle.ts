// This file is generated from the Rust types by ts-rs. Do not edit it.
// Regenerate with: cargo test -p aurdeck export_bindings -- --ignored

import type { Approval } from "./Approval";
import type { RepoFile } from "./RepoFile";
import type { ReviewState } from "./ReviewState";

export type ReviewBundle = { name: string, pkgbase: string, version: string, 
/**
 * Commit the files below were read from; approve exactly this one.
 */
commit: string, files: Array<RepoFile>, truncated: boolean, status: ReviewState, 
/**
 * The latest earlier approval, if any.
 */
previous?: Approval, 
/**
 * Unified diff from `previous.commit` to `commit`; `None` when there is
 * no earlier approval, it is the same commit, or the old commit is gone.
 */
diff?: string, maintainer: string | null, submitter: string | null, url: string | null, votes: number, popularity: number, outOfDate: number | null, firstSubmitted: number, lastModified: number, };
