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
    with tempfile.TemporaryDirectory(prefix="h-prompts-") as directory:
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
            terminal.send("1")
            terminal.expect("> 1.")
            terminal.send("\r")
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

        for mode in ("manual_time", "keep_clear", "failure", "list", "cancel_text", "secret", "waiting", "fields", "sleep_fields"):
            terminal = Terminal([driver, mode])
            try:
                if mode == "manual_time":
                    helper = "E.g. '1:23 pm' or '123pm' or '32 min ago' are all valid"
                    terminal.expect("When did it begin?")
                    terminal.expect(helper)
                    terminal.send("not a time\r")
                    terminal.expect("is not a time I can read")
                    terminal.expect("When did it begin?")
                    terminal.expect(helper)
                    terminal.send("120 minutes ago.\r")
                    terminal.expect("When did it end?")
                    terminal.expect(helper)
                    terminal.send("now\r")
                    terminal.expect("MANUAL_RELATIVE_OK")
                    terminal.expect("When?")
                    terminal.expect(helper)
                    terminal.send("32 min ago\r")
                    terminal.expect("EVENT_RELATIVE_OK")
                elif mode == "fields":
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
                elif mode == "sleep_fields":
                    terminal.expect("Fields to change")
                    terminal.expect("Edit start: Jan 01, 12:15 am UTC")
                    terminal.expect("Edit stop: Jan 01, 1:45 am UTC")
                    terminal.send("jj\r")
                    terminal.expect("When did it end?")
                    terminal.expect("E.g. '1:23 pm' or '123pm' or '32 min ago' are all valid")
                    terminal.send("2:00am\r")
                    terminal.expect("Edit stop: Jan 01, 2:00 am UTC (changed)")
                    terminal.send("\r")
                    terminal.expect("SLEEP_STOP_SET")
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
    with tempfile.TemporaryDirectory(prefix="h-session-") as directory:
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
            terminal.expect("1. View latest")
            terminal.expect("5. Edit")
            terminal.expect("9. Delete")
            terminal.expect("10. More")
            terminal.expect("11. Exit")
            terminal.send("\r")
            terminal.expect("PTY Baby")
            terminal.expect("Anything else?")
            terminal.send("\r")
            terminal.expect("What would you like to do?")
            # Every write enters dispatch immediately, including offline errors.
            for choice in ("2", "5", "9"):
                terminal.send(choice + "\r")
                terminal.expect("nothing to write to")
                terminal.expect("Continue")
                terminal.send("\r")
                terminal.expect("What would you like to do?")
            terminal.send("6\r")
            terminal.expect("1. Dashboard")
            terminal.send("\r")
            terminal.expect("PTY Baby")
            terminal.send("q")
            terminal.expect("Anything else?")
            terminal.send("\r")
            terminal.expect("1. Dashboard")
            terminal.send("\x1b")
            terminal.expect("What would you like to do?")
            terminal.send("jjjj\r")
            terminal.expect("7. Help")
            terminal.send("4\r")
            terminal.expect("4. Back")
            terminal.send("2\r")
            terminal.expect("Which setting?")
            terminal.send("4\r")
            terminal.expect("volume units for amounts: ml or oz?")
            terminal.send("2\r")
            terminal.expect("Setting saved")
            terminal.expect("Continue")
            terminal.send("\r")
            terminal.expect("4. Back")
            assert 'units = "oz"' in config.read_text()
            terminal.send("2\r")
            terminal.expect("Which setting?")
            terminal.send("\x1b")
            terminal.expect("4. Back")
            assert 'units = "oz"' in config.read_text()
            terminal.send("\x1b")
            terminal.expect("7. Help")
            terminal.send("\x1b")
            terminal.expect("What would you like to do?")
            terminal.send("\x03")
            terminal.finish()
            assert b": ready" not in terminal.transcript
            assert b"Options" not in terminal.transcript
            assert b"options" not in terminal.transcript
        finally:
            terminal.close()

        terminal = Terminal([str(ROOT / "target/debug/huckleberry-cli"), "--config", str(config), "--offline", str(snapshot)])
        saved_snapshot = snapshot.read_text()
        snapshot.unlink()
        try:
            terminal.expect("What would you like to do?")
            terminal.send("\r")
            terminal.expect("error:")
            terminal.expect("2. Retry")
            # A retry is explicit and reopens the source rather than caching failure.
            snapshot.write_text(saved_snapshot)
            terminal.send("2\r")
            terminal.expect("PTY Baby")
            terminal.expect("Anything else?")
            terminal.send("\x1b")
            terminal.expect("What would you like to do?")
            terminal.send("\x03")
            terminal.finish()
            assert b": ready" not in terminal.transcript
            assert b"Options" not in terminal.transcript
            assert b"options" not in terminal.transcript
        finally:
            terminal.close()

        terminal = Terminal([str(ROOT / "target/debug/huckleberry-cli"), "--config", str(config)], pipe_stdout=True)
        try:
            terminal.expect("What would you like to do?")
            terminal.send("9j\r")
            terminal.expect("6. About this build")
            terminal.send("j" * 5 + "\r")
            terminal.expect("Anything else?")
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
