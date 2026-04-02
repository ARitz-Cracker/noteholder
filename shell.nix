{ pkgs ? import <nixpkgs> { } }:

let
  runtimeLibs = pkgs.lib.makeLibraryPath (with pkgs; [
    libGL
    xorg.libX11
    xorg.libxcb
    xorg.libXcursor
    xorg.libXrandr
    xorg.libXi
    libxkbcommon
  ]);
in
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    rustup
    pkg-config
    binutils
    patchelf
  ];

  buildInputs = with pkgs; [
    xorg.libX11
    xorg.libXcursor
    xorg.libXrandr
    xorg.libXi
    xorg.libxcb
    libGL
    libxkbcommon
    lv2
  ];

  shellHook = ''
    export LD_LIBRARY_PATH="${runtimeLibs}:$LD_LIBRARY_PATH"

    # Wrap cargo xtask bundle so patchelf runs automatically after every build.
    bundle() {
      cargo xtask bundle "$@" && \
      for so in target/bundled/*.vst3/Contents/x86_64-linux/*.so \
                 target/bundled/*.clap; do
        [ -f "$so" ] && patchelf --set-rpath "${runtimeLibs}" "$so"
      done
    }
    echo "Use 'bundle noteholder --release' instead of 'cargo xtask bundle'."
  '';
}
