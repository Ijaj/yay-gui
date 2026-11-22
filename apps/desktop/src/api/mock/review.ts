// Mock review screen backend.
import type { ReviewBundle } from "../types";
import { aurPackage, currentCommit, DAY, delay, now, pkgbuild } from "./data";
import type { MockState } from "./state";

export async function review(s: MockState, name: string): Promise<ReviewBundle> {
  await delay();
  const p = aurPackage(name);
  if (!p) throw new Error(`${name} is not in the AUR`);
  const commit = currentCommit(name);
  const prev = s.approvals.get(name);
  const files = [
    { path: "PKGBUILD", size: 0, content: pkgbuild(p, "2"), symlink: false },
    { path: `${name}.install`, size: 0, content: "post_install() {\n  :\n}\n", symlink: false },
    { path: "icon.png", size: 2048, content: null, symlink: false },
    { path: ".SRCINFO", size: 0, content: `pkgbase = ${name}\n`, symlink: false },
  ].map((f) => ({ ...f, size: f.content?.length ?? f.size }));
  const diff =
    prev && prev.commit !== commit
      ? [
          "diff --git a/PKGBUILD b/PKGBUILD",
          "--- a/PKGBUILD",
          "+++ b/PKGBUILD",
          "@@ -3,2 +3,2 @@",
          ` pkgver=${p.version.split("-")[0]}`,
          "-pkgrel=1",
          "+pkgrel=2",
          "",
        ].join("\n")
      : undefined;
  return {
    name,
    pkgbase: name,
    version: p.version,
    commit,
    files,
    truncated: false,
    status:
      prev === undefined
        ? "not_reviewed"
        : prev.commit === commit
          ? "approved"
          : "changed_since_review",
    ...(prev ? { previous: { ...prev, maintainer: p.maintainer ?? null } } : {}),
    ...(diff ? { diff } : {}),
    maintainer: p.maintainer ?? null,
    submitter: p.maintainer ?? null,
    url: `https://example.org/${name}`,
    votes: p.votes ?? 0,
    popularity: p.popularity ?? 0,
    outOfDate: p.outOfDate ?? null,
    firstSubmitted: (p.lastModified ?? now) - 400 * DAY,
    lastModified: p.lastModified ?? now,
  };
}
