{pkgs, ...}: {
  packages = [pkgs.protobuf pkgs.fuse];
  languages.rust.enable = true;

  env.RUST_LOG = "debug,geoscribefs=trace";
  # See full reference at https://devenv.sh/reference/options/
}
