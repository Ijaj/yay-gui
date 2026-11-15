// This file is generated from the Rust types by ts-rs. Do not edit it.
// Regenerate with: cargo test -p aurdeck export_bindings -- --ignored

/**
 * Lifecycle state of a transaction.
 */
export type TransactionState = { "state": "planned" } | { "state": "reviewing" } | { "state": "approved" } | { "state": "running" } | { "state": "succeeded" } | { "state": "failed", 
/**
 * Human-readable failure reason.
 */
reason: string, } | { "state": "cancelled" };
