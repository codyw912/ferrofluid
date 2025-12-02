{
  inputs = {
    # nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    # Use stable nixpkgs to avoid broken builds in unstable (clang version incompatible with some tool versions as of 2025-10-09)
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    devenv.url = "github:cachix/devenv";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  nixConfig = {
    extra-trusted-public-keys = "devenv.cachix.org-1:w1cLUi8dv3hnoSPGAuibQv+f9TZLr6cv/Hm9XgU50cw=";
    extra-substituters = "https://devenv.cachix.org";
  };

  outputs = { self, nixpkgs, devenv, ... } @ inputs:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];
      forEachSystem = f: builtins.listToAttrs (map (name: { inherit name; value = f name; }) systems);
    in
    {
      devShells = forEachSystem (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = devenv.lib.mkShell {
            inherit inputs pkgs;
            modules = [
              ./devenv.nix
            ];
          };
        }
      );

      # Template definition for nix flake init
      templates.default = {
        path = ./.;
        description = "Rust development environment with devenv";
        welcomeText = ''
          🦀 Rust development environment initialized!
          
          Files created:
          - devenv.nix: Development environment configuration
          - .cargo/config.toml: Cargo linker configuration for macOS
          - flake.nix: Nix flake configuration
          
          Next steps:
          1. Run 'direnv allow' to activate the environment
          2. Run 'cargo init' to initialize a new Rust project (if needed)
        '';
      };
    };
}
