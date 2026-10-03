# J2Play

An independent Java ME emulator written in Rust. J2Play runs original MIDlet
JARs without an external JVM, with a shared interface and runtime for
**Android arm64** and **Linux x86_64 / aarch64**. Android is the main client.

Features include automatic phone profile selection, 2D and 3D graphics, audio,
game saves, customizable touch controls, keyboard and gamepad support.

Android requires **Android 11 or newer** and a **64-bit ARM system
(`arm64-v8a`)**. A 64-bit processor with a 32-bit Android installation cannot
run this APK. The Android 11 baseline was checked in the official Android
Emulator using ARM64 translation; physical Android 11 devices remain unverified.

## Using J2Play

**Stop** saves a snapshot for the next launch. Choose **Continue** to resume it
or **Start normally** to launch with the latest in-game saves. This preference
can be remembered per game. Pausing or closing the app does not create a snapshot;
snapshots may become incompatible after updates.

Virtual buttons are enabled by default on Android and disabled on Linux.
Change them in **Virtual controls**; keyboard and gamepad bindings are under
**Physical controls**. Save both the editor and its parent settings page.

## Physical controls

Default bindings are listed below. Saved custom and per-game bindings take priority.

| Action | Keyboard | Gamepad |
| --- | --- | --- |
| Directions | Arrows / WASD | D-pad / left stick |
| Phone `5` | Space / Z | A / Cross |
| Phone `0` | X | Y / Triangle or Select / Create |
| Phone `*` | C | B / Circle |
| Phone `#` | V | X / Square |
| Left / right soft key | Q / E or F1 / F2 | L1 / R1 |
| Fire / OK | Enter | — |
| Phone Back / Clear | F3 / Backspace | — |
| App menu / back | Esc | Start / Options |
| Monitoring | F10 | Left stick click |
| Fullscreen | F11 | — |
| Stop game | — | Right stick click |

The number row and NumPad send their printed digits. Letter bindings use physical
key positions. In menus, use the D-pad or left stick to navigate, A / Cross to
confirm, and B / Circle or Start / Options to go back.

## Building from source

The standard build uses **Linux**, Git and [Nix](https://nix.dev/install-nix).
Android packaging requires an **x86_64** build host. Enable flakes in your Nix
configuration:

```ini
experimental-features = nix-command flakes
```

Open the build, run and test menu from the repository root:

```sh
./dev.sh
```

Or select a command directly:

| Command | Action |
| --- | --- |
| `./dev.sh android` | Build the Android APK |
| `./dev.sh android-key` | Create a private Android signing key without building an APK |
| `./dev.sh android-install` | Build and install on an authorized USB debugging device, preserving app data |
| `./dev.sh android-run` | Build, install and launch on Android |
| `./dev.sh linux` | Build Linux for this computer |
| `./dev.sh run` | Build and launch the Linux app |
| `./dev.sh run-current` | Launch the existing Linux build |
| `./dev.sh all` | Build native Linux and Android |
| `./dev.sh test` | Choose a test category |

Dependencies are prepared automatically. Android Studio, Gradle and a separate
Rust installation are not needed. Use `./dev.sh --help` for all commands.

The finished APK and native Linux executable are moved to:

- Android: `builds/android/arm64-v8a/j2play.apk`
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
