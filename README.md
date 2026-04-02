# NoteHolder

A multi-note sustain pad MIDI generator plugin with a piano keyboard UI (C2–C6).
Click keys to latch notes on; click again to release. Multiple notes can be held
simultaneously. Built with [nih-plug](https://github.com/robbert-vdh/nih-plug) and egui.

**By Aritz Beobide-Cardinal & Claude**

## Features

- Piano keyboard spanning C2–C6 (49 keys)
- Click to toggle notes on/off — hold as many as you like
- All keys labelled with note names
- Global velocity, MIDI channel, and octave offset controls
- All Notes Off panic button
- Key toggle states exposed as plugin parameters (automatable, visible in host rack view)
- CLAP + VST3 output via nih-plug

> **Note:** nih-plug does not support LV2. The plugin is built as CLAP and VST3.
> Carla fully supports both formats.

## Building

Requires Rust and the system libraries listed below. Inside `nix-shell`:

```bash
nix-shell
bundle noteholder --release
# → target/bundled/noteholder.clap
# → target/bundled/noteholder.vst3
```

To load in Carla without installing, symlink the bundle:

```bash
mkdir -p ~/.vst3
ln -s "$PWD/target/bundled/noteholder.vst3" ~/.vst3/noteholder.vst3
```

## Installing on NixOS

Add to your `configuration.nix`:

```nix
{ config, pkgs, ... }:

let
  noteholder = import (builtins.fetchGit {
    url = "https://github.com/ARitz-Cracker/noteholder.git";
    rev = "<commit-hash>";
  }) { inherit pkgs; };
in {
  environment.systemPackages = [ noteholder ];

  # CLAP — set the scan path:
  environment.sessionVariables.CLAP_PATH = "${noteholder}/lib/clap";

  # VST3 — merges $out/lib/vst3 from all systemPackages into
  # /run/current-system/sw/lib/vst3 — configure your DAW to scan that path:
  environment.pathsToLink = [ "/lib/vst3" ];
}
```

Then rebuild:

```bash
sudo nixos-rebuild switch
```

## Dependencies

On Linux, the following system libraries are required at runtime:

- `libGL` — egui OpenGL renderer
- `xorg.libX11`, `xorg.libxcb`, `xorg.libXcursor`, `xorg.libXrandr`, `xorg.libXi` — X11 window embedding
- `libxkbcommon` — keyboard input

The `default.nix` and `shell.nix` handle all of this automatically.

## Known limitations

- Window resizing is not supported — nih-plug hardcodes `can_resize = false` in its
  CLAP and VST3 wrappers ([nih-plug#195](https://github.com/robbert-vdh/nih-plug/pull/195)).
  This affects all hosts, not just Carla.
