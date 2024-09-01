{
  description = "Markdown Utilities";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.05";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
  }: let
    system_outputs = system: let
      pkgs = import nixpkgs {inherit system;};

      mdutils = pkgs.rustPlatform.buildRustPackage {
        pname = "mdutils";
        version = "0.0.1";
        src = ./.;
        cargoLock.lockFile = ./Cargo.lock;
        meta = {
          description = "Markdown CLI Utilities";
          homepage = "https://codeberg.org/HU90m/mdutils";
          license = with pkgs.lib.licenses; [asl20 mit];
          mainProgram = "mdsummary";
        };
      };
    in {
      formatter = pkgs.alejandra;
      packages.default = mdutils;
      apps = builtins.listToAttrs (map (binName: {
        name = binName;
        value = {
          type = "app";
          program = "${mdutils}/bin/${binName}";
        };
      }) ["mdsummary" "mdmove" "mdbook-replace"]);
    };
  in
    flake-utils.lib.eachDefaultSystem system_outputs;
}
