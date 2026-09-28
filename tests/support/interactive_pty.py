"""Bounded PTY journeys using only synthetic/offline data."""
import argparse
import fcntl
import struct
import glob
import os
import pathlib
import pty
import select
import subprocess
import tempfile
import termios
import time

ROOT = pathlib.Path(__file__).resolve().parents[2]


class Terminal:
    def __init__(self, arguments):
        self.master, self.slave = pty.openpty()
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
        self.original = termios.tcgetattr(self.slave)
        self.process = subprocess.Popen(
            arguments, stdin=self.slave, stdout=self.slave, stderr=self.slave,
            cwd=ROOT, env={**os.environ, "NO_COLOR": "1", "TERM": "xterm-256color"},
        )
        self.buffer = b""

    def expect(self, text):
        wanted = text.encode()
        deadline = time.monotonic() + 15
        while wanted not in self.buffer:
            if time.monotonic() >= deadline:
                raise AssertionError(f"Timeout waiting for {text!r}: {self.buffer[-4000:]!r}")
            readable, _, _ = select.select([self.master], [], [], 0.1)
            if readable:
                self.buffer += os.read(self.master, 65536)
        self.buffer = self.buffer.split(wanted, 1)[1]

    def send(self, value):
        os.write(self.master, value.encode())

    def finish(self):
        assert self.process.wait(timeout=10) == 0
        restored = termios.tcgetattr(self.slave)
        assert restored[3] == self.original[3], "terminal input/echo was not restored"

    def close(self):
        if self.process.poll() is None:
            self.process.kill()
            self.process.wait()
        os.close(self.master)
        os.close(self.slave)


def prompts():
    subprocess.run(["cargo", "build", "-p", "huckleberry-cli"], cwd=ROOT, check=True, capture_output=True)
    with tempfile.TemporaryDirectory(prefix="hb-prompts-") as directory:
        driver = str(pathlib.Path(directory) / "prompt-driver")
        anyhow = max(glob.glob(str(ROOT / "target/debug/deps/libanyhow-*.rlib")), key=os.path.getmtime)
        subprocess.run([
            "rustc", "--edition=2024", "tests/support/prompt_driver.rs", "-o", driver,
            "-L", "dependency=target/debug/deps", "--extern", "app=target/debug/libapp.rlib",
            "--extern", f"anyhow={anyhow}",
        ], cwd=ROOT, check=True, capture_output=True)
        terminal = Terminal([driver])
        try:
            terminal.expect("Choose answer")
            terminal.send("\r")
            terminal.expect("CHOICE=no")
            terminal.expect("Optional note")
            terminal.send("\r")
            terminal.expect("NOTE=None")
            terminal.expect("Enter time")
            terminal.send("40 minutes ago.\r")
            terminal.expect("TEXT=40 minutes ago.")
            terminal.finish()
        finally:
            terminal.close()
        terminal = Terminal([driver, "cancel"])
        try:
            terminal.expect("Choose answer")
            terminal.send("\x1b")
            terminal.expect("CANCELLED")
            terminal.finish()
        finally:
            terminal.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--scenario", choices=["prompts", "session", "all"], default="all")
    args = parser.parse_args()
    if args.scenario in ("prompts", "all"):
        prompts()
    print("PTY checks passed")
