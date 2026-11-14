export function formatPopularity(v: number): string {
  return v.toFixed(2);
}

export function formatDate(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toISOString().slice(0, 10);
}

const UNITS = ["B", "KiB", "MiB", "GiB", "TiB"];

/** Binary units like pacman: 1536 -> "1.5 KiB". */
export function formatSize(bytes: number): string {
  let v = bytes;
  let u = 0;
  while (v >= 1024 && u < UNITS.length - 1) {
    v /= 1024;
    u++;
  }
  return u === 0 ? `${v} B` : `${v.toFixed(v < 10 ? 1 : 0)} ${UNITS[u]}`;
}
