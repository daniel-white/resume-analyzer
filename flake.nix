{
  description = "Resume analyzer — pinned Rust and Node toolchains";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        lib = pkgs.lib;

        # Pinned so every worktree builds with the same compiler.
        rustToolchain = pkgs.rust-bin.stable."1.99.1".default.override {
          extensions = [ "rust-src" "rustfmt" "clippy" ];
        };

        # Native libraries the AWS SDK and sqlx need at link time.
        buildInputs = with pkgs; [
          openssl
          pkg-config
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            rustToolchain
            just
            sqlx-cli
            kubernetes-helm
            kubectl
            minikube
            mkcert
            awscli2
          ]
          ++ lib.optionals pkgs.stdenv.isDarwin [ pkgs.vfkit ]
          ++ buildInputs;

          env = {
            RUST_BACKTRACE = "1";
            PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
          };

          shellHook = ''
            echo "rust $(rustc --version) — resume-analyzer dev shell"
          '';
        };

        devShells.web = pkgs.mkShell {
          packages = with pkgs; [
            nodejs_22
            pnpm
          ];
        };
      });
}
