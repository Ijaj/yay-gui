export function formatPopularity(v: number): string {
  return v.toFixed(2);
}

export function formatDate(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toISOString().slice(0, 10);
}
