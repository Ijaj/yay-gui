import en from "./en.json";

export type MessageKey = keyof typeof en;

/** Minimal translation helper: key lookup plus `{name}` interpolation. */
export function t(key: MessageKey, params?: Record<string, string | number>): string {
  const msg: string = en[key];
  if (!params) return msg;
  return msg.replace(/\{(\w+)\}/g, (m, k: string) => (k in params ? String(params[k]) : m));
}
