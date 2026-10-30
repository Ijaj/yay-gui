import { describe, expect, it } from "vitest";
import { commandLine, renderLog, shellQuote } from "./log";

describe("renderLog", () => {
  it("strips colours and OSC titles", () => {
    expect(renderLog("\x1b[1;34m::\x1b[0m hi\x1b]0;title\x07!")).toBe(":: hi!");
  });
  it("applies carriage returns like a terminal", () => {
    expect(renderLog("10%\r50%\rdone\r\nnext\n")).toBe("done\nnext\n");
    expect(renderLog("a\r\nb")).toBe("a\nb");
  });
});

describe("commandLine", () => {
  it("quotes only when needed", () => {
    expect(shellQuote("--needed")).toBe("--needed");
    expect(shellQuote("a b")).toBe("'a b'");
    expect(shellQuote("it's")).toBe("'it'\\''s'");
    expect(commandLine(["yay", "-S", "--", "foo"])).toBe("yay -S -- foo");
  });
});
