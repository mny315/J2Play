#!/bin/sh
set -eu

SCRIPT_PATH=$(realpath "$0")
PROJECT_ROOT=$(dirname "$SCRIPT_PATH")

usage() {
    cat <<'USAGE'
Usage: ./dev.sh [command] [architecture]
       ./dev.sh test [category] [--list] [--verbose]

  all      Build Linux for this host and all three Android APKs
  linux    Build the Linux release executable
  android  Build Android APKs: all (default), armeabi-v7a, arm64-v8a or universal
  run      Build Linux and launch its GUI
  run-current  Launch the existing Linux executable from builds/
  menu     Choose a build, run or test action interactively
  test     Run checks by category, or open the checks menu
  linux-prepare  Prepare the pinned portable Linux SDK (no Android tools)
  linux-package  Build AppDir archives: all (default), x86_64 or aarch64
  linux-check    Check prepared packages: all (default), x86_64 or aarch64
  android-prepare  Prepare pinned tools and locked Android dependencies
  android-key      Create a private signing key without building an APK
  android-check    Check both Android targets
  android-clippy   Lint both Android targets
  android-install  Build and install an APK (default: universal), preserving app data
  android-run      Build, install and launch an APK (default: universal)

With no arguments, show the menu in a terminal; otherwise build all.
Ready files: builds/<platform>/<architecture>/
Android packaging requires a Linux x86_64 build host.
Android architecture aliases: v7 = armeabi-v7a, v8 = arm64-v8a.
Set ANDROID_SERIAL to select an ADB device when several are connected.
USAGE
}

