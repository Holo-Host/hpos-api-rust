{
  description = "Template for Holochain app development";

  inputs = {
    versions.url = "github:holochain/holochain?dir=versions/0_2";
    versions.inputs.holochain.url = "github:holochain/holochain/holochain-0.2.6";

    holochain-flake.url = "github:holochain/holochain";
    holochain-flake.inputs.versions.follows = "versions";

    nixpkgs.follows = "holochain-flake/nixpkgs";
    flake-parts.follows = "holochain-flake/flake-parts";
  };

  outputs = inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; }
      {
        systems = builtins.attrNames inputs.holochain-flake.devShells;

        perSystem =
          { inputs'
          , config
          , pkgs
          , system
          , ...
          }: {

            devShells.default = pkgs.mkShell {
              inputsFrom = [ inputs'.holochain-flake.devShells.holonix ];
              packages = [
                pkgs.nodejs-18_x
                pkgs.binaryen
                # more packages go here
              ];
            };
          };
      };
}
