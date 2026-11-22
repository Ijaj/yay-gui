// Mock queue planner.
import type { PlannedCommand, PlanResponse, QueueItemRequest, ReviewItem } from "../types";
import { aurPackage, currentCommit, MOCK_DEPS, NAME_RE, REPO_NAMES } from "./data";
import type { MockState } from "./state";

export function plan(s: MockState, items: QueueItemRequest[], skipReview: boolean): PlanResponse {
  const base: PlanResponse = {
    commands: [],
    reviews: [],
    lockHeld: false,
    skipReview,
    canRun: false,
  };
  const fail = (kind: NonNullable<PlanResponse["error"]>["kind"], packages: string[]) => ({
    ...base,
    error: { kind, message: `${kind.replaceAll("_", " ")}: ${packages.join(", ")}`, packages },
  });
  for (const i of items) {
    if ("name" in i && !NAME_RE.test(i.name)) return fail("invalid", [i.name]);
  }
  const installs = items.flatMap((i) => (i.kind === "install" ? [i] : []));
  const removes = items.flatMap((i) => (i.kind === "remove" ? [i] : []));
  const upgrades = items.flatMap((i) => (i.kind === "system_upgrade" ? [i] : []));
  const repo = installs.filter((i) => i.source.kind === "repo").map((i) => i.name);
  const aur = installs.filter((i) => i.source.kind === "aur").map((i) => i.name);
  const notInRepos = repo.filter((n) => !REPO_NAMES.has(n));
  if (notInRepos.length) return fail("not_in_repos", notInRepos);
  const notInAur = aur.filter((n) => !aurPackage(n));
  if (notInAur.length) return fail("not_in_aur", notInAur);
  const conflict = installs.find((i) => removes.some((r) => r.name === i.name));
  if (conflict) return fail("conflict", [conflict.name]);
  if (repo.length && !upgrades.length) return fail("partial_upgrade", repo);

  const deps = aur.flatMap((n) => (MOCK_DEPS[n] ?? []).map((d) => ({ d, of: n })));
  const reviews: ReviewItem[] = [
    ...aur.map((name) => ({ name, of: undefined as string | undefined })),
    ...deps.filter((x) => !aur.includes(x.d)).map((x) => ({ name: x.d, of: x.of })),
  ].map(({ name, of }) => {
    const p = aurPackage(name);
    const a = s.approvals.get(name);
    const status =
      a === undefined
        ? "not_reviewed"
        : a.commit === currentCommit(name)
          ? "approved"
          : "changed_since_review";
    return {
      name,
      pkgbase: name,
      version: p?.version ?? "?",
      status,
      ...(status === "approved" ? { approvedCommit: a?.commit } : {}),
      ...(of ? { dependencyOf: of } : {}),
      maintainer: p?.maintainer ?? null,
      lastModified: p?.lastModified ?? 0,
    };
  });

  const commands: PlannedCommand[] = [];
  if (upgrades.length || installs.length) {
    const devel = upgrades.some((u) => u.devel);
    const argv = ["yay", upgrades.length ? "-Syu" : "-S"];
    if (installs.length) argv.push("--needed");
    if (devel) argv.push("--devel");
    if (aur.length && !skipReview) argv.push("--answerdiff", "None", "--answeredit", "None");
    const names = [...new Set(installs.map((i) => i.name))];
    let description = upgrades.length ? "Full system upgrade" : "";
    if (names.length) {
      description += `${description ? " and install " : "Install "}${names.join(", ")}`;
      argv.push("--", ...names);
    }
    commands.push({ description, argv });
  }
  for (const [mode, flag] of [
    ["plain", "-R"],
    ["with_deps", "-Rs"],
    ["with_deps_and_config", "-Rns"],
  ] as const) {
    const names = removes.filter((r) => r.mode === mode).map((r) => r.name);
    if (names.length)
      commands.push({
        description: `Remove ${names.join(", ")} (${flag})`,
        argv: ["yay", flag, "--", ...names],
      });
  }
  if (items.some((i) => i.kind === "clean_orphans"))
    commands.push({ description: "Remove orphaned packages", argv: ["yay", "-Yc"] });

  const unreviewed = reviews.filter((r) => r.status !== "approved").map((r) => r.name);
  const out = { ...base, commands, reviews };
  if (unreviewed.length && !skipReview) {
    return {
      ...out,
      error: {
        kind: "unreviewed",
        message: `AUR packages not reviewed: ${unreviewed.join(", ")}`,
        packages: unreviewed,
      },
    };
  }
  return { ...out, canRun: commands.length > 0 };
}
