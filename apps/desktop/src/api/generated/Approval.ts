// This file is generated from the Rust types by ts-rs. Do not edit it.
// Regenerate with: cargo test -p aurdeck export_bindings -- --ignored

/**
 * One approval of a package base at a commit.
 */
export type Approval = { 
/**
 * Approved git commit.
 */
commit: string, 
/**
 * Unix seconds when the user approved it.
 */
approvedAt: number, 
/**
 * AUR maintainer at the time, if known.
 */
maintainer: string | null, };