build_android() (
    COMMAND=$1
    VARIANT=${2:-all}
    if [ "$(uname -s)" != Linux ] || [ "$(uname -m)" != x86_64 ]; then
        echo "Android packaging requires a Linux x86_64 build host." >&2
        exit 1
    fi
    case "$COMMAND" in
        build|install|run) python3 "$PROJECT_ROOT/tools/android_signing.py" check ;;
    esac
    FLAKE="git+file://${PROJECT_ROOT}"
    BUILD_ROOT="$PROJECT_ROOT/android-build"
    LEGACY_BUILD_ROOT="$PROJECT_ROOT/.android-build"
    # Rename the old directory as a whole: keep signing keys, caches and diagnostics.
    if [ -d "$LEGACY_BUILD_ROOT" ] && [ ! -e "$BUILD_ROOT" ] && [ ! -L "$BUILD_ROOT" ]; then
        mv "$LEGACY_BUILD_ROOT" "$BUILD_ROOT"
        echo "Moved .android-build/ to android-build/ (contents preserved)."
    fi
    mkdir -p "$BUILD_ROOT"
    DEBUG_KEYSTORE="$BUILD_ROOT/debug.keystore"
    # If both directories already exist, preserve the old signing identity without
    # merging or removing either directory's other files.
    if [ ! -e "$DEBUG_KEYSTORE" ] && [ -f "$LEGACY_BUILD_ROOT/debug.keystore" ]; then
        cp -p "$LEGACY_BUILD_ROOT/debug.keystore" "$DEBUG_KEYSTORE"
    fi

    TOOLCHAIN_LINK="$BUILD_ROOT/toolchain"
    CARGO_HOME="$BUILD_ROOT/cargo"
    TARGET_ROOT="$BUILD_ROOT/target"
    CALLBACK_ROOT="$BUILD_ROOT/callbacks"
    CALLBACK_GENERATOR="$PROJECT_ROOT/crates/j2play-android/build-support/callback_classes.rs"
    ANDROID_TARGETS="aarch64-linux-android armv7-linux-androideabi"
    APK="$TARGET_ROOT/release/apk/j2play.apk"

    # Nix outputs are immutable, shared between checkouts and eligible for binary
    # caches. This link also keeps the complete toolchain rooted against Nix GC.
    nix build "$FLAKE#android-tools" --no-update-lock-file --out-link "$TOOLCHAIN_LINK"
    TOOLCHAIN_ROOT=$(realpath "$TOOLCHAIN_LINK")
    SDK_STORE=$(realpath "$TOOLCHAIN_ROOT/sdk")
    AMR_STORE=$(realpath "$TOOLCHAIN_ROOT/opencore-amr")
    RUST_STORE=$(realpath "$TOOLCHAIN_ROOT/rust")
    PACKAGER_STORE=$(realpath "$TOOLCHAIN_ROOT/packager")
    export CARGO_HOME
    export ANDROID_HOME="$SDK_STORE/libexec/android-sdk"
    export ANDROID_SDK_ROOT="$ANDROID_HOME"
    export RUSTC="$RUST_STORE/bin/rustc"
    export J2PLAY_ANDROID_AMR_ROOT="$AMR_STORE"
    export PATH="$RUST_STORE/bin:$PACKAGER_STORE/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/bin:$PATH"
    NDK_TOOLCHAIN="$ANDROID_HOME/ndk-bundle/toolchains/llvm/prebuilt/linux-x86_64/bin"
    export CC_aarch64_linux_android="$NDK_TOOLCHAIN/aarch64-linux-android23-clang"
    export AR_aarch64_linux_android="$NDK_TOOLCHAIN/llvm-ar"
    export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CC_aarch64_linux_android"
    export CC_armv7_linux_androideabi="$NDK_TOOLCHAIN/armv7a-linux-androideabi23-clang"
    export AR_armv7_linux_androideabi="$NDK_TOOLCHAIN/llvm-ar"
    export CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER="$CC_armv7_linux_androideabi"

    mkdir -p "$CARGO_HOME" "$TARGET_ROOT"
    # Use the same verified UI input as native Linux and the development shell.
    # Absolute immutable paths also invalidate Cargo when patched sources change.
    python3 "$PROJECT_ROOT/tools/prepare_ui_sources.py" \
        --prepared "$TOOLCHAIN_ROOT/ui-sources" --cargo-home "$CARGO_HOME" >/dev/null
    cd "$PROJECT_ROOT"
    for ANDROID_TARGET in $ANDROID_TARGETS; do
        cargo fetch --locked --target "$ANDROID_TARGET"
    done
    cargo metadata --locked --format-version 1 --no-deps >/dev/null

    if [ "$COMMAND" = prepare ]; then
        rustc --version
        cargo-apk2 apk2 --version
        echo "Android tools and dependencies are ready in $BUILD_ROOT"
        exit 0
    fi

    # Generated declarations are build products; callback implementations are Rust.
    rm -rf "$CALLBACK_ROOT/classes"
    mkdir -p "$CALLBACK_ROOT/classes"
    rustc --edition=2024 "$CALLBACK_GENERATOR" -o "$CALLBACK_ROOT/generate"
    "$CALLBACK_ROOT/generate" "$CALLBACK_ROOT/classes"
    jar --create --file "$CALLBACK_ROOT/callbacks.jar" -C "$CALLBACK_ROOT/classes" .
    export J2PLAY_ANDROID_CALLBACK_JAR="$CALLBACK_ROOT/callbacks.jar"

    case "$COMMAND" in
        check)
            python3 "$PROJECT_ROOT/tools/build_cache.py" -- \
                cargo-apk2 apk2 check -p j2play-android --target-dir "$TARGET_ROOT"
            ;;
        clippy)
            for ANDROID_TARGET in $ANDROID_TARGETS; do
                python3 "$PROJECT_ROOT/tools/build_cache.py" -- \
                    cargo clippy -p j2play-android --target "$ANDROID_TARGET" \
                    --target-dir "$TARGET_ROOT" --locked -- -D warnings
            done
            ;;
        build|install|run)
            python3 "$PROJECT_ROOT/tools/package_legal.py" android \
                --rust "$RUST_STORE" --ndk "$ANDROID_HOME/ndk-bundle"
            if [ ! -f "$DEBUG_KEYSTORE" ]; then
                (umask 077
                 keytool -genkeypair -noprompt -keystore "$DEBUG_KEYSTORE" -storepass android \
                    -alias androiddebugkey -keypass android -keyalg RSA -keysize 2048 -validity 10000 \
                    -dname "CN=Android Debug,O=J2Play,C=US")
            fi
            # cargo-apk2 signs its intermediate APK with the local development
            # key. Re-sign below using a password file: this packager version
            # would otherwise expose the release password in process arguments.
            export CARGO_APK_DEV_KEYSTORE="$DEBUG_KEYSTORE"
            export CARGO_APK_RELEASE_KEYSTORE="$DEBUG_KEYSTORE"
            export CARGO_APK_RELEASE_KEYSTORE_PASSWORD=android
            if [ "$VARIANT" = all ]; then
                VARIANTS="armeabi-v7a arm64-v8a universal"
            else
                VARIANTS=$VARIANT
            fi
            for ABI in $VARIANTS; do
                set --
                case "$ABI" in
                    armeabi-v7a) set -- --target armv7-linux-androideabi ;;
                    arm64-v8a) set -- --target aarch64-linux-android ;;
                esac
                # All APKs share compiled artifacts. The packager creates a new
                # archive for each variant and passes --locked to Cargo.
                python3 "$PROJECT_ROOT/tools/build_cache.py" -- \
                    cargo-apk2 apk2 build --release -p j2play-android --target-dir "$TARGET_ROOT" "$@"
                APK="$TARGET_ROOT/release/apk/j2play.apk"
                GENERATED_MANIFEST="$TARGET_ROOT/release/apk/AndroidManifest.xml"
                if [ ! -f "$GENERATED_MANIFEST" ] || [ ! -f "$APK" ]; then
                    echo "Android build did not produce the APK and manifest." >&2
                    exit 1
                fi
                if grep -q '<android:' "$GENERATED_MANIFEST"; then
                    echo "Generated Android manifest contains namespaced child elements" >&2
                    exit 1
                fi
                python3 "$PROJECT_ROOT/tools/android_signing.py" sign "$APK" \
                    --apksigner "$ANDROID_HOME/build-tools/37.0.0/apksigner"
                APK=$(python3 "$PROJECT_ROOT/tools/build_outputs.py" android "$APK" --architecture "$ABI")
                echo "APK: $APK"
            done
            if [ "$COMMAND" = install ] || [ "$COMMAND" = run ]; then
                adb install -r "$APK"
            fi
            if [ "$COMMAND" = run ]; then
                adb shell am start -n io.github.mny315.j2play/io.github.mny315.j2play.J2PlayActivity
            fi
            ;;
    esac
)

