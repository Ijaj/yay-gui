#!/usr/bin/env python3
"""Record a command's raw PTY output, answering prompts from a script.

usage: record.py SCENARIO.json

SCENARIO.json:
  {
    "out": "/path/to/output.bin",   # raw PTY bytes, written unmodified
    "argv": ["yay", "-S", "pkg"],
    "rows": 40, "cols": 120,
    "env": {"KEY": "value"},        # added to the environment
    "timeout": 300,
    "expect_timeout": false,        # true: the run is meant to be killed at the timeout
    "steps": [ {"expect": "regex", "send": "text\\n", "delay": 0.3} ]
  }

Each step waits until its regex matches the ANSI-stripped output received since
the previous step matched, then waits `delay` seconds and writes `send` to the
PTY. The recording is every byte read from the PTY master, in order. Typed
input is not part of the recording unless the terminal echoes it (a password
is not echoed).

Safety: refuses to run outside a container (/.dockerenv or /run/.containerenv) or as root,
because the recorded commands install and remove packages. Exits non-zero when the timeout is
hit (unless "expect_timeout") or when a step never matched.
"""
import fcntl
import json
import os
import pty
import re
import select
import signal
import struct
import sys
import termios
import time

ANSI = re.compile(rb"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[()][A-Za-z0-9]|\r")


def main():
    if not (os.path.exists("/.dockerenv") or os.path.exists("/run/.containerenv")):
        sys.exit("refusing to run outside a container (no /.dockerenv or /run/.containerenv)")
    if os.geteuid() == 0:
        sys.exit("refusing to run as root; use a non-root user")
    sc = json.load(open(sys.argv[1]))
    pid, fd = pty.fork()
    if pid == 0:
        env = dict(os.environ)
        env.update(sc.get("env", {}))
        os.execvpe(sc["argv"][0], sc["argv"], env)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", sc.get("rows", 40), sc.get("cols", 120), 0, 0))
    steps = list(sc.get("steps", []))
    out = open(sc["out"], "wb")
    pending = b""
    deadline = time.time() + sc.get("timeout", 300)
    status = None
    timed_out = False
    while time.time() < deadline:
        r, _, _ = select.select([fd], [], [], 0.2)
        if fd in r:
            try:
                data = os.read(fd, 4096)
            except OSError:
                data = b""
            if not data:
                break
            out.write(data)
            out.flush()
            pending += data
        if steps:
            plain = ANSI.sub(b"", pending).decode("utf-8", "replace")
            m = re.search(steps[0]["expect"], plain)
            if m:
                step = steps.pop(0)
                time.sleep(step.get("delay", 0.3))
                if step["send"]:
                    os.write(fd, step["send"].encode())
                pending = plain[m.end():].encode()
    else:
        os.kill(pid, signal.SIGKILL)
        timed_out = True
        print("TIMEOUT", file=sys.stderr)
    try:
        _, status = os.waitpid(pid, 0)
    except ChildProcessError:
        pass
    out.close()
    code = os.waitstatus_to_exitcode(status) if status is not None else -1
    print(f"exit={code} unmatched_steps={len(steps)}", file=sys.stderr)
    failed = bool(steps) or (timed_out and not sc.get("expect_timeout", False))
    sys.exit(1 if failed else 0)


main()
