{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, naersk }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        naersk-lib = pkgs.callPackage naersk { };

        nativeBuildInputs = with pkgs; [ pkg-config alsaLib ];
        buildInputs = with pkgs; [ openssl ];
      in {
        packages.default = naersk-lib.buildPackage {
          src = ./.;
          inherit nativeBuildInputs buildInputs;
        };
        devShells.default = with pkgs;
          mkShell {
            buildInputs = [
              cargo
              pre-commit
              rust-analyzer
              rustPackages.clippy
              rustc
              rustfmt
              sea-orm-cli
            ] ++ buildInputs;
            inherit nativeBuildInputs;

            RUST_SRC_PATH = rustPlatform.rustLibSrc;
          };
      });
}
