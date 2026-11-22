// This file is generated from the Rust types by ts-rs. Do not edit it.
// Regenerate with: cargo test -p aurdeck export_bindings -- --ignored

export type InstalledPackage = { name: string, version: string, description: string, 
/**
 * Sync repo that has this package; `null` for foreign (AUR or local) packages.
 */
repo: string | null, 
/**
 * Installed explicitly rather than as a dependency.
 */
explicit: boolean, 
/**
 * A dependency nothing requires or optionally requires any more (`pacman -Qdt`).
 */
orphan: boolean, 
/**
 * Unix seconds.
 */
installDate: number | null, 
/**
 * Bytes.
 */
installedSize: number, requiredBy: Array<string>, optionalFor: Array<string>, };
