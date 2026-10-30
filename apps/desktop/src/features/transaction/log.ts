// Turns raw terminal output into plain text for the log view.

// CSI, OSC (BEL or ST terminated) and two-byte escapes.
// biome-ignore lint/suspicious/noControlCharactersInRegex: matching terminal escapes is the point
const ESCAPES = /\x1b(?:\[[0-?]*[ -/]*[@-~]|\][^\x07\x1b]*(?:\x07|\x1b\\)|[@-Z\\-_])/g;

/** Strips ANSI escapes and applies carriage returns (progress bars redraw a line). */
export function renderLog(raw: string): string {
  const text = raw.replace(ESCAPES, "").replace(/\r\n/g, "\n");
  return text
    .split("\n")
    .map((line) => {
      const i = line.lastIndexOf("\r", line.length - 2);
      return (i >= 0 ? line.slice(i + 1) : line).replace(/\r$/, "");
    })
    .join("\n");
}

/** Quotes an argument for display the way a POSIX shell would need it. */
export function shellQuote(arg: string): string {
  return /^[A-Za-z0-9@%+=:,./_-]+$/.test(arg) ? arg : `'${arg.replace(/'/g, "'\\''")}'`;
}

export function commandLine(argv: string[]): string {
  return argv.map(shellQuote).join(" ");
}
