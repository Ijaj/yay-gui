// This file is generated from the Rust types by ts-rs. Do not edit it.
// Regenerate with: cargo test -p aurdeck export_bindings -- --ignored

import type { PlannedCommand } from "./PlannedCommand";
import type { TransactionState } from "./TransactionState";

/**
 * The last transaction, persisted so a crash mid-run is visible on the next launch.
 */
export type TxRecord = { state: TransactionState, commands: Array<PlannedCommand>, startedAt: number, updatedAt: number, 
/**
 * Set when read back: the record says "running" but no transaction is.
 */
interrupted: boolean, };
