{
  description = "Sakamoto — pipeline-oriented coding agent orchestrator";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable."1.85.0".default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
          targets = [ "wasm32-unknown-unknown" ];
        };

        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
        ];

        buildInputs = with pkgs; [
          openssl
        ] ++ lib.optionals stdenv.isDarwin [
          darwin.apple_sdk.frameworks.Security
          darwin.apple_sdk.frameworks.SystemConfiguration
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          inherit nativeBuildInputs buildInputs;

          packages = with pkgs; [
            cargo-watch
            cargo-edit
            cargo-audit

            pre-commit
            jq
            ripgrep
            fd
          ];

          shellHook = ''
            echo "=== Sakamoto Development Shell ==="
            echo "Rust: $(rustc --version)"
            echo ""
            echo "Commands:"
            echo "  cargo build            Build all crates"
            echo "  cargo test --workspace  Run all tests"
            echo "  cargo clippy            Lint"
            echo ""
          '';
        };

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "sakamoto";
          version = "0.1.0";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
            allowBuiltinFetchGit = true;
          };

          inherit nativeBuildInputs buildInputs;

          meta = with pkgs.lib; {
            description = "Pipeline-oriented coding agent orchestrator";
            homepage = "https://github.com/Industrial-Algebra/Sakamoto";
            license = licenses.mit;
          };
        };

        formatter = pkgs.nixpkgs-fmt;
      }
    );
}
