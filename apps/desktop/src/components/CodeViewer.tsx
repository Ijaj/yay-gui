import { defaultHighlightStyle, StreamLanguage, syntaxHighlighting } from "@codemirror/language";
import { diff } from "@codemirror/legacy-modes/mode/diff";
import { shell } from "@codemirror/legacy-modes/mode/shell";
import { EditorState } from "@codemirror/state";
import { EditorView, lineNumbers } from "@codemirror/view";
import { useEffect, useRef } from "react";

export type CodeLanguage = "shell" | "diff" | "text";

const theme = EditorView.theme({
  "&": {
    backgroundColor: "var(--surface)",
    color: "var(--text)",
    border: "1px solid var(--border)",
    borderRadius: "var(--radius-sm)",
    maxHeight: "60vh",
  },
  ".cm-scroller": { fontFamily: "var(--font-mono)", fontSize: "13px", overflow: "auto" },
  ".cm-gutters": {
    backgroundColor: "var(--surface-2)",
    color: "var(--text-muted)",
    borderRight: "1px solid var(--border)",
  },
  ".cm-content": { caretColor: "transparent" },
});

function languageFor(lang: CodeLanguage) {
  if (lang === "shell") return [StreamLanguage.define(shell)];
  if (lang === "diff") return [StreamLanguage.define(diff)];
  return [];
}

/** Read-only CodeMirror view. The text is also exposed to assistive tech and tests. */
export function CodeViewer({
  text,
  language,
  label,
}: {
  text: string;
  language: CodeLanguage;
  label: string;
}) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!host.current) return;
    const view = new EditorView({
      parent: host.current,
      state: EditorState.create({
        doc: text,
        extensions: [
          lineNumbers(),
          EditorState.readOnly.of(true),
          EditorView.editable.of(false),
          EditorView.lineWrapping,
          EditorView.contentAttributes.of({ "aria-label": label }),
          syntaxHighlighting(defaultHighlightStyle),
          theme,
          ...languageFor(language),
        ],
      }),
    });
    return () => view.destroy();
  }, [text, language, label]);
  return <div ref={host} data-testid="code-viewer" data-text={text} />;
}

/** PKGBUILDs and install scripts are bash. */
export function languageForPath(path: string): CodeLanguage {
  if (path === "PKGBUILD" || path.endsWith(".install") || path.endsWith(".sh")) return "shell";
  if (path.endsWith(".diff") || path.endsWith(".patch")) return "diff";
  return "text";
}
