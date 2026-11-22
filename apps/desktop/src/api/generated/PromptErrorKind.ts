// This file is generated from the Rust types by ts-rs. Do not edit it.
// Regenerate with: cargo test -p aurdeck export_bindings -- --ignored

/**
 * Wire form of [`ErrorKind`]; serializes to the same snake_case strings.
 */
export type PromptErrorKind = "db_lock" | "target_not_found" | "commit_failed" | "conflicting_files" | "build_failed" | "auth_failed";
