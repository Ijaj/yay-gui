// This file is generated from the Rust types by ts-rs. Do not edit it.
// Regenerate with: cargo test -p aurdeck export_bindings -- --ignored

import type { PackageSource } from "./PackageSource";
import type { RemoveMode } from "./RemoveMode";

export type QueueItemRequest = { "kind": "install", name: string, source: PackageSource, } | { "kind": "remove", name: string, mode: RemoveMode, } | { "kind": "system_upgrade", devel: boolean, } | { "kind": "clean_orphans" };
