#!/usr/bin/env python3
"""Run J2Play checks by category and replace that category's last report."""

import argparse
import os
from pathlib import Path
import shlex
import shutil
import signal
import sys

from check_catalog import DESCRIPTIONS, GATES
from check_report import Interrupted, run_checks


ROOT = Path(__file__).resolve().parents[1]


def categories():
    for name, description in DESCRIPTIONS.items():
        print(f"  {name:12} {description}")


def menu():
    names = list(DESCRIPTIONS)
    print("J2Play checks\n")
    for index, name in enumerate(names, 1):
        print(f"  {index:2}) {name:12} {DESCRIPTIONS[name]}")
    print("   0) Exit\n")
    while True:
        try:
            answer = input("Choose a category (number or name, Enter = host): ").strip()
        except EOFError:
            return None
        if answer == "0":
            return None
        if not answer:
            return "host"
        if answer in GATES:
            return answer
        if answer.isdecimal() and 1 <= int(answer) <= len(names):
            return names[int(answer) - 1]
        print("Choose one of the categories above.")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, epilog=(
        "Use sh dev.sh test for a menu, sh dev.sh test --list for categories, or "
        "sh dev.sh test CATEGORY --list to preview commands. Reports: test-results/CATEGORY/"
    ))
    parser.add_argument("category", nargs="?", choices=(*GATES, "menu"))
    parser.add_argument("--list", action="store_true", help="list categories/commands without running")
    parser.add_argument("-v", "--verbose", action="store_true", help="also stream full command output")
    parser.add_argument("--shell", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args(argv)
    category = args.category or ("menu" if args.shell else "host")
    if category == "menu":
        if args.list or not sys.stdin.isatty():
            categories()
            print("\nRun: sh dev.sh test CATEGORY")
            return 0
        category = menu()
        if category is None:
            return 0
    if args.list:
        for step in GATES[category]:
            print(f"{step.name}: {step.title}\n  {shlex.join(step.command)}")
        return 0
    if args.shell and os.environ.get("J2PLAY_DEV_SHELL") != "1":
        if not shutil.which("nix"):
            parser.error("Install Nix with flakes enabled first; see README.md.")
        print("Opening the J2Play development environment...", flush=True)
        os.chdir(ROOT)
        os.execvp("nix", ["nix", "develop", f"git+file://{ROOT}", "--no-update-lock-file",
                          "--command", "sh", str(ROOT / "dev.sh"), "test", category,
                          *(["--verbose"] if args.verbose else [])])
    return run_checks(category, GATES[category], args.verbose)


def signal_interrupt(signum, _frame):
    raise Interrupted(signum)


if __name__ == "__main__":
    signal.signal(signal.SIGTERM, signal_interrupt)
    try:
        raise SystemExit(main())
    except (KeyboardInterrupt, Interrupted) as error:
        raise SystemExit(error.code if isinstance(error, Interrupted) else 130) from None
    except (OSError, ValueError) as error:
        print(f"Checks: {error}", file=sys.stderr)
        raise SystemExit(1) from None
