import { useEffect, useId, useRef, useState } from "react";
import type { Prompt } from "../../api";
import { t } from "../../i18n";
import { useTransaction } from "../../state/transaction";

const BUTTON =
  "rounded-ui-sm border border-border bg-surface px-3 py-1.5 font-medium hover:bg-surface-2";
const PRIMARY =
  "rounded-ui-sm bg-accent px-4 py-1.5 font-semibold text-accent-contrast hover:bg-accent-hover";

function Frame({ title, children }: { title: string; children: React.ReactNode }) {
  const id = useId();
  return (
    <div
      role="dialog"
      aria-labelledby={id}
      className="rounded-ui border-2 border-accent bg-surface p-4 shadow-lg"
    >
      <h2 id={id} className="m-0 text-lg font-semibold">
        {title}
      </h2>
      {children}
    </div>
  );
}

function PasswordPrompt({ user }: { user: string }) {
  const answer = useTransaction((s) => s.answerPassword);
  const [value, setValue] = useState("");
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => ref.current?.focus(), []);
  return (
    <Frame title={t("tx.password.title", { user })}>
      <p className="mt-1 text-muted">{t("tx.password.desc")}</p>
      <form
        className="mt-3 flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          const pw = value;
          setValue("");
          void answer(pw);
        }}
      >
        <label className="sr-only" htmlFor="tx-password">
          {t("tx.password.label")}
        </label>
        <input
          ref={ref}
          id="tx-password"
          type="password"
          autoComplete="off"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          className="flex-1 rounded-ui-sm border border-border bg-bg px-3 py-1.5"
        />
        <button type="submit" className={PRIMARY}>
          {t("tx.send")}
        </button>
      </form>
    </Frame>
  );
}

function YesNoPrompt({ question, defaultYes }: { question: string; defaultYes: boolean }) {
  const answer = useTransaction((s) => s.answerYesNo);
  const yes = useRef<HTMLButtonElement>(null);
  const no = useRef<HTMLButtonElement>(null);
  useEffect(() => (defaultYes ? yes : no).current?.focus(), [defaultYes]);
  return (
    <Frame title={t("tx.question")}>
      <p className="mt-2 font-medium">{question}</p>
      <div className="mt-3 flex gap-2">
        <button
          ref={yes}
          type="button"
          className={defaultYes ? PRIMARY : BUTTON}
          onClick={() => answer(true)}
        >
          {t("tx.yes")}
        </button>
        <button
          ref={no}
          type="button"
          className={defaultYes ? BUTTON : PRIMARY}
          onClick={() => answer(false)}
        >
          {t("tx.no")}
        </button>
      </div>
    </Frame>
  );
}

function MenuPrompt({ text }: { text: string }) {
  const answer = useTransaction((s) => s.answerText);
  const [value, setValue] = useState("");
  return (
    <Frame title={t("tx.menu.title")}>
      <pre className="mt-2 whitespace-pre-wrap font-mono text-sm">{text}</pre>
      <form
        className="mt-3 flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          void answer(value);
          setValue("");
        }}
      >
        <label className="sr-only" htmlFor="tx-menu">
          {t("tx.menu.label")}
        </label>
        <input
          id="tx-menu"
          value={value}
          autoComplete="off"
          onChange={(e) => setValue(e.target.value)}
          className="flex-1 rounded-ui-sm border border-border bg-bg px-3 py-1.5 font-mono"
        />
        <button type="submit" className={PRIMARY}>
          {t("tx.send")}
        </button>
      </form>
    </Frame>
  );
}

export function PromptPanel({ prompt }: { prompt: Prompt }) {
  switch (prompt.kind) {
    case "sudo_password":
      return <PasswordPrompt user={prompt.user} />;
    case "yes_no":
      return <YesNoPrompt question={prompt.question} defaultYes={prompt.defaultYes} />;
    case "menu":
      return <MenuPrompt text={prompt.text} />;
    case "error":
      return null;
  }
}