if [ "${1:-}" = test ]; then
    shift
    exec python3 "$PROJECT_ROOT/tools/check.py" --shell "$@"
fi
if [ "$#" -gt 2 ]; then
    usage >&2
    exit 2
fi
COMMAND=${1:-}
if [ -z "$COMMAND" ]; then
    if [ -t 0 ]; then COMMAND=menu; else COMMAND=all; fi
fi
case "$COMMAND" in
    linux-prepare|linux-package|linux-check)
        cd "$PROJECT_ROOT"
        # Ordinary Linux hosts use the portable SDK directly. NixOS needs its
        # host loader and build tools; reuse an already-open shell when present.
        if [ -e /etc/NIXOS ] && [ "${J2PLAY_DEV_SHELL:-}" != 1 ]; then
            exec nix develop "git+file://${PROJECT_ROOT}" --no-update-lock-file \
                --command "$SCRIPT_PATH" "$@"
        fi
        case "$COMMAND" in
            linux-prepare)
                if [ "$#" -ne 1 ]; then usage >&2; exit 2; fi
                exec python3 tools/linux_sdk.py prepare ;;
            linux-package) exec python3 tools/linux_package.py build "${2:-all}" ;;
            linux-check) exec python3 tools/linux_package.py check "${2:-all}" ;;
        esac
        ;;
