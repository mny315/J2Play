# J2Play

An independent Java ME emulator written in Rust. J2Play runs original MIDlet
JARs without an external JVM, with a shared interface and runtime for
**Android ARM32 / ARM64** and **Linux x86_64 / aarch64**. Android is the main client.

Open the build, run and test menu from the repository root:

```sh
./dev.sh
```

Or select a command directly:

| Command | Action |
| --- | --- |
| `./dev.sh android` | Build all three Android APKs |
| `./dev.sh android v7` / `v8` / `universal` | Build one Android APK |
| `./dev.sh android-key` | Create a private Android signing key without building an APK |
| `./dev.sh android-install [architecture]` | Build and install through USB or Wi-Fi ADB, preserving app data (default: universal) |
| `./dev.sh android-run [architecture]` | Build, install and launch on Android (default: universal) |
| `./dev.sh linux` | Build Linux for this computer |
| `./dev.sh run` | Build and launch the Linux app |
| `./dev.sh run-current` | Launch the existing Linux build |
| `./dev.sh all` | Build native Linux and Android |
| `./dev.sh test` | Choose a test category |

Dependencies are prepared automatically. Android Studio, Gradle and a separate
Rust installation are not needed. Use `./dev.sh --help` for all commands.
When multiple ADB devices are connected, select one with `ANDROID_SERIAL`:
`ANDROID_SERIAL=IP:PORT ./dev.sh android-run v7`.

The finished APKs and native Linux executable are moved to:

- Android: `builds/android/{armeabi-v7a,arm64-v8a,universal}/j2play.apk`
- Linux: `builds/linux/<architecture>/j2play`

The native Linux executable uses development-environment libraries; build a
portable package to run it on another system.

## Portable Linux packages

Packages use an **AppDir** with a user-local installer. They run without Nix,
FUSE or a source checkout.

To build without Nix on a Linux x86_64 or aarch64 host, install Python 3.11+,
a C/C++ compiler, CMake 3.16+, make, pkg-config, patch, binutils, Git,
CA certificates, and OpenSSL/zlib development libraries. On Debian or Ubuntu:

```sh
sudo apt install python3 build-essential cmake pkg-config patch binutils git ca-certificates libssl-dev zlib1g-dev
./dev.sh linux-prepare
./dev.sh linux-package all
./dev.sh linux-check all
```

Replace `all` with `x86_64` or `aarch64` to select one target. Packages appear in
`builds/linux/<architecture>/` as `J2Play.AppDir/` and
`j2play-linux-<architecture>.tar.gz`.

The runtime needs glibc 2.28+, Linux 4.4+, a Wayland or X11 session, a compatible
Vulkan 1.1+ driver, PulseAudio or PipeWire-Pulse, and logind/elogind with permission
to use sleep delay inhibitors. Games stay suspended if the sleep service is
unavailable. Desktop portals provide system file dialogs and external links;
an in-window file picker is also available.

Extract the archive for your architecture and install as your normal user:

```sh
tar -xzf j2play-linux-x86_64.tar.gz
./J2Play.AppDir/install.sh
```

Launch **J2Play** from the application menu, or run `J2Play.AppDir/J2Play` directly.
To update, close the app and run the new archive's installer. Games, settings and
saves are kept separately from program versions, under
`$XDG_DATA_HOME/io.github.mny315.j2play/library/`
(default: `~/.local/share/io.github.mny315.j2play/library/`).

On **Steam Deck**, install in Desktop Mode and add
`~/.local/share/j2play-program/current/J2Play` as a non-Steam game.
Start with Steam Input's Gamepad template; Gaming Mode uses the in-window picker.

Experimental Ports archives are not a supported handheld port.

## Checks

```sh
./dev.sh test host     # Host tests and code quality checks
./dev.sh test all      # Host checks, Linux packages and Android APK
./dev.sh test --list   # Available test categories
```

The launcher prepares the pinned development environment automatically.
Reports are written to `test-results/<category>/summary.txt`.

## License

J2Play is an independent clean-room project, unaffiliated with phone manufacturers,
Oracle or game publishers. Its code is licensed under [MIT](LICENSE).
Third-party notices: [NOTICE](NOTICE).
