{
  description = "Geo-replicating filesystem with a single-writer model and bare-metal write performance";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs =
    { nixpkgs, ... }:
    let
      inherit (nixpkgs) lib;
      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);

      package =
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          pname = cargoToml.package.name;
        in
        pkgs.rustPlatform.buildRustPackage {
          inherit pname;
          version = cargoToml.package.version;
          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
            allowBuiltinFetchGit = true;
          };

          nativeBuildInputs = [
            pkgs.pkg-config
            pkgs.protobuf
            pkgs.makeWrapper
          ];

          # fusermount3 is exec'd through PATH by fuse-backend-rs; the btrfs and
          # zfs backends are optional and expected on the host PATH.
          postFixup = ''
            wrapProgram "$out/bin/${pname}" --prefix PATH : ${lib.makeBinPath [ pkgs.fuse3 ]}
          '';

          # tests/ need /dev/fuse, /dev/zfs, a mounted Btrfs subvolume and a zpool.
          doCheck = false;

          meta = {
            description = "Geo-replicating filesystem with a single-writer model and bare-metal write performance";
            longDescription = "Geo-replicates Btrfs/ZFS volumes across nodes under a single-writer model: only the Scribe may write, and once write authority migrates to it the FUSE overmount detaches, leaving native filesystem throughput.";
            homepage = "https://github.com/Musholic/geoscribefs";
            license = lib.licenses.asl20;
            mainProgram = pname;
            platforms = lib.platforms.linux;
          };
        };
    in
    {
      packages = lib.genAttrs [
        "x86_64-linux"
        "aarch64-linux"
      ] (system: {
        default = package system;
      });
    };
}
