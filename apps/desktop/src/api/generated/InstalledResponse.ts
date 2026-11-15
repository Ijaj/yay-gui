// This file is generated from the Rust types by ts-rs. Do not edit it.
// Regenerate with: cargo test -p aurdeck export_bindings -- --ignored

import type { InstalledPackage } from "./InstalledPackage";

export type InstalledResponse = { packages: Array<InstalledPackage>, 
/**
 * Set when the pacman database could not be read.
 */
error?: string, 
/**
 * Sync databases that were skipped (their packages look foreign).
 */
warnings?: Array<string>, };