esac
ANDROID_VARIANT=all
case "$COMMAND" in
    android) ANDROID_VARIANT=${2:-all} ;;
    android-install|android-run) ANDROID_VARIANT=${2:-universal} ;;
    *) if [ "$#" -gt 1 ]; then usage >&2; exit 2; fi ;;
esac
case "$ANDROID_VARIANT" in
    v7) ANDROID_VARIANT=armeabi-v7a ;;
    v8) ANDROID_VARIANT=arm64-v8a ;;
    all|armeabi-v7a|arm64-v8a|universal) ;;
    *) usage >&2; exit 2 ;;
esac
if [ "$ANDROID_VARIANT" = all ]; then
    case "$COMMAND" in android-install|android-run) usage >&2; exit 2 ;; esac
fi
if [ "$COMMAND" = menu ]; then
    if [ ! -t 0 ]; then
        echo "The menu needs a terminal. Use ./dev.sh all, linux or android." >&2
        exit 2
    fi
    while :; do
        cat <<'MENU'
J2Play development
  1) All: Linux + Android
  2) Linux
  3) Android
  4) Build and launch Linux
  5) Portable Linux packages: x86_64 + aarch64
  6) Prepare portable Linux SDK
  7) Run
  8) Tests and checks
  0) Exit
MENU
        printf 'Choose [1]: '
        if ! IFS= read -r choice; then exit 0; fi
        case "$choice" in
            1|"") COMMAND=all; break ;;
            2) COMMAND=linux; break ;;
            3) COMMAND=android; break ;;
            4) COMMAND=run; break ;;
            5) exec "$SCRIPT_PATH" linux-package all ;;
            6) exec "$SCRIPT_PATH" linux-prepare ;;
            7) COMMAND=run-current; break ;;
            8) exec "$SCRIPT_PATH" test ;;
            0) exit 0 ;;
            *) echo "Choose a number from 0 to 8." >&2 ;;
        esac
    done
fi
case "$COMMAND" in
    -h|--help|help) usage; exit 0 ;;
    all|linux|android|run|run-current|android-key|android-prepare|android-check|android-clippy|android-install|android-run) ;;
    *) usage >&2; exit 2 ;;
esac

if [ "$COMMAND" = run-current ]; then
    RUN_BINARY="$PROJECT_ROOT/builds/linux/$(uname -m)/j2play"
    if [ ! -x "$RUN_BINARY" ]; then
        echo "No ready Linux executable. Build it first with ./dev.sh linux." >&2
        exit 1
    fi
fi

if [ "${J2PLAY_DEV_SHELL:-}" != 1 ]; then
    if ! command -v nix >/dev/null 2>&1; then
        echo "Install Nix with flakes enabled first; see README.md." >&2
        exit 1
    fi
    cd "$PROJECT_ROOT"
    set -- "$COMMAND"
    case "$COMMAND" in
        android|android-install|android-run) set -- "$@" "$ANDROID_VARIANT" ;;
    esac
    exec nix develop "git+file://${PROJECT_ROOT}" --no-update-lock-file \
        --command "$SCRIPT_PATH" "$@"
fi

cd "$PROJECT_ROOT"
case "$COMMAND" in
    all|linux|run)
        python3 tools/prepare_ui_sources.py --check-cargo >/dev/null
        LINUX_BINARY=$(python3 tools/build_outputs.py linux)
        echo "Linux: $LINUX_BINARY"
        ;;
esac
case "$COMMAND" in
    all|android) build_android build "$ANDROID_VARIANT" ;;
    android-key) exec python3 "$PROJECT_ROOT/tools/android_signing.py" create ;;
    android-*) build_android "${COMMAND#android-}" "$ANDROID_VARIANT" ;;
    run) exec "$LINUX_BINARY" ;;
    run-current) exec "$RUN_BINARY" ;;
esac
echo "Ready builds: $PROJECT_ROOT/builds/"
