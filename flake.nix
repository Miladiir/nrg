{
  description = "nrg development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs =
    { nixpkgs, ... }:
    let
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      workerHelperSources = {
        aarch64-darwin = {
          binaryenTarget = "arm64-macos";
          binaryenHash = "sha256-Sutp+3KaaRCTzDlnkB49GduH26CKWinZS1kaZWs8UDk=";
          esbuildTarget = "darwin-arm64";
          esbuildHash = "sha256-O11mynda5uh9n9Mzk8gOPe0ESa2IHpU8wKxw2SElaFc=";
        };
        aarch64-linux = {
          binaryenTarget = "aarch64-linux";
          binaryenHash = "sha256-b0DfqgmxBXNCAYEiFCuTSJVgoFct5aYvXs/fLv/+Tiw=";
          esbuildTarget = "linux-arm64";
          esbuildHash = "sha256-H7RxCz4IjqCYLb0metf1LAVS+bR60vvxT7wic65MP70=";
        };
        x86_64-linux = {
          binaryenTarget = "x86_64-linux";
          binaryenHash = "sha256-z1I2wzHvDm8z02oWA8PkLmLiFGWq1/e0vetP0+kvqlk=";
          esbuildTarget = "linux-x64";
          esbuildHash = "sha256-mNJ9DRbUJVRMwdfL6p7HEdcT6+ZRyqqAJGjInjHz1Sg=";
        };
      };
    in
    {
      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          helperSources = workerHelperSources.${system};
          binaryen = pkgs.fetchzip {
            name = "binaryen-130-${system}";
            url = "https://github.com/WebAssembly/binaryen/releases/download/version_130/binaryen-version_130-${helperSources.binaryenTarget}.tar.gz";
            hash = helperSources.binaryenHash;
          };
          esbuild = pkgs.fetchzip {
            name = "esbuild-0.28.1-${system}";
            url = "https://registry.npmjs.org/@esbuild/${helperSources.esbuildTarget}/-/${helperSources.esbuildTarget}-0.28.1.tgz";
            hash = helperSources.esbuildHash;
          };
          pnpm = pkgs.pnpm.override {
            version = "11.21.0";
            hash = "sha256-hyN9N+rbedxiagV26zpS0j1wQiwyOuXgD8BckfQyN4A=";
            nodejs-slim = pkgs.nodejs-slim_24;
          };
        in
        {
          default = pkgs.mkShell {
            packages = [
              binaryen
              esbuild
              pkgs.cargo
              pkgs.clippy
              pkgs.curl
              pkgs.docker-client
              pkgs.gnumake
              pkgs.lld
              pkgs.nodejs_24
              pkgs.rustc
              pkgs.rustfmt
              pkgs.wasm-bindgen-cli_0_2_126
              pkgs.wasm-pack
              pkgs.worker-build
              pnpm
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.pkg-config ];

            buildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.openssl ];

            ESBUILD_BIN = "${esbuild}/bin/esbuild";
            WASM_BINDGEN_BIN = pkgs.lib.getExe' pkgs.wasm-bindgen-cli_0_2_126 "wasm-bindgen";
            WASM_OPT_BIN = "${binaryen}/bin/wasm-opt";
          };
        }
      );
    };
}
