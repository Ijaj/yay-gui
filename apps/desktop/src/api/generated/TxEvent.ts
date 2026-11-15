// This file is generated from the Rust types by ts-rs. Do not edit it.
// Regenerate with: cargo test -p aurdeck export_bindings -- --ignored

import type { Prompt } from "./Prompt";

export type TxEvent = { "type": "state", } & ({ "state": "planned" } | { "state": "reviewing" } | { "state": "approved" } | { "state": "running" } | { "state": "succeeded" } | { "state": "failed", 
/**
 * Human-readable failure reason.
 */
reason: string, } | { "state": "cancelled" }) | { "type": "command_started", index: number, total: number, description: string, argv: Array<string>, } | { "type": "output", text: string, } | { "type": "prompt", prompt: Prompt, } | { "type": "command_finished", index: number, code: number, signal?: string, };
