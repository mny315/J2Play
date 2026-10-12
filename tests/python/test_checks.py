"""Exercise category selection, replaceable reports and interrupted command trees."""

from contextlib import nullcontext, redirect_stdout, redirect_stderr
import fcntl
import io
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import tomllib
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
import check
import check_report
from check_catalog import GATES, PACKAGES, Step


def command(name, source, needs=()):
    return Step(name, name, (sys.executable, "-c", source), needs)


class ReportTest(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="j2play-check-report-")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.folder = self.root / "fixture"
        patch = mock.patch.object(check_report, "managed_build", nullcontext)
        patch.start()
        self.addCleanup(patch.stop)

    def run_steps(self, steps, **kwargs):
        self.console = io.StringIO()
        with redirect_stdout(self.console), redirect_stderr(self.console):
            return check_report.run_checks("fixture", steps, output_root=self.root, **kwargs)

    def read(self, path):
        return (self.folder / path).read_text(encoding="utf-8")

    def test_failure_keeps_running_and_separates_by_exit_status_not_stderr(self):
        steps = (
            command("good", "import sys; print('success-marker'); print('warning-marker', file=sys.stderr)"),
            command("bad", "import sys; print('failure-marker'); sys.exit(7)"),
            command("later", "print('later-marker')"),
        )
        self.assertEqual(self.run_steps(steps), 7)
        self.assertIn("success-marker\nwarning-marker", self.read("passed.log"))
        self.assertIn("later-marker", self.read("passed.log"))
        self.assertNotIn("failure-marker", self.read("passed.log"))
        self.assertIn("failure-marker", self.read("failed.log"))
        self.assertNotIn("warning-marker", self.read("failed.log"))
        self.assertIn("PASS 2 | FAIL 1 | SKIP 0 | NOT RUN 0", self.read("summary.txt"))
        self.assertIn("Status: FAILED", self.read("summary.txt"))
        self.assertNotIn("success-marker", self.console.getvalue())
        self.assertIn("failure-marker", self.console.getvalue())

    def test_rerun_overwrites_outputs_and_skips_failed_prerequisite(self):
        self.run_steps((command("prepare", "print('old-prepare')"),
                        command("dependent", "print('old-dependent')", ("prepare",))))
        (self.root / "other").mkdir()
        other = self.root / "other/summary.txt"
        other.write_text("another category")
        self.assertEqual(self.run_steps((command("prepare", "raise SystemExit(4)"),
                                        command("dependent", "raise SystemExit(9)", ("prepare",)))), 4)
        self.assertEqual(self.read("passed.log"), "")
        self.assertEqual(self.read("steps/dependent.log"), "SKIP: prerequisite: prepare\n")
        self.assertNotIn("old-prepare", self.read("failed.log"))
        self.assertIn("PASS 0 | FAIL 1 | SKIP 1 | NOT RUN 0", self.read("summary.txt"))
        self.assertEqual(other.read_text(), "another category")
        self.assertEqual(self.run_steps((command("prepare", "print('new')"),)), 0)
        self.assertEqual(self.read("failed.log"), "")
        self.assertIn("Status: PASSED", self.read("summary.txt"))
        self.assertFalse((self.folder / "summary.tmp").exists())

    def test_missing_executable_and_invalid_utf8_are_reported(self):
        self.assertEqual(self.run_steps((
            Step("absent", "Missing tool", (str(self.root / "absent-command"),)),
            command("bytes", "import os; os.write(1, b'hello\\xff\\n')"),
        )), 127)
        self.assertIn("Cannot run", self.read("failed.log"))
        self.assertIn("hello�", self.read("passed.log"))

    def test_verbose_shows_success_output_and_signals_are_not_success(self):
        self.assertEqual(self.run_steps((command("good", "print('visible-marker')"),), verbose=True), 0)
        self.assertIn("visible-marker", self.console.getvalue())
        self.assertEqual(self.run_steps((command("signal", "import os, signal; os.kill(os.getpid(), signal.SIGKILL)"),)), 137)
        self.assertIn("Status: FAILED", self.read("summary.txt"))

    def test_concurrent_run_does_not_truncate_existing_report(self):
        self.folder.mkdir()
        (self.folder / "summary.txt").write_text("active report")
        with (self.folder / ".lock").open("a") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            self.assertEqual(self.run_steps((command("unused", "pass"),)), 1)
        self.assertEqual(self.read("summary.txt"), "active report")
        self.assertFalse((self.folder / "passed.log").exists())

    def test_cache_setup_failure_replaces_previous_success(self):
        self.run_steps((command("good", "pass"),))
        with mock.patch.object(check_report, "managed_build", side_effect=ValueError("cache unavailable")):
            self.assertEqual(self.run_steps((command("good", "pass"),)), 1)
        self.assertIn("cache unavailable", self.read("failed.log"))
        self.assertIn("Status: FAILED", self.read("summary.txt"))
        self.assertIn("NOT RUN 1", self.read("summary.txt"))

    def test_interrupt_stops_descendants_and_marks_remaining_steps_skipped(self):
        for signum in (signal.SIGINT, signal.SIGTERM):
            with self.subTest(signal=signum):
                ready = self.root / f"ready-{signum}"
                child_source = (
                    "import os, subprocess, sys, time; from pathlib import Path; "
                    "child = subprocess.Popen([sys.executable, '-c', "
                    "'import signal, time; signal.signal(signal.SIGTERM, signal.SIG_IGN); time.sleep(60)']); "
                    f"Path({str(ready)!r}).write_text(str(os.getpid()) + ' ' + str(child.pid)); "
                    "time.sleep(60)"
                )
                source = (
                    f"import sys; sys.path.insert(0, {str(ROOT / 'tools')!r}); "
                    "import check, check_report, signal; from contextlib import nullcontext; "
                    "from pathlib import Path; from check_catalog import Step; "
                    "check_report.managed_build = nullcontext; "
                    "signal.signal(signal.SIGTERM, check.signal_interrupt); "
                    f"steps = (Step('long', 'Long command', ({sys.executable!r}, '-c', {child_source!r})), "
                    f"Step('later', 'Later', ({sys.executable!r}, '-c', 'pass'))); "
                    f"sys.exit(check_report.run_checks('fixture', steps, output_root=Path({str(self.root)!r})))"
                )
                with subprocess.Popen([sys.executable, "-c", source], stdout=subprocess.PIPE,
                                      stderr=subprocess.PIPE, text=True) as runner:
                    try:
                        deadline = time.monotonic() + 5
                        while not ready.exists() and time.monotonic() < deadline:
                            time.sleep(0.01)
                        self.assertTrue(ready.exists(), "test command did not start")
                        runner.send_signal(signum)
                        stdout, stderr = runner.communicate(timeout=8)
                        self.assertEqual(runner.returncode, 128 + signum, stdout + stderr)
                        self.assertIn("Status: INTERRUPTED", self.read("summary.txt"))
                        self.assertIn("PASS 0 | FAIL 1 | SKIP 1 | NOT RUN 0", self.read("summary.txt"))
                        self.assertIn("Interrupted", self.read("failed.log"))
                        for pid in ready.read_text().split():
                            stat = Path(f"/proc/{pid}/stat")
                            deadline = time.monotonic() + 2
                            while stat.exists() and stat.read_text().split()[2] != "Z":
                                self.assertLess(time.monotonic(), deadline, f"descendant {pid} survived")
                                time.sleep(0.01)
                    finally:
                        if runner.poll() is None:
                            runner.kill()
                            runner.communicate()
                        if ready.exists():
                            try:
                                os.killpg(int(ready.read_text().split()[0]), signal.SIGKILL)
                            except ProcessLookupError:
                                pass


