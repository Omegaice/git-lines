{
  description = "Non-interactive git hunk staging tool";

  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    git-hooks-nix = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs =
    inputs@{
      flake-parts,
      crane,
      rust-overlay,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [
        inputs.treefmt-nix.flakeModule
        inputs.git-hooks-nix.flakeModule
      ];
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
        "x86_64-darwin"
      ];
      perSystem =
        {
          config,
          self',
          inputs',
          pkgs,
          system,
          ...
        }:
        let
          pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ (import rust-overlay) ];
          };

          rustToolchain = pkgs.rust-bin.stable.latest.default.override {
            extensions = [ "rust-src" ];
          };
          craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

          # Common source filtering for Rust projects
          # Include both Cargo sources and insta snapshot files
          unfilteredRoot = ./.;
          src = pkgs.lib.fileset.toSource {
            root = unfilteredRoot;
            fileset = pkgs.lib.fileset.unions [
              (craneLib.fileset.commonCargoSources unfilteredRoot)
              (pkgs.lib.fileset.fileFilter (file: file.hasExt "snap") unfilteredRoot)
            ];
          };

          # Common build inputs
          commonArgs = {
            inherit src;
            strictDeps = true;

            nativeBuildInputs = with pkgs; [
              pkg-config
              git # Required for e2e tests
            ];

            buildInputs =
              with pkgs;
              [
                openssl
                zlib
                libgit2
              ]
              ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
                pkgs.darwin.apple_sdk.frameworks.Security
                pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
                pkgs.libiconv
              ];

            # Use vendored libgit2
            LIBGIT2_NO_VENDOR = "0";
          };

          # Build only the dependencies (cached separately)
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;

          # Build the actual package
          git-stager = craneLib.buildPackage (
            commonArgs
            // {
              inherit cargoArtifacts;
            }
          );
        in
        {
          packages = {
            default = git-stager;
            git-stager = git-stager;
          };

          checks = {
            inherit git-stager;

            # Run clippy
            git-stager-clippy = craneLib.cargoClippy (
              commonArgs
              // {
                inherit cargoArtifacts;
                cargoClippyExtraArgs = "--all-targets -- --deny warnings";
              }
            );

            # Check formatting
            git-stager-fmt = craneLib.cargoFmt {
              inherit src;
            };

            # Run tests
            git-stager-test = craneLib.cargoTest (
              commonArgs
              // {
                inherit cargoArtifacts;
              }
            );

            # Code coverage
            git-stager-coverage = craneLib.cargoTarpaulin (
              commonArgs
              // {
                inherit cargoArtifacts;
              }
            );

            # Security audit
            git-stager-audit = craneLib.cargoAudit (
              commonArgs
              // {
                advisory-db = inputs.advisory-db;
              }
            );

            # Dependency policy check (licenses, banned crates, sources)
            git-stager-deny = craneLib.cargoDeny (commonArgs // { });

            # Generate documentation
            git-stager-doc = craneLib.cargoDoc (
              commonArgs
              // {
                inherit cargoArtifacts;
              }
            );
          };

          treefmt = {
            projectRootFile = "flake.nix";
            programs.rustfmt.enable = true;
            programs.nixfmt.enable = true;
          };

          pre-commit.settings.hooks = {
            treefmt.enable = true;
          };

          devShells.default = craneLib.devShell {
            checks = self'.checks;

            shellHook = ''
              ${config.pre-commit.shellHook}
            '';

            packages =
              with pkgs;
              [
                rust-analyzer
                cargo-watch
                cargo-edit
                cargo-insta

                git-cliff # Changelog generation
              ]
              ++ config.pre-commit.settings.enabledPackages;
          };
        };
    };
}
