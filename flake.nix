{
  description = "Resume analyzer — pinned Rust and Node toolchains";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    # minikube 1.38.1 ships containerd 2.x and supports Kubernetes 1.35, where
    # the ImageVolume feature is enabled by default. The nixos-25.11 nixpkgs
    # pins minikube 1.37.0 (K8s 1.34, containerd 1.x), which cannot mount an
    # OCI image as a pod volume. 26.05 is the last branch that supports
    # x86_64-darwin, which this Intel Mac needs.
    minikube-nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, minikube-nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachSystem [
      "aarch64-darwin"
      "aarch64-linux"
      "x86_64-darwin"
      "x86_64-linux"
    ] (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        lib = pkgs.lib;
        minikube = minikube-nixpkgs.legacyPackages.${system}.minikube;

        # Pinned so every worktree builds with the same compiler.
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        # Native libraries the AWS SDK and sqlx need at link time.
        buildInputs = with pkgs; [
          openssl
          pkg-config
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          packages = (with pkgs; [
            rustToolchain
            just
            sqlx-cli
            kubernetes-helm
            kubectl
            mkcert
            awscli2
            # On the default shell so `use flake` can run the web app.
            # PKG_CONFIG_PATH stays pinned below so Node's OpenSSL does not
            # steal the Rust link.
            nodejs_24
            pnpm
          ])
          # From nixos-26.05 for Kubernetes 1.35 + containerd 2.x (image
          # volumes). See the minikube-nixpkgs input above.
          ++ [ minikube ]
          ++ lib.optionals pkgs.stdenv.isDarwin [ pkgs.vfkit ]
          ++ buildInputs;

          env = {
            RUST_BACKTRACE = "1";
            PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
          };

          shellHook = ''
            # This flake is the single source of the toolchain versions that
            # Docker image builds consume. Export what the shell actually
            # provides instead of reading version files that can drift.
            export RUST_VERSION=$(rustc --version | cut -d ' ' -f 2)
            export NODEJS_VERSION=$(node --version | cut -c 2-)
            export PNPM_VERSION=$(pnpm --version)
            echo "rust $(rustc --version), node $(node --version), pnpm $(pnpm --version) — resume-analyzer dev shell"
          '';
        };

        devShells.web = pkgs.mkShell {
          packages = with pkgs; [
            nodejs_24
            pnpm
          ];
        };
      });
}
