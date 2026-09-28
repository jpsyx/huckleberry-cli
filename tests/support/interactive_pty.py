"""Bounded PTY journeys using only synthetic/offline data."""
import argparse
import fcntl
import struct
import glob
import json
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
    def __init__(self, arguments, pipe_stdout=False):
        self.master, self.slave = pty.openpty()
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
        self.original = termios.tcgetattr(self.slave)
        self.process = subprocess.Popen(
            arguments, stdin=self.slave, stdout=subprocess.PIPE if pipe_stdout else self.slave, stderr=self.slave,
            cwd=ROOT, env={**os.environ, "NO_COLOR": "1", "TERM": "xterm-256color"},
        )
        self.buffer = b""
        self.transcript = b""

    def expect(self, text):
        wanted = text.encode()
        deadline = time.monotonic() + 15
        while wanted not in self.buffer:
            if time.monotonic() >= deadline:
                raise AssertionError(f"Timeout waiting for {text!r}: {self.buffer[-4000:]!r}")
            readable, _, _ = select.select([self.master], [], [], 0.1)
            if readable:
                chunk = os.read(self.master, 65536)
                self.buffer += chunk
                self.transcript += chunk
        self.buffer = self.buffer.split(wanted, 1)[1]

    def send(self, value):
        os.write(self.master, value.encode())

    def finish(self):
        assert self.process.wait(timeout=10) == 0
        restored = termios.tcgetattr(self.slave)
        assert restored[3] & ~getattr(termios, "PENDIN", 0) == self.original[3] & ~getattr(termios, "PENDIN", 0), f"terminal input/echo was not restored for {self.process.args}: {self.original[3]} -> {restored[3]}"

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
        tokio = max(glob.glob(str(ROOT / "target/debug/deps/libtokio-*.rlib")), key=os.path.getmtime)
        anyhow = max(glob.glob(str(ROOT / "target/debug/deps/libanyhow-*.rlib")), key=os.path.getmtime)
        subprocess.run([
            "rustc", "--edition=2024", "tests/support/prompt_driver.rs", "-o", driver,
            "-L", "dependency=target/debug/deps", "--extern", "app=target/debug/libapp.rlib",
            "--extern", f"anyhow={anyhow}", "--extern", f"tokio={tokio}",
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
            terminal.expect("Choose answer")
            terminal.send("P\r")
            terminal.expect("AFTER=yes")
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

        for mode in ("keep_clear", "failure", "list", "cancel_text", "secret", "waiting", "fields"):
            terminal = Terminal([driver, mode])
            try:
                if mode == "fields":
                    terminal.expect("Fields to change")
                    terminal.send("\r")
                    terminal.expect("FIELDS_KEPT")
                    terminal.expect("Fields to change")
                    terminal.send("jjj\r")
                    terminal.expect("Keep current value")
                    terminal.send("jjjj\r")
                    terminal.expect("Fields to change")
                    terminal.send("\r")
                    terminal.expect("OUTCOME_CLEARED")
                elif mode == "waiting":
                    terminal.expect("WAITING_FOR_INTERRUPT")
                    # PTYs created by this harness are not controlling terminals.
                    # Deliver the same SIGINT a cooked terminal sends for Ctrl-C.
                    time.sleep(0.1)
                    terminal.process.send_signal(2)
                    terminal.expect("INTERRUPTED")
                    terminal.expect("Choose answer")
                    terminal.send("\r")
                    terminal.expect("AFTER=no")
                elif mode == "keep_clear":
                    terminal.expect("Existing note")
                    terminal.send("\r")
                    terminal.expect('KEPT=Some("exact stored note")')
                    terminal.expect("Existing note")
                    terminal.send("HH\r")
                    terminal.expect("CLEARED=None")
                    terminal.expect("After text")
                    terminal.send("\r")
                    terminal.expect("AFTER=yes")
                elif mode == "failure":
                    terminal.expect("RESTORED_AFTER_ERROR")
                elif mode == "list":
                    terminal.expect("Synthetic foods")
                    terminal.send("H" * 35)
                    terminal.send("/Food 79\r\r")
                    terminal.expect('SELECTED=Some("id-79")')
                elif mode == "cancel_text":
                    terminal.expect("Text to cancel")
                    terminal.send("not submitted\x03")
                    terminal.expect("CANCELLED")
                elif mode == "secret":
                    terminal.expect("Secret input")
                    terminal.send("test-private-password\r")
                    terminal.expect("SECRET_OK")
                    assert b"test-private-password" not in terminal.transcript
                terminal.finish()
            finally:
                terminal.close()


def session():
    subprocess.run(["cargo", "build", "-p", "huckleberry-cli"], cwd=ROOT, check=True, capture_output=True)
    with tempfile.TemporaryDirectory(prefix="hb-session-") as directory:
        config = pathlib.Path(directory) / "config.toml"
        config.write_text("units = 'ml'\n")
        snapshot = pathlib.Path(directory) / "snapshot.json"
        now = time.time()
        snapshot.write_text(json.dumps({
            "version": 1, "fetched_at": now, "timezone": "UTC", "days": 7,
            "child": {"cid": "synthetic", "name": "PTY Baby", "birthdate": "2026-09-01", "night_start_hour": 20, "morning_cutoff_hour": 7},
            "growth": None, "sleep": [], "feeds": [], "diapers": [], "pumps": [], "milestones": [], "live": {}, "notes": [],
        }))
        terminal = Terminal([str(ROOT / "target/debug/huckleberry-cli"), "--config", str(config), "--offline", str(snapshot)])
        try:
            terminal.expect("What would you like to do?")
            terminal.expect("4. Edit")
            terminal.expect("8. Delete")
            terminal.send("jjj\r")
            terminal.expect("edit: ready")
            terminal.send("\x1b")
            terminal.expect("What would you like to do?")
            terminal.send("jjjj\r")
            terminal.expect("delete: ready")
            terminal.send("\r")
            terminal.expect("nothing to write to")
            terminal.expect("Continue")
            terminal.send("\r")
            terminal.expect("delete: ready")
            terminal.send("\x1b")
            terminal.expect("What would you like to do?")
            terminal.send("kkk\r")
            terminal.expect("1. Dashboard")
            terminal.send("\r")
            terminal.expect("dash: ready")
            terminal.send("\r")
            terminal.expect("PTY Baby")
            terminal.send("q")
            terminal.expect("1. Dashboard")
            terminal.send("\x1b")
            terminal.expect("What would you like to do?")
            terminal.send("jjjj\r")
            terminal.expect("7. Session options")
            terminal.send("jjj\r")
            terminal.expect("Settings")
            terminal.send("j\r")
            terminal.expect("config set: ready")
            terminal.send("\r")
            terminal.expect("Which setting?")
            terminal.send("HHH\r")
            terminal.expect("volume units for amounts: ml or oz?")
            terminal.send("H\r")
            terminal.expect("Setting saved")
            terminal.expect("Continue")
            terminal.send("\r")
            terminal.expect("Settings")
            assert 'units = "oz"' in config.read_text()
            terminal.send("\x1b")
            terminal.expect("7. Session options")
            terminal.send("\x1b")
            terminal.expect("What would you like to do?")
            terminal.send("\x03")
            terminal.finish()
        finally:
            terminal.close()

        terminal = Terminal([str(ROOT / "target/debug/huckleberry-cli"), "--config", str(config)], pipe_stdout=True)
        try:
            terminal.expect("What would you like to do?")
            terminal.send("j" * 8 + "\r")
            terminal.expect("6. About this build")
            terminal.send("j" * 5 + "\r")
            terminal.expect("info: ready")
            terminal.send("\r")
            terminal.expect("Continue")
            terminal.send("\r")
            terminal.expect("6. About this build")
            terminal.send("\x1b")
            terminal.expect("What would you like to do?")
            terminal.send("\x03")
            terminal.finish()
            output = terminal.process.stdout.read()
            assert b"version=" in output and b"\x1b" not in output
            assert b"What would you like" not in output
        finally:
            terminal.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--scenario", choices=["prompts", "session", "all"], default="all")
    args = parser.parse_args()
    if args.scenario in ("prompts", "all"):
        prompts()
    if args.scenario in ("session", "all"):
        session()
    print("PTY checks passed")
