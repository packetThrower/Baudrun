{
  lib,
  rustPlatform,
  pkg-config,
  fontconfig,
  freetype,
  libxkbcommon,
  vulkan-loader,
  wayland,
  xorg,
  libGL,
  udev,
  stdenv,
  makeBinaryWrapper,
}:

# Built against the local working tree by default — handy for
# iterating against an unreleased main. To consume a tagged release
# instead (the path nixpkgs upstreaming will take), swap the `src`
# block for a `fetchFromGitHub { rev = "v${version}"; hash = …; }`.
rustPlatform.buildRustPackage (finalAttrs: {
  pname = "baudrun";
  version = "0.10.0";

  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [
      ./Cargo.toml
      ./Cargo.lock
      ./src
      ./resources
      ./packaging
    ];
  };

  # `cargoHash` covers the whole vendored dep tree, including the
  # seven git sources Baudrun pulls (zed-industries/zed, longbridge/
  # gpui-component, and zed forks of font-kit / reqwest / scap /
  # wgpu / xim-rs). On a hash mismatch nix prints the correct
  # value — paste it back here and re-run.
  cargoHash = "sha256-USXTSUi/+oLk5bUeefHGQwWJn4J4EdksJP7fGYYJlcI=";

  nativeBuildInputs = [
    pkg-config
    rustPlatform.bindgenHook
  ]
  ++ lib.optionals stdenv.hostPlatform.isLinux [
    makeBinaryWrapper
  ];

  buildInputs = [
    fontconfig
    freetype
  ]
  ++ lib.optionals stdenv.hostPlatform.isLinux [
    libxkbcommon
    wayland
    xorg.libX11
    xorg.libxcb
    xorg.xcbutilcursor
    libGL
    udev
  ];

  # Same trick zed-editor's nixpkgs derivation uses: gpui's macOS
  # Metal shader path normally invokes the proprietary Metal shader
  # compiler at build time, which doesn't work inside the nix
  # sandbox. The `runtime_shaders` feature switches gpui to a
  # runtime-compile path that does.
  buildFeatures = lib.optionals stdenv.hostPlatform.isDarwin [
    "gpui_platform/runtime_shaders"
  ];

  # gpui-component's `icon_named!` proc-macro (in crates/macros)
  # reads SVG icons from `../assets/assets/icons` relative to its
  # CARGO_MANIFEST_DIR — relying on the upstream workspace layout
  # where `crates/ui` sits next to `crates/assets`. Cargo's
  # per-crate vendoring flattens that: the UI crate ends up at
  # `<vendor>/gpui-component-0.5.2/`, the assets crate at
  # `<vendor>/gpui-component-assets-0.5.1/`, and the relative
  # path no longer resolves. Symlink the assets crate to where the
  # macro expects it so the proc-macro can read the SVGs and emit
  # the `IconName` enum.
  preBuild = ''
    GPUI_UI=$(find "$NIX_BUILD_TOP" -maxdepth 6 -type d -name 'gpui-component-0.5.2' 2>/dev/null | head -1)
    GPUI_ASSETS=$(find "$NIX_BUILD_TOP" -maxdepth 6 -type d -name 'gpui-component-assets-0.5.1' 2>/dev/null | head -1)
    if [ -n "$GPUI_UI" ] && [ -n "$GPUI_ASSETS" ]; then
      ln -snf "$GPUI_ASSETS" "$(dirname "$GPUI_UI")/assets"
      echo "gpui-component workspace fix: $GPUI_ASSETS -> $(dirname "$GPUI_UI")/assets"
    else
      echo "WARN: gpui-component-0.5.2 or gpui-component-assets-0.5.1 not found under NIX_BUILD_TOP" >&2
      echo "WARN: GPUI_UI=$GPUI_UI  GPUI_ASSETS=$GPUI_ASSETS" >&2
    fi
  '';

  # gpui dlopens Vulkan + Wayland + libGL at runtime, so the bare
  # binary won't find them through the nix store without an explicit
  # rpath. Same `patchelf` recipe zed-editor uses.
  postFixup = lib.optionalString stdenv.hostPlatform.isLinux ''
    patchelf $out/bin/Baudrun --add-rpath ${
      lib.makeLibraryPath [
        libGL
        vulkan-loader
        wayland
      ]
    }
  '';

  # Per-platform desktop integration. Mirrors what release.yml's
  # `.dmg` (macOS) and `.deb` / `.rpm` / `.AppImage` / `.pkg.tar.zst`
  # (Linux) bundles install, just translated to nix store paths.
  postInstall =
    # macOS: assemble a real `Baudrun.app` bundle so the Dock shows
    # the icon (via Info.plist's CFBundleIconFile + the `.icns` in
    # `Resources/`) and macOS treats us as a proper app — same
    # shape `cargo packager` ships in the release `.dmg`. The bare
    # `$out/bin/Baudrun` binary would otherwise show the generic
    # system icon and miss the Info.plist-driven identity. CLI
    # symlink stays in `$out/bin/` so `nix run` and shell launches
    # still work; macOS resolves the symlink at exec time, the
    # process's executable path lands inside the bundle, and Cocoa
    # walks up from there to find Info.plist. Same pattern
    # zed-editor's nixpkgs derivation uses.
    lib.optionalString stdenv.hostPlatform.isDarwin ''
      APP=$out/Applications/Baudrun.app
      mkdir -p $APP/Contents/MacOS $APP/Contents/Resources
      mv $out/bin/Baudrun $APP/Contents/MacOS/Baudrun
      install -m644 $src/resources/Info.plist $APP/Contents/Info.plist
      install -m644 $src/resources/icons/icon.icns $APP/Contents/Resources/icon.icns
      ln -s $APP/Contents/MacOS/Baudrun $out/bin/Baudrun
    ''
    # Linux: lowercase the binary, install the `.desktop` entry +
    # icon set + udev rule. NixOS users can pick the udev rule up
    # via `services.udev.packages = [ pkgs.baudrun ];`.
    + lib.optionalString stdenv.hostPlatform.isLinux ''
      mv $out/bin/Baudrun $out/bin/baudrun

      install -Dm644 $src/packaging/linux/baudrun.desktop \
        $out/share/applications/baudrun.desktop

      install -Dm644 $src/resources/icons/icon.png \
        $out/share/icons/hicolor/512x512/apps/baudrun.png
      install -Dm644 $src/resources/icons/128x128.png \
        $out/share/icons/hicolor/128x128/apps/baudrun.png
      install -Dm644 $src/resources/icons/64x64.png \
        $out/share/icons/hicolor/64x64/apps/baudrun.png
      install -Dm644 $src/resources/icons/32x32.png \
        $out/share/icons/hicolor/32x32/apps/baudrun.png

      install -Dm644 $src/packaging/linux/60-baudrun-serial.rules \
        $out/lib/udev/rules.d/60-baudrun-serial.rules
    '';

  # The in-binary unit tests build fine but can't reasonably run in
  # the nix sandbox (font system + windowing backends). CI's
  # `cargo test` job on six platforms is where these get exercised.
  doCheck = false;

  meta = {
    description = "Serial terminal for network engineers";
    longDescription = ''
      A native serial terminal for hardware tinkerers and network
      engineers — `alacritty_terminal` for the terminal grid,
      `gpui` for everything else.
    '';
    homepage = "https://packetthrower.github.io/Baudrun/";
    changelog = "https://github.com/packetThrower/Baudrun/blob/v${finalAttrs.version}/CHANGELOG.md";
    license = lib.licenses.gpl3Plus;
    mainProgram = if stdenv.hostPlatform.isLinux then "baudrun" else "Baudrun";
    platforms = lib.platforms.linux ++ lib.platforms.darwin;
    maintainers = [ ];
  };
})
