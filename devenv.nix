{pkgs, ...}: {
  packages = [pkgs.protobuf];
  languages.rust.enable = true;
  # See full reference at https://devenv.sh/reference/options/
}
