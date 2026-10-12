"""Run bounded command trees and keep one readable report per check category."""

from collections import Counter, deque
from contextlib import ExitStack
from datetime import datetime
import fcntl
import os
from pathlib import Path
import shlex
import shutil
import signal
import subprocess
import sys
import time

from build_cache import managed_build
from check_catalog import DESCRIPTIONS


ROOT = Path(__file__).resolve().parents[1]


class Interrupted(Exception):
    def __init__(self, signum):
        self.code = 128 + signum
        super().__init__(f"Interrupted by signal {signum}")


def terminate(process):
    """Stop Cargo/build-script descendants too, even if their parent exits first."""
    try:
        os.killpg(process.pid, signal.SIGTERM)
        process.wait(timeout=5)
    except (ProcessLookupError, subprocess.TimeoutExpired):
        pass
    finally:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait()


def execute(step, path, verbose):
    tail = deque(maxlen=16)
    started = time.monotonic()
    with path.open("w", encoding="utf-8") as log:
        log.write(f"{step.title}\n$ {shlex.join(step.command)}\n\n")
        log.flush()
        try:
            with subprocess.Popen(
                step.command, cwd=ROOT, stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                text=True, encoding="utf-8", errors="replace", bufsize=1,
                start_new_session=True,
                env={**os.environ, "CARGO_TERM_COLOR": "never", "NO_COLOR": "1",
                     "TERM": "dumb", "PYTHONUNBUFFERED": "1"},
            ) as process:
                try:
                    for line in process.stdout:
                        log.write(line)
                        log.flush()
                        tail.append(line.rstrip()[-1000:])
                        if verbose:
                            print(line, end="", flush=True)
                    code = process.wait()
                    code = code if code >= 0 else 128 - code
                except BaseException:
                    terminate(process)
                    raise
        except OSError as error:
            message = f"Cannot run {step.command[0]}: {error}"
            log.write(message + "\n")
            tail.append(message)
            code = 127
        except (KeyboardInterrupt, Interrupted) as error:
            code = error.code if isinstance(error, Interrupted) else 130
            log.write(f"\nInterrupted (exit {code})\n")
        elapsed = time.monotonic() - started
        log.write(f"\nExit: {code}; duration: {elapsed:.1f}s\n")
    return code, elapsed, tail


class Report:
    def __init__(self, folder, category, steps):
        self.folder = folder
        self.category = category
        self.steps = steps
        self.started = datetime.now().astimezone().isoformat(timespec="seconds")
        self.results = {}
        self.problem = None

    def summary(self, state):
        counts = Counter(result[0] for result in self.results.values())
        lines = [f"J2Play checks: {self.category}", f"Started: {self.started}",
                 f"Status: {state}",
                 f"PASS {counts['PASS']} | FAIL {counts['FAIL']} | SKIP {counts['SKIP']}"
                 f" | NOT RUN {len(self.steps) - len(self.results)}", ""]
        if self.problem:
            lines.extend((self.problem, ""))
        for step in self.steps:
            status, detail = self.results.get(step.name, ("NOT RUN", ""))
            lines.append(f"{status:7} {step.title}" + (f" — {detail}" if detail else ""))
            lines.append(f"        Log: steps/{step.name}.log")
            lines.append(f"        Command: {shlex.join(step.command)}")
        lines.extend(("", "passed.log: full output of successful steps",
                      "failed.log: full output of failed/interrupted steps and runner errors",
                      "SKIP means a prerequisite failed or the run was interrupted.", ""))
        # A reader never sees half of the previous/current summary.
        temporary = self.folder / "summary.tmp"
        temporary.write_text("\n".join(lines), encoding="utf-8")
        temporary.replace(self.folder / "summary.txt")
        return lines[3]


def run_checks(category, steps, verbose=False, output_root=None):
    folder = (output_root or ROOT / "test-results") / category
    folder.mkdir(parents=True, exist_ok=True)
    report = Report(folder, category, steps)
    with ExitStack() as stack:
        lock = stack.enter_context((folder / ".lock").open("a"))
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            print(f"Checks for {category} are already running: {folder}", file=sys.stderr)
            return 1
        (folder / "steps").mkdir(exist_ok=True)
        passed = stack.enter_context((folder / "passed.log").open("w", encoding="utf-8"))
        failed = stack.enter_context((folder / "failed.log").open("w", encoding="utf-8"))
        for step in steps:
            (folder / "steps" / f"{step.name}.log").write_text("Not run.\n", encoding="utf-8")
        report.summary("RUNNING")
        print(f"J2Play checks: {category} — {DESCRIPTIONS.get(category, category)}\n"
              f"Reports: {folder}\n", flush=True)
        code = 0
        interrupted = False
        try:
            with managed_build():
                for index, step in enumerate(steps, 1):
                    path = folder / "steps" / f"{step.name}.log"
                    missing = [name for name in step.needs
                               if report.results.get(name, (None,))[0] != "PASS"]
                    if interrupted or missing:
                        reason = "run interrupted" if interrupted else "prerequisite: " + ", ".join(missing)
                        code = code or 1
                        report.results[step.name] = ("SKIP", reason)
                        path.write_text(f"SKIP: {reason}\n", encoding="utf-8")
                        print(f"[{index}/{len(steps)}] SKIP {step.title} ({reason})", flush=True)
                    else:
                        print(f"[{index}/{len(steps)}] RUN  {step.title}", flush=True)
                        step_code, elapsed, tail = execute(step, path, verbose)
                        status = "PASS" if step_code == 0 else "FAIL"
                        detail = f"{elapsed:.1f}s, exit {step_code}"
                        report.results[step.name] = (status, detail)
                        print(f"[{index}/{len(steps)}] {status} {step.title} ({detail})", flush=True)
                        destination = passed if step_code == 0 else failed
                        destination.write(f"\n===== {step.name}: {status} ({detail}) =====\n")
                        with path.open(encoding="utf-8") as source:
                            shutil.copyfileobj(source, destination)
                        destination.flush()
                        if step_code:
                            code = code or step_code
                            print(f"  Log: {path}", flush=True)
                            if not verbose:
                                for line in tail:
                                    print(f"  {line}", flush=True)
                        if step_code in (130, 143):
                            interrupted, code = True, step_code
                    report.summary("RUNNING")
        except (KeyboardInterrupt, Interrupted) as error:
            code = error.code if isinstance(error, Interrupted) else 130
            interrupted = True
            report.problem = f"Runner interrupted (exit {code})."
        except (OSError, ValueError) as error:
            code = code or 1
            report.problem = f"Runner error: {error}"
        if report.problem:
            failed.write(report.problem + "\n")
            print(report.problem, file=sys.stderr, flush=True)
        state = "INTERRUPTED" if interrupted else "FAILED" if code else "PASSED"
        counts = report.summary(state)
        print(f"\n{state}: {counts}\nSummary: {folder / 'summary.txt'}\n"
              f"Failures: {folder / 'failed.log'}", flush=True)
        return code
