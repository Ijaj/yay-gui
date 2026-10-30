import { useEffect } from "react";
import { useUi } from "../state/ui";
import { GLOBAL_SEARCH_ID } from "./Header";

function isTypingTarget(el: EventTarget | null): boolean {
  if (!(el instanceof HTMLElement)) return false;
  return el.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(el.tagName);
}

export function useGlobalShortcuts() {
  const toggleTerminal = useUi((s) => s.toggleTerminal);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && !e.altKey && !e.metaKey && e.key.toLowerCase() === "j") {
        e.preventDefault();
        toggleTerminal();
        return;
      }
      if (e.key === "/" && !e.ctrlKey && !e.metaKey && !e.altKey && !isTypingTarget(e.target)) {
        e.preventDefault();
        document.getElementById(GLOBAL_SEARCH_ID)?.focus();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [toggleTerminal]);
}
