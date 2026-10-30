import type { PackageSource } from "../api";

const COLORS: Record<string, string> = {
  core: "var(--badge-core)",
  extra: "var(--badge-extra)",
  multilib: "var(--badge-multilib)",
  aur: "var(--badge-aur)",
};

export function SourceBadge({ source }: { source: PackageSource }) {
  const label = source.kind === "aur" ? "AUR" : source.repo;
  const color = COLORS[source.kind === "aur" ? "aur" : source.repo] ?? "var(--badge-other)";
  return (
    <span
      data-testid="source-badge"
      className="inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-semibold"
      style={{ color, borderColor: color }}
    >
      {label}
    </span>
  );
}
