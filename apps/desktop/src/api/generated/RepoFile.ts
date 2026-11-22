// This file is generated from the Rust types by ts-rs. Do not edit it.
// Regenerate with: cargo test -p aurdeck export_bindings -- --ignored

/**
 * One file of the package repository at the fetched commit.
 */
export type RepoFile = { 
/**
 * Path inside the repository.
 */
path: string, 
/**
 * Size in bytes.
 */
size: number, 
/**
 * Text content; `None` when binary or larger than [`MAX_FILE_BYTES`].
 */
content: string | null, 
/**
 * True for a symlink (the content is the link target).
 */
symlink: boolean, };
