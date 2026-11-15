// This file is generated from the Rust types by ts-rs. Do not edit it.
// Regenerate with: cargo test -p aurdeck export_bindings -- --ignored

import type { PackageSummary } from "./PackageSummary";

export type SearchResponse = { repo: Array<PackageSummary>, aur: Array<PackageSummary>, aurError?: string, };
