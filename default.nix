# NoteHolder — multi-note sustain pad MIDI generator
#
# IMPORTANT: nih-plug does NOT support LV2. This package produces a CLAP bundle
# (and VST3), which Carla supports natively.
#
# ── NixOS integration ─────────────────────────────────────────────────────────
#
#  In your configuration.nix:
#
#    let noteholder = import (builtins.fetchGit {
#          url = "https://github.com/ARitz-Cracker/noteholder.git";
#          rev = "<commit-hash>";
#        }) { inherit pkgs; };
#    in {
#      environment.systemPackages = [ noteholder ];
#
#      # CLAP — set the scan path:
#      environment.sessionVariables.CLAP_PATH = "${noteholder}/lib/clap";
#
#      # VST3 — merge into /run/current-system/sw/lib/vst3 (= /usr/lib/vst3 on NixOS):
#      environment.pathsToLink = [ "/lib/vst3" ];
#    }
#
# Alternatively, use home-manager's sessionVariables if you prefer a per-user
# install.

{ pkgs ? import <nixpkgs> { } }:

pkgs.rustPlatform.buildRustPackage rec {
  pname = "noteholder";
  version = "0.1.0";

  src = ./.;

  cargoLock = {
    lockFile = ./Cargo.lock;
    outputHashes = {
      "nih_plug-0.0.0"    = "sha256-rPd1Vmw3YK9aUz+hFlVUuf9la9nIhi74Goo8et6pZqE=";
      "baseview-0.1.0"   = "sha256-vFTzEh/PrrKEN5S/GnCBdJ+Im3wOZM4PY1nQebuVx14=";
      "egui-baseview-0.5.0" = "sha256-T0rfvFedUWh/6VUQyoEkNQhCeuzDeAVrEh47Udlmvj0=";
      "clap-sys-0.5.0"   = "sha256-Ha/UJlMFCVKxx1axrdRQR+T/G0xK3828xFKdfBIehKM=";
      "reflink-0.1.3"    = "sha256-1o5d/mepjbDLuoZ2/49Bi6sFgVX4WdCuhGJkk8ulhcI=";
      "vst3-sys-0.1.0"   = "sha256-tKWEmJR9aRpfsiuVr0K8XXYafVs+CzqCcP+Ea9qvZ7Y=";
    };
  };

  # ── Build tooling ──────────────────────────────────────────────────────────
  nativeBuildInputs = with pkgs; [
    pkg-config
    binutils
    autoPatchelfHook  # automatically sets rpath on all ELF files in $out
  ];

  # ── Runtime link dependencies ──────────────────────────────────────────────
  # egui on Linux uses baseview, which embeds into the host window via X11.
  buildInputs = with pkgs; [
    # X11 window embedding (baseview)
    xorg.libX11
    xorg.libXcursor
    xorg.libXrandr
    xorg.libXi
    xorg.libxcb
    # OpenGL for egui rendering
    libGL
    # Keyboard input
    libxkbcommon
    # GCC runtime (libgcc_s.so.1) linked by Rust cdylib output
    stdenv.cc.cc.lib
    # LV2 headers kept for completeness; CLAP has no equivalent pkg-config dep
    lv2
  ];

  # pkg-config must be able to find X11 libraries at build time
  PKG_CONFIG_PATH = pkgs.lib.makeSearchPathOutput "dev" "lib/pkgconfig" (
    with pkgs; [ xorg.libX11 xorg.libXcursor xorg.libXrandr xorg.libXi libGL ]
  );

  # ── Build ──────────────────────────────────────────────────────────────────
  # Use the default cargoBuildHook (avoids cargo xtask bundle, which spawns a
  # child cargo process that loses the vendored-source config Nix sets up).
  # The bundle layout is assembled manually in installPhase below.

  # ── Install ────────────────────────────────────────────────────────────────
  # CLAP: a .clap is just a renamed cdylib .so.
  # VST3: standard bundle tree — Contents/x86_64-linux/<name>.so.
  installPhase = ''
    runHook preInstall

    mkdir -p "$out/lib/clap"
    cp target/release/libnoteholder.so "$out/lib/clap/noteholder.clap"

    mkdir -p "$out/lib/vst3/noteholder.vst3/Contents/x86_64-linux"
    cp target/release/libnoteholder.so \
       "$out/lib/vst3/noteholder.vst3/Contents/x86_64-linux/noteholder.so"

    runHook postInstall
  '';

  # autoPatchelfHook picks up buildInputs automatically; nothing extra needed.

  meta = with pkgs.lib; {
    description = "Multi-note sustain pad MIDI generator (CLAP/VST3)";
    longDescription = ''
      NoteHolder is a CLAP/VST3 MIDI generator plugin with an egui piano UI
      spanning C2–C6. Click keys to latch notes on; click again to release.
      Multiple notes can be held simultaneously. Global velocity and MIDI
      channel controls are provided, along with an "All Notes Off" panic button.

      Note: nih-plug does not produce LV2 bundles. Carla fully supports CLAP
      on NixOS — set CLAP_PATH to $out/lib/clap in your environment.
    '';
    homepage = "https://github.com/ARitz-Cracker/noteholder";
    license = licenses.mit;
    platforms = [ "x86_64-linux" "aarch64-linux" ];
    maintainers = [ ];
  };
}
