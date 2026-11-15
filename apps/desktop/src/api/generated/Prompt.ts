// This file is generated from the Rust types by ts-rs. Do not edit it.
// Regenerate with: cargo test -p aurdeck export_bindings -- --ignored

import type { PromptErrorKind } from "./PromptErrorKind";

export type Prompt = { "kind": "sudo_password", user: string, } | { "kind": "yes_no", question: string, defaultYes: boolean, } | { "kind": "menu", text: string, } | { "kind": "error", errorKind: PromptErrorKind, line: string, };
