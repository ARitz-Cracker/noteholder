# NoteHolder — multi-note sustain pad MIDI generator
#
# IMPORTANT: nih-plug does NOT support LV2. This package produces a CLAP bundle
# (and VST3), which Carla supports natively. On NixOS you need to expose the
# CLAP path to Carla; see the "NixOS integration" section below.
#
# ── First-time setup ──────────────────────────────────────────────────────────
#
#  1. Generate the lock file (requires network):
#       cd <this-repo>
#       cargo generate-lockfile
#
#  2. Compute the outputHash for the nih-plug git dependency:
#       nix-prefetch-git --url https://github.com/robbert-vdh/nih-plug \
#                        --rev "$(grep 'nih_plug' Cargo.lock | grep 'rev =' | head -1 | \
#                                 sed 's/.*rev = "\(.*\)".*/\1/')"
#     Paste the printed sha256 into `outputHashes` below.
#     (There will be one entry per unique git source in the lockfile.)
#
# ── NixOS integration ─────────────────────────────────────────────────────────
#
#  In your configuration.nix:
#
#    let noteholder = import /path/to/noteholder/default.nix { inherit pkgs; };
#    in {
#      environment.systemPackages = [ noteholder ];
#
#      # Make Carla (and other CLAP hosts) find the plugin:
#      environment.sessionVariables.CLAP_PATH =
#        "${noteholder}/lib/clap";
#    }
#
# Alternatively, use home-manager's sessionVariables if you prefer a per-user
# install.

{ pkgs ? import <nixpkgs> { } }:

pkgs.rustPlatform.buildRustPackage rec {
  pname = "noteholder";
  version = "0.1.0";

  src = ./.;

  # ── Cargo dependency locking ───────────────────────────────────────────────
  # Cargo.lock must be committed to the repository.
  # `outputHashes` maps "<crate-name>-<version>" to the sha256 of its git tree.
  # After running `cargo generate-lockfile`, compute hashes with:
  #   nix store prefetch-file --hash-type sha256 \
  #     "$(nix-prefetch-git --url <url> --rev <rev> --fetch-submodules \
  #        2>/dev/null | jq -r .sha256)"
  # or use `carnix`/`crane`/`naersk` for a more automated approach.
  cargoLock = {
    lockFile = ./Cargo.lock;
    outputHashes = {
      # Replace these placeholder hashes with the real ones after generating
      # Cargo.lock.  Run: nix-prefetch-git --url https://github.com/robbert-vdh/nih-plug
      # (with the exact rev from your Cargo.lock) to obtain the correct sha256.
      "nih-plug-0.0.0" = pkgs.lib.fakeSha256;
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
    # LV2 headers kept for completeness; CLAP has no equivalent pkg-config dep
    lv2
  ];

  # pkg-config must be able to find X11 libraries at build time
  PKG_CONFIG_PATH = pkgs.lib.makeSearchPathOutput "dev" "lib/pkgconfig" (
    with pkgs; [ xorg.libX11 xorg.libXcursor xorg.libXrandr xorg.libXi libGL ]
  );

  # ── Build ──────────────────────────────────────────────────────────────────
  # Override the default `cargo build` with nih-plug's xtask bundler so we get
  # the correctly structured .clap (and .vst3) bundle directories.
  buildPhase = ''
    runHook preBuild
    cargo xtask bundle noteholder --release
    runHook postBuild
  '';

  # ── Install ────────────────────────────────────────────────────────────────
  installPhase = ''
    runHook preInstall

    # CLAP plugins
    mkdir -p "$out/lib/clap"
    for bundle in target/bundled/*.clap; do
      cp -r "$bundle" "$out/lib/clap/"
    done

    # VST3 plugins (bonus; hosts like Bitwig / REAPER pick these up too)
    mkdir -p "$out/lib/vst3"
    for bundle in target/bundled/*.vst3; do
      cp -r "$bundle" "$out/lib/vst3/"
    done

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
    homepage = "https://github.com/youruser/noteholder";
    license = licenses.mit;
    platforms = [ "x86_64-linux" "aarch64-linux" ];
    maintainers = [ ];
  };
}
