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
                  rev = "v0.11.3"; # b2684d9
                  hash = "sha256-B/An+SiRWC3Ea0T/sEk8aNBS1Ab9OENx/l4Z3nn8xE4=";
                };
              });
            })
          ];
        };
        lib = pkgs.lib;
        git_dirty =
          if (self.sourceInfo ? rev)
          then "false"
          else "true";
        git_commit_sha =
          self.sourceInfo.rev or (
            if (self.sourceInfo ? dirtyRev)
            then lib.strings.removeSuffix "-dirty" self.sourceInfo.dirtyRev
            else "unknown"
          );
        git_last_modified = toString self.sourceInfo.lastModified or "unknown";
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
            packages = with pkgs; [
              # Development
              rust-analyzer-nightly
              pre-commit

              # Integration tests
              neovim-unwrapped
              lua-language-server
              go
              gopls
            ];
            shellHook = ''
              # Unset SOURCE_DATE_EPOCH to prevent reproducible build timestamps during development.
              # This allows timestamps to reflect the current time, which is useful for development workflows.
              unset SOURCE_DATE_EPOCH
            '';
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
            meta = builtins.fromTOML (builtins.readFile ./Cargo.toml);
            inherit (meta.package) version name;
          in
            rustPlatform.buildRustPackage {
              pname = name;
              inherit version;
              meta = {
                description = "MCP server for Neovim";
                mainProgram = name;
              };
              src = ./.;
              env = {
                GIT_COMMIT_SHA = git_commit_sha;
                GIT_DIRTY = git_dirty;
                SOURCE_DATE_EPOCH = git_last_modified;
              };
              cargoLock = {lockFile = ./Cargo.lock;};
              checkFlags = [
                "--skip=integration_tests"
              ];
            };
        };
        apps = {
          default = {
            type = "app";
            meta = {
              description = "MCP server for Neovim";
            };
            program = lib.getExe self.packages.${system}.nvim-mcp;
          };
        };
      }
    );
}
