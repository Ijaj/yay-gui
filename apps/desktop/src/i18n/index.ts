import app from "./en/app.json";
import header from "./en/header.json";
import installed from "./en/installed.json";
import nav from "./en/nav.json";
import pkg from "./en/pkg.json";
import placeholder from "./en/placeholder.json";
import queue from "./en/queue.json";
import review from "./en/review.json";
import search from "./en/search.json";
import terminal from "./en/terminal.json";
import tx from "./en/tx.json";

/** One catalog per key prefix; keys stay flat and fully qualified. */
export const catalogs = {
  app,
  header,
  installed,
  nav,
  pkg,
  placeholder,
  queue,
  review,
  search,
  terminal,
  tx,
};

const en = {
  ...app,
  ...header,
  ...installed,
  ...nav,
  ...pkg,
  ...placeholder,
  ...queue,
  ...review,
  ...search,
  ...terminal,
  ...tx,
};

export type MessageKey = keyof typeof en;

/** Minimal translation helper: key lookup plus `{name}` interpolation. */
export function t(key: MessageKey, params?: Record<string, string | number>): string {
  const msg: string = en[key];
  if (!params) return msg;
  return msg.replace(/\{(\w+)\}/g, (m, k: string) => (k in params ? String(params[k]) : m));
}
