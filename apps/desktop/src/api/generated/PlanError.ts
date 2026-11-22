// This file is generated from the Rust types by ts-rs. Do not edit it.
// Regenerate with: cargo test -p aurdeck export_bindings -- --ignored

import type { PlanErrorKind } from "./PlanErrorKind";

export type PlanError = { kind: PlanErrorKind, message: string, packages: Array<string>, };
