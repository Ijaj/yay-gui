// This file is generated from the Rust types by ts-rs. Do not edit it.
// Regenerate with: cargo test -p aurdeck export_bindings -- --ignored

import type { PlanError } from "./PlanError";
import type { PlannedCommand } from "./PlannedCommand";
import type { ReviewItem } from "./ReviewItem";

export type PlanResponse = { commands: Array<PlannedCommand>, reviews: Array<ReviewItem>, error?: PlanError, 
/**
 * `/var/lib/pacman/db.lck` exists: another package manager is running (or crashed).
 */
lockHeld: boolean, 
/**
 * The session-wide "skip review" opt-in was used for this plan.
 */
skipReview: boolean, canRun: boolean, };
