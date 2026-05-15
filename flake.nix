{
  description = "Baudrun — a serial terminal for network engineers";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        baudrun = pkgs.callPackage ./package.nix { };
      in
      {
        packages = {
          default = baudrun;
          baudrun = baudrun;
        };

        # `nix run github:packetThrower/Baudrun` launches the app.
        apps.default = {
          type = "app";
          program = "${baudrun}/bin/${baudrun.meta.mainProgram}";
        };

        # `nix develop` brings in the same Rust toolchain the package
        # builds with, plus rustfmt / clippy / rust-analyzer for
        # editor integration. Use it as a contributor on-ramp:
        # clone, `nix develop`, `cargo run`.
        devShells.default = pkgs.mkShell {
          inputsFrom = [ baudrun ];
          packages = with pkgs; [
            rustfmt
            clippy
            rust-analyzer
          ];
        };

        formatter = pkgs.nixpkgs-fmt;
      }
    );
}
