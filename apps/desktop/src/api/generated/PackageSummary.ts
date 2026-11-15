// This file is generated from the Rust types by ts-rs. Do not edit it.
// Regenerate with: cargo test -p aurdeck export_bindings -- --ignored

import type { PackageSource } from "./PackageSource";

export type PackageSummary = { name: string, version: string, description: string, source: PackageSource, installed: boolean, installedVersion?: string, votes?: number, popularity?: number, 
/**
 * Outer `None` omits the field (repo packages); `Some(None)` is JSON null.
 */
outOfDate?: number | null, 
/**
 * Outer `None` omits the field (repo packages); `Some(None)` is JSON null (orphan).
 */
maintainer?: string | null, lastModified?: number, };
