# let
#   rust_overlay = import (builtins.fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz");
#   pkgs = import <nixpkgs> { overlays = [ rust_overlay ]; };
#   # rustVersion = "latest";
#   #rustVersion = "1.62.0";
#   rust = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default.override {
#     extensions = [ "rust-src" "rust-analyzer" "rustfmt" ];
#     targets = [ "arm-unknown-linux-gnueabihf" ];
#   });
#   # rust = pkgs.rust-bin.stable.${rustVersion}.default.override {
#   #   extensions = [
#   #     "rust-src" # for rust-analyzer
#   #     "rust-analyzer"
#   #   ];
#   # };
# in
let pkgs = import <nixpkgs> {  }; in
pkgs.mkShell {
  buildInputs = with pkgs; [
    cargo
    rustc
    rust-analyzer
    rustfmt
    papermcServers.papermc-1_21_10
  ];
}