class SelectionTest(unittest.TestCase):
    def test_categories_cover_workspace_and_dependencies_are_ordered(self):
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
        packages = [tomllib.loads((ROOT / member / "Cargo.toml").read_text())["package"]["name"]
                    for member in manifest["workspace"]["members"]]
        self.assertCountEqual(packages, [package for group in PACKAGES.values() for package in group])
        for category, steps in GATES.items():
            seen = set()
            for step in steps:
                self.assertNotIn(step.name, seen, category)
                self.assertLessEqual(set(step.needs), seen, category)
                seen.add(step.name)

    def test_shell_help_and_previews_work_outside_checkout_without_nix(self):
        for args in (("--help",), ("--list",), ("graphics", "--list"), ()):
            with self.subTest(args=args):
                result = subprocess.run(["sh", str(ROOT / "dev.sh"), "test", *args], cwd="/tmp",
                                        stdin=subprocess.DEVNULL, capture_output=True, text=True,
                                        timeout=5, check=False)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertNotIn("Opening the J2Play", result.stdout)
                self.assertIn("graphics", result.stdout)

    def test_preview_does_not_run_or_replace_reports(self):
        with mock.patch.object(check, "run_checks") as run, redirect_stdout(io.StringIO()):
            self.assertEqual(check.main(["--shell", "android", "--list"]), 0)
            run.assert_not_called()

    def test_menu_retries_accepts_names_and_exits(self):
        for answers, expected in ((["invalid", "graphics"], "graphics"), ([""], "host"),
                                  (["2"], "vm"), (["0"], None)):
            with mock.patch("builtins.input", side_effect=answers), redirect_stdout(io.StringIO()):
                self.assertEqual(check.menu(), expected)

    def test_shell_bootstraps_once_and_preserves_verbose_argument(self):
        with mock.patch.dict(os.environ, {"J2PLAY_DEV_SHELL": ""}), \
                mock.patch.object(check.shutil, "which", return_value="nix"), \
                mock.patch.object(check.os, "chdir") as chdir, \
                mock.patch.object(check.os, "execvp", side_effect=SystemExit(0)) as execute, \
                redirect_stdout(io.StringIO()):
            with self.assertRaises(SystemExit):
                check.main(["--shell", "graphics", "--verbose"])
            chdir.assert_called_once_with(ROOT)
            self.assertEqual(execute.call_args.args[1], [
                "nix", "develop", f"git+file://{ROOT}", "--no-update-lock-file",
                "--command", "sh", str(ROOT / "dev.sh"), "test", "graphics", "--verbose",
            ])
        with mock.patch.dict(os.environ, {"J2PLAY_DEV_SHELL": "1"}), \
                mock.patch.object(check, "run_checks", return_value=7) as run, \
                mock.patch.object(check.os, "execvp") as execute:
            self.assertEqual(check.main(["--shell", "graphics"]), 7)
            run.assert_called_once_with("graphics", GATES["graphics"], False)
            execute.assert_not_called()


if __name__ == "__main__":
    unittest.main()
