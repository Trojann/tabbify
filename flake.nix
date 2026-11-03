{
  description = "Fig-style IDE autocomplete for your Linux terminal";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      forAllSystems = nixpkgs.lib.genAttrs [
        "x86_64-linux"
        "aarch64-linux"
      ];
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "tabbify";
            version = (nixpkgs.lib.importTOML ./Cargo.toml).package.version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            meta = {
              description = "Fig-style IDE autocomplete for your Linux terminal";
              homepage = "https://github.com/Trojann/tabbify";
              license = nixpkgs.lib.licenses.mit;
              mainProgram = "tabbify";
              platforms = nixpkgs.lib.platforms.linux;
            };
          };
        }
      );

      homeModules.default = import ./nix/home-manager.nix self;

      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            inputsFrom = [ self.packages.${system}.default ];
            packages = with pkgs; [
              rustfmt
              rust-analyzer
              # PTY tests (tests/*.py, tools/try-fish.py)
              python3
              fish
              zsh
              bashInteractive
              tmux
              # tools/update-fig-specs.sh
              git
              nodejs
            ];
          };
        }
      );
    };
}
