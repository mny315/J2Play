"""Named checks shared by the shell menu and development/automation commands."""

from dataclasses import dataclass


@dataclass(frozen=True)
class Step:
    name: str
    title: str
    command: tuple[str, ...]
    needs: tuple[str, ...] = ()


UI = Step("ui-sources", "Verify prepared UI sources",
          ("python3", "tools/prepare_ui_sources.py", "--check-cargo"))


def cargo(name, title, *args):
    return Step(name, title, ("cargo", *args), (UI.name,))


# Together these groups cover every workspace package. Host still uses
# --workspace so that adding a package cannot silently weaken the full gate.
PACKAGES = {
    "vm": ("bytecode", "classfile", "heap", "jar", "native-code", "vm", "parser-fuzz"),
    "graphics": ("graphics", "m3g", "micro3d"),
    "audio": ("amr-nb", "mmapi"),
    "runtime": ("bluetooth", "cldc", "compatibility", "device-profile", "diagnostics",
                "gcf", "launch", "midp", "natives", "platform", "rms", "runtime",
                "runtime-bootstrap", "save-state"),
    "ui": ("frontend-core", "frontend-ui", "j2play-linux", "j2play-android"),
    "conformance": ("j2play-conformance",),
}
DESCRIPTIONS = {
    "host": "All host tests and code quality checks",
    "vm": "VM, bytecode, heap, JAR, AOT and bounded parser fixtures",
    "graphics": "2D graphics, M3G and Micro3D",
    "audio": "Audio decoding and playback",
    "runtime": "Java ME APIs, profiles, runtime and saves",
    "ui": "Interface, library, controls and platform shells (host tests)",
    "conformance": "Java ME behavior with project-owned guest programs",
    "python": "Build scripts, packaging and development tools",
    "quality": "Validation, formatting, linting and dependency checks",
    "linux": "Build and inspect portable Linux packages (both architectures)",
    "android": "Check, lint and build Android APK (no device run)",
    "all": "Host checks plus Linux packages and Android APK",
}

PYTHON = Step("python", "Python tool tests", (
    "python3", "-m", "unittest", "discover", "-s", "tests/python", "-p", "test_*.py", "-v",
))
VALIDATION = (
    Step("profiles", "Device profile validation", ("python3", "tools/validate_profile.py")),
    Step("source-tree", "Rust source tree rules", ("python3", "tools/validate_rust_source_tree.py")),
    Step("cldc-fixtures", "CLDC fixture validation", ("python3", "tools/validate_cldc_fixtures.py")),
)
FORMAT = cargo("rustfmt", "Rust formatting", "fmt", "--all", "--", "--check")
CLIPPY = cargo("clippy", "Rust lints", "clippy", "--workspace", "--all-targets",
               "--all-features", "--locked", "--", "-D", "warnings")
DEPENDENCIES = cargo("dependencies", "Dependency advisories, licenses and sources",
                     "deny", "check", "advisories", "licenses", "sources")
FINAL = (
    Step("nixfmt", "Nix formatting", ("nixfmt", "--check", "flake.nix")),
    Step("whitespace", "Diff whitespace", ("git", "diff", "--check")),
)
HOST = (UI, *VALIDATION, PYTHON, FORMAT, CLIPPY,
        cargo("rust-tests", "All Rust workspace tests", "test", "--workspace", "--locked",
              "--no-fail-fast"), DEPENDENCIES, *FINAL)
LINUX = (
    UI,
    cargo("linux-interpreter", "Linux without AOT", "check", "-p", "j2play-linux",
          "--no-default-features", "--locked"),
    Step("linux-build", "Build portable Linux packages",
         ("python3", "tools/linux_package.py", "build", "all"), (UI.name,)),
    Step("linux-packages", "Inspect portable Linux packages",
         ("python3", "tools/linux_package.py", "check", "all"), ("linux-build",)),
)
ANDROID = tuple(Step(action, title, ("./dev.sh", action)) for action, title in (
    ("android-check", "Android target check"),
    ("android-clippy", "Android lints"),
    ("android", "Build Android APK"),
))

GATES = {
    "host": HOST,
    **{name: (UI, cargo(name, DESCRIPTIONS[name], "test",
                       *(arg for package in packages for arg in ("-p", package)),
                       "--locked", "--no-fail-fast"))
       for name, packages in PACKAGES.items()},
    "python": (PYTHON,),
    "quality": (UI, *VALIDATION, FORMAT, CLIPPY, DEPENDENCIES, *FINAL),
    "linux": LINUX,
    "android": ANDROID,
    "all": (*HOST, *LINUX[1:], *ANDROID),
}
