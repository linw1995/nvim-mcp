{
  description = "nvim-mcp - MCP server for Neovim";

  inputs = {
    utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = {
    self,
    nixpkgs,
    utils,
    ...
  } @ inputs:
    utils.lib.eachDefaultSystem
    (
      system: let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [
            inputs.fenix.overlays.default
            (final: prev: {
              neovim-unwrapped = prev.neovim-unwrapped.overrideAttrs (old: {
                src = prev.fetchFromGitHub {
                  owner = "neovim";
                  repo = "neovim";
                  rev = "b2684d9"; # 0.11.3
                  hash = "sha256-B/An+SiRWC3Ea0T/sEk8aNBS1Ab9OENx/l4Z3nn8xE4=";
                };
              });
            })
          ];
        };
        lib = pkgs.lib;
      in {
        devShells = {
          default = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [
              (fenix.stable.withComponents [
                "cargo"
                "clippy"
                "rust-src"
                "rustc"
                "rustfmt"
              ])
            ];
            buildInputs = with pkgs; [
              libiconv
            ];
            packages = with pkgs; [
              # Development
              rust-analyzer-nightly
              pre-commit

              # Integration tests
              neovim-unwrapped
              lua-language-server
            ];
          };
        };
        packages = rec {
          default = nvim-mcp;
          nvim-mcp = let
            inherit (pkgs.fenix.stable) toolchain;
            rustPlatform = pkgs.makeRustPlatform {
              cargo = toolchain;
              rustc = toolchain;
            };
            crate = builtins.fromTOML (builtins.readFile ./Cargo.toml);
            inherit (crate.package) version;
          in
            rustPlatform.buildRustPackage {
              pname = "nvim-mcp";
              inherit version;
              src = ./.;
              cargoLock = {lockFile = ./Cargo.lock;};
              checkFlags = [
                "--skip=integration_tests"
              ];
            };
        };
        apps = {
          default = {
            type = "app";
            program = lib.getExe self.packages.${system}.nvim-mcp;
          };
        };
      }
    );
}
