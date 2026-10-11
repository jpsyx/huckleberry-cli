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
import re
import signal

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
        retained(driver)
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
                    terminal.send("1\r")
                    terminal.expect("OUTCOME_CLEARED")
                elif mode == "sleep_fields":
                    terminal.expect("Fields to change")
                    terminal.expect("Edit start · Jan 01, 12:15 am UTC")
                    terminal.expect("Edit stop · Jan 01, 1:45 am UTC")
                    terminal.send("jj\r")
                    terminal.expect("stop\r\n")
                    terminal.send("jljjjjjjjjjjjjjjj\r")
                    terminal.expect("Edit stop · Jan 01, 2:00 am UTC (changed)")
                    terminal.send("1\r")
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
                    terminal.send("3\r")
                    terminal.expect("CLEARED=None")
                    terminal.expect("After text")
                    terminal.send("\r")
                    terminal.expect("AFTER=yes")
                elif mode == "failure":
                    terminal.expect("RESTORED_AFTER_ERROR")
                elif mode == "list":
                    terminal.expect("Synthetic foods")
                    terminal.send("j" * 35)
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


def ui(terminal, text, keys=None):
    """Force a complete frame on resize so assertions also see unchanged cells."""
    if keys is not None:
        terminal.send(keys)
        time.sleep(max(0.3, len(keys) * 0.05))
    terminal.buffer = b""
    rows = getattr(terminal, "ui_rows", 24)
    terminal.ui_rows = 25 if rows == 24 else 24
    fcntl.ioctl(terminal.slave, termios.TIOCSWINSZ, struct.pack("HHHH", terminal.ui_rows, 110, 0, 0))
    terminal.process.send_signal(signal.SIGWINCH)
    deadline = time.monotonic() + 15
    refreshed = time.monotonic()
    earliest = refreshed + 0.3
    while True:
        ready, _, _ = select.select([terminal.master], [], [], 0.1)
        if ready:
            chunk = os.read(terminal.master, 65536)
            terminal.buffer += chunk
            terminal.transcript += chunk
        latest = terminal.buffer.rsplit(b"\x1b[2J", 1)[-1]
        plain = re.sub(rb"\x1b\[[0-?]*[ -/]*[@-~]", b"", latest).decode("utf-8", errors="replace")
        if time.monotonic() >= earliest and re.sub(r"\s+", "", text) in re.sub(r"\s+", "", plain):
            return
        if time.monotonic() > deadline:
            raise AssertionError(f"UI timeout for {text!r}: {plain[-4000:]}")
        if time.monotonic() - refreshed > 0.3:
            terminal.ui_rows = 25 if terminal.ui_rows == 24 else 24
            fcntl.ioctl(terminal.slave, termios.TIOCSWINSZ, struct.pack("HHHH", terminal.ui_rows, 110, 0, 0))
            terminal.process.send_signal(signal.SIGWINCH)
            refreshed = time.monotonic()


def session():
    subprocess.run(["cargo", "build", "-p", "huckleberry-cli"], cwd=ROOT, check=True, capture_output=True)
    with tempfile.TemporaryDirectory(prefix="h-session-") as directory:
        config = pathlib.Path(directory) / "config.toml"
        snapshot = pathlib.Path(directory) / "snapshot.json"
        snapshot.write_text(json.dumps({
            "version": 1, "fetched_at": time.time(), "timezone": "UTC", "days": 7,
            "child": {"cid": "synthetic", "name": "PTY Baby", "birthdate": None, "night_start_hour": 20, "morning_cutoff_hour": 7},
            "growth": None, "sleep": [], "feeds": [], "diapers": [], "pumps": [], "milestones": [], "live": {}, "notes": [],
        }))
        terminal = Terminal([str(ROOT / "target/debug/huckleberry-cli"), "--config", str(config), "--offline", str(snapshot)])
        try:
            ui(terminal, "What time does a day start?")
            ui(terminal, "What time does a day end?", "\r")
            assert not config.exists()
            ui(terminal, "What would you like to do?", "\x1b")
            assert not config.exists()
            ui(terminal, "What time does a day start?", "6\r")
            ui(terminal, "What time does a day end?", "\r")
            ui(terminal, 'How should "today" be counted?', "\r")
            ui(terminal, "Summary", "\r")
            assert 'day_mode = "discrete"' in config.read_text()
            ui(terminal, "What would you like to do?", "\x1b")
            ui(terminal, "Nursing", "2\r")
            ui(terminal, "Pause", "2\r")
            ui(terminal, "Nursing", "h")
            ui(terminal, "What would you like to do?", "\x1b")
            # The tenth Home row and sixth More row open settings.
            ui(terminal, "Home › More", "G\r")
            ui(terminal, "Show", "6\r")
            ui(terminal, "Which setting?", "2\r")
            ui(terminal, "volume units", "4\r")
            ui(terminal, "Fluid ounces (oz)", "2\r")
            ui(terminal, "Show", "\r")
            assert 'units = "oz"' in config.read_text()
            ui(terminal, "Which setting?", "2\r")
            ui(terminal, "Show", "h")
            ui(terminal, "Which setting?", "\r")
            ui(terminal, "What would you like to do?", "\x1b")
            terminal.send("\x11")
            terminal.finish()
        finally:
            terminal.close()


def retained(driver):
    terminal = Terminal([driver, "retained"])
    try:
        ui(terminal, "What would you like to do?")
        ui(terminal, "When?", "\r")
        ui(terminal, "What was in it?", "\r")
        ui(terminal, "How much dirty?", "2\r")
        ui(terminal, "What colour?", "3\r")
        ui(terminal, "consistency", "3\r")
        ui(terminal, "rash", "\r")
        ui(terminal, "Anything to note?", "\r")
        ui(terminal, "ha remembered note", "\x1b[200~ha remembered\n note\x1b[201~")
        ui(terminal, "When?", "\x1b[D" * 6)
        ui(terminal, "What was in it?", "k" * 20 + "\r")
        ui(terminal, "ha remembered note", "\r" * 5)
        ui(terminal, "When?", "\x1b[D" * 6)
        ui(terminal, "What would you like to do?", "\x1b[D" * 2)
        ui(terminal, "When?", "\r")
        ui(terminal, "ha remembered note", "\r" * 6)
        ui(terminal, "DIAPER_ACCEPTED_ONCE", "\r")
        ui(terminal, "What would you like to do?", "\x1b")
        ui(terminal, "When?", "\r")
        ui(terminal, "What would you like to do?", "\x1b")
        terminal.send("\x11")
        terminal.expect("RETAINED_WRITES=1")
        terminal.finish()
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
