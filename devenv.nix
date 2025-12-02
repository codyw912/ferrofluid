{ pkgs, lib, config, inputs, ... }:

{
  # https://devenv.sh/basics/
  env = {
    CARGO_HOME = "${config.env.DEVENV_STATE}/cargo";
    CARGO_TARGET_DIR = "${config.env.DEVENV_STATE}/target";
    RUST_BACKTRACE = "1";
  } // lib.optionalAttrs pkgs.stdenv.isDarwin {
    # Help linker find system libraries on macOS
    LIBRARY_PATH = lib.makeLibraryPath (with pkgs; [ libiconv zlib openssl ]);
  };
  
  # https://devenv.sh/packages/
  packages = with pkgs; [
    git
    just  # command runner
    # Native build dependencies for linking
    pkg-config
    openssl
    zlib
    libiconv  # Explicit libiconv for reliable macOS linking
  ];

  # Rust environment
  languages.rust = {
    enable = true;
    channel = "stable";
    components = [ "rustc" "cargo" "clippy" "rustfmt" "rust-analyzer" ];
    targets = [ "aarch64-apple-darwin" "aarch64-unknown-linux-gnu" ];
  };

  # Enable C support for proper compiler wrapper (handles macOS SDK)
  languages.c.enable = true;

  # https://devenv.sh/scripts/
  scripts = {
    test.exec = "cargo test";
    check.exec = "cargo check";
    build.exec = "cargo build";
    run.exec = "cargo run";
    fmt.exec = "cargo fmt";
    lint.exec = "cargo clippy";
  };

  # https://devenv.sh/tasks/
  tasks = {
    "cargo:check" = {
      exec = ''
        if [ -f "Cargo.toml" ]; then
          cargo check || echo "⚠️  Cargo check failed, but continuing with shell startup..."
        fi
      '';
      # Removed 'before = [ "devenv:enterShell" ]' so compile errors don't block shell startup
    };
  };

  # https://devenv.sh/reference/options/#git-hooks
  git-hooks.hooks = {
    rustfmt.enable = true;
    clippy.enable = true;
  };

  # Enter shell message
  enterShell = ''
    echo "🦀 Rust development environment activated"
    echo "Available commands:"
    echo "  - run      # cargo run"
    echo "  - build    # cargo build"
    echo "  - test     # cargo test"
    echo "  - check    # cargo check"
    echo "  - fmt      # cargo fmt"
    echo "  - lint     # cargo clippy"
    echo ""
    if [ ! -f "Cargo.toml" ]; then
      echo "💡 Create a new project with: cargo init"
    fi
  '';
}
