{
  description = "Julia installer and version multiplexer";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.11";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
        version = cargoToml.package.version;
        buildInputs = with pkgs; [
          openssl
        ] ++ (if pkgs.stdenv.isDarwin then [
          darwin.apple_sdk.frameworks.SystemConfiguration
        ] else [ ]);
      in
      {
        packages.juliaup = pkgs.rustPlatform.buildRustPackage {
          pname = "juliaup";
          inherit version;

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = with pkgs; [
            pkg-config
          ];

          inherit buildInputs;

          doCheck = false; # Disable tests

          meta = with pkgs.lib; {
            description = "Julia installer and version multiplexer";
            homepage = "https://github.com/JuliaLang/juliaup";
            license = licenses.mit;
            maintainers = with maintainers; [ sjkelly ];
          };
        };

        defaultPackage = self.packages.${system}.juliaup;

        apps.juliaup = flake-utils.lib.mkApp {
          drv = self.packages.${system}.juliaup;
        };

        defaultApp = self.apps.${system}.juliaup;
      }
    );
}
