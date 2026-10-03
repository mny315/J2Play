{
  description = "J2Play clean-room Java ME emulator development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
      pkgsFor =
        system:
        import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
          config.android_sdk.accept_license = true;
          config.allowUnfreePredicate =
            package:
            builtins.elem (nixpkgs.lib.getName package) [
              "androidsdk"
              "android-sdk-cmdline-tools"
              "androidsdk-tools"
              "android-sdk-tools"
              "android-sdk-platform-tools"
              "android-sdk-platforms"
              "android-sdk-build-tools"
              "android-sdk-ndk"
              "platform-tools"
              "build-tools"
              "platforms"
              "ndk"
              "tools"
              "cmdline-tools"
            ];
        };
      androidSdkFor =
        pkgs:
        (pkgs.androidenv.composeAndroidPackages {
          # cargo-apk2 1.3.10 filters its initial SDK discovery through the
          # NDK's native API range (r29 ends at 35), even though compileSdk 36
          # is valid. Keep 35 as the discovery anchor and package against 36.
          platformVersions = [
            "35"
            "36"
          ];
          buildToolsVersions = [ "37.0.0" ];
          includeNDK = true;
          ndkVersions = [ "29.0.14206865" ];
          includeCmake = false;
          includeEmulator = false;
          includeSystemImages = false;
          includeSources = false;
        }).androidsdk;
      androidAmrFor =
        pkgs:
        let
          sdk = androidSdkFor pkgs;
          toolchain = "${sdk}/libexec/android-sdk/ndk-bundle/toolchains/llvm/prebuilt/linux-x86_64/bin";
        in
        pkgs.stdenv.mkDerivation {
          pname = "j2play-android-opencore-amr";
          inherit (pkgs.opencore-amr) version src;
          nativeBuildInputs = [ pkgs.libtool ];
          configureFlags = [
            "--host=aarch64-linux-android"
            "--enable-static"
            "--disable-shared"
          ];
          dontDisableStatic = true;
          preConfigure = ''
            export CC="${toolchain}/aarch64-linux-android23-clang"
            export CXX="${toolchain}/aarch64-linux-android23-clang++"
            export AR="${toolchain}/llvm-ar"
            export RANLIB="${toolchain}/llvm-ranlib"
            export STRIP="${toolchain}/llvm-strip"
          '';
          enableParallelBuilding = true;
        };
      androidRustFor =
        pkgs:
        pkgs.rust-bin.stable."1.97.1".minimal.override {
          extensions = [ "clippy" ];
          targets = [ "aarch64-linux-android" ];
        };
      androidPackagerFor =
        pkgs:
        let
          cargoApk = pkgs.fetchCrate {
            pname = "cargo-apk2";
            version = "1.3.10";
            unpack = false;
            sha256 = "ec3b76ce1d6ca782a6608ce2a9272d004834df8f8c44e2bf4169de1a12561afc";
          };
          ndkBuild = pkgs.fetchCrate {
            pname = "ndk-build2";
            version = "1.3.10";
            unpack = false;
            sha256 = "f1ce4d065b8233e951ce81dbb730e5ff9ca9d2b9d4e1ca6798295d5f53027a3e";
          };
          source = pkgs.runCommand "j2play-cargo-apk2-source-1.3.10" { } ''
            mkdir -p "$out"
            tar -xf ${cargoApk} -C "$out"
            tar -xf ${ndkBuild} -C "$out"
            chmod -R u+w "$out"
            sed -i 's/\r$//' "$out/cargo-apk2-1.3.10/src/apk.rs"
            sed -i 's/\r$//' "$out/ndk-build2-1.3.10/src/manifest.rs"
            patch -s -d "$out/cargo-apk2-1.3.10" -p1 < ${./crates/j2play-android/build-support/cargo-apk2-1.3.10-local-ndk-build.patch}
            patch -s -d "$out/ndk-build2-1.3.10" -p1 < ${./crates/j2play-android/build-support/ndk-build2-1.3.10-manifest-attributes.patch}
          '';
        in
        pkgs.rustPlatform.buildRustPackage {
          pname = "j2play-cargo-apk2";
          version = "1.3.10";
          src = source;
          cargoRoot = "cargo-apk2-1.3.10";
          buildAndTestSubdir = "cargo-apk2-1.3.10";
          cargoHash = "sha256-rNb4SlDNq+UZhLP1nAve4OjfSKNIbc+hIRBzucR24Aw=";
          meta = {
            description = "Pinned Android packager with J2Play callback and manifest fixes";
            license = with pkgs.lib.licenses; [
              mit
              asl20
            ];
            platforms = [ "x86_64-linux" ];
            mainProgram = "cargo-apk2";
          };
        };
      uiSourcesFor =
        pkgs:
        let
          support = ./crates/frontend-ui/build-support;
          manifest = builtins.fromJSON (builtins.readFile (support + "/sources.json"));
          archives = pkgs.linkFarm "j2play-ui-archives" (
            map (crate: {
              name = "${crate.name}-${crate.version}.crate";
              path = pkgs.fetchCrate {
                pname = crate.name;
                inherit (crate) version sha256;
                unpack = false;
              };
            }) manifest.crates
          );
        in
        pkgs.runCommand "j2play-ui-sources"
          {
            nativeBuildInputs = [
              pkgs.python3
              pkgs.patch
            ];
          }
          ''
            prepared=$(python3 ${./tools/prepare_ui_sources.py} \
              --manifest ${support}/sources.json --archive-dir ${archives} \
              --output "$TMPDIR/ui" --offline)
            cp -R "$prepared" "$out"
          '';
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
        in
        {
          ui-sources = uiSourcesFor pkgs;
        }
        // nixpkgs.lib.optionalAttrs (system == "x86_64-linux") {
          android-sdk = androidSdkFor pkgs;
          android-opencore-amr = androidAmrFor pkgs;
          android-rust = androidRustFor pkgs;
          android-packager = androidPackagerFor pkgs;
          android-tools = pkgs.linkFarm "j2play-android-tools" [
            {
              name = "sdk";
              path = androidSdkFor pkgs;
            }
            {
              name = "opencore-amr";
              path = androidAmrFor pkgs;
            }
            {
              name = "rust";
              path = androidRustFor pkgs;
            }
            {
              name = "packager";
              path = androidPackagerFor pkgs;
            }
            {
              name = "ui-sources";
              path = uiSourcesFor pkgs;
            }
          ];
        }
      );

      devShells = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              # Rust compiler, package manager, lints, formatting and IDE support.
              cargo
              cargo-deny
              rustc
              clippy
              rustfmt
              rust-analyzer
              nixfmt

              # Android packaging tools (jar/keytool/d8/apksigner). Callback
              # declarations are generated by Rust; javac is not used.
              jdk17_headless

              # Project/bootstrap and archive inspection tools.
              python3
              git
              jq
              zip
              unzip
              file

              # Native build discovery and trusted CA roots. Keeping these here
              # avoids host-specific failures if a future crate needs them.
              pkg-config
              cmake
              cacert

              # Linux audio/controller adapter and winit/wgpu window backends.
              # Keep ALSA and PulseAudio available for audio output.
              # Do not expose PipeWire development headers here:
              # bundled SDL 2.26.4 is not source-compatible with newer PipeWire
              # headers (for example PipeWire 1.6.x). On PipeWire desktops the
              # PulseAudio backend works through pipewire-pulse.
              alsa-lib
              libpulseaudio
              wayland
              wayland-scanner
              wayland-protocols
              libffi
              libxkbcommon
              libglvnd
              vulkan-loader
              # GPU-independent startup error dialog when no notification
              # service is available in the desktop session.
              zenity
              # Private logind fixture bus for the opt-in lifecycle check.
              dbus
              libxcb
              libx11
              libxcursor
              libxext
              libxfixes
              libxi
              libxinerama
              libxrandr
              libxrender
              opencore-amr
            ];

            env = {
              J2PLAY_DEV_SHELL = "1";
              RUST_BACKTRACE = "1";
              CARGO_TERM_COLOR = "always";
              CMAKE_POLICY_VERSION_MINIMUM = "3.5";
              LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
                pkgs.alsa-lib
                pkgs.libpulseaudio
                pkgs.wayland
                pkgs.libffi
                pkgs.libxkbcommon
                pkgs.libglvnd
                pkgs.vulkan-loader
                pkgs.libxcb
                pkgs.libx11
                pkgs.libxcursor
                pkgs.libxext
                pkgs.libxfixes
                pkgs.libxi
                pkgs.libxinerama
                pkgs.libxrandr
                pkgs.libxrender
                pkgs.opencore-amr
              ];
            };

            shellHook = ''
              export CARGO_HOME="$PWD/target/dev-cargo"
              python3 tools/prepare_ui_sources.py --prepared ${uiSourcesFor pkgs} \
                --cargo-home "$CARGO_HOME" >/dev/null || exit 1
              if [ -d /run/opengl-driver/lib ]; then
                export LD_LIBRARY_PATH="/run/opengl-driver/lib:$LD_LIBRARY_PATH"
              fi
              echo "J2Play development shell"
              echo "Rust: $(rustc --version)"
              echo "Run workspace checks: see README.md"
            '';
          };

        }
      );

      formatter = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
        in
        pkgs.nixfmt
      );
    };
}
