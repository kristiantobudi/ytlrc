{
  description = "ytlrc — synced YouTube lyrics in your terminal";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "ytlrc";
          version = "0.2.0";
          src = pkgs.lib.cleanSource ./.;
          cargoLock = ./Cargo.lock;
          nativeBuildInputs = [ pkgs.pkg-config pkgs.makeWrapper ];
          buildInputs = [ pkgs.curl ];
          # curl is a runtime dependency (ytlrc shells out to the `curl`
          # binary for LRCLIB requests instead of linking a TLS stack).
          postInstall = ''
            wrapProgram $out/bin/ytlrc --prefix PATH : ${pkgs.curl}/bin
          '';
          meta = with pkgs.lib; {
            description = "Synced YouTube lyrics in your terminal";
            homepage = "https://github.com/kristiantobudi/ytlrc";
            license = licenses.mit;
            mainProgram = "ytlrc";
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [ pkgs.cargo pkgs.rustc pkgs.rustfmt pkgs.clippy pkgs.curl ];
        };
      });
}
