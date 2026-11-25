#!/bin/bash
set -e

# Btrfs configuration
BTRFS_IMG_PATH="/tmp/btrfs.img"
BTRFS_SUBVOL_NAME="geoscribefs"
BTRFS_MOUNT_POINT="/var/lib/geoscribefs/btrfs"

# ZFS configuration
ZFS_IMG_PATH="/tmp/zfs.img"
ZFS_POOL_NAME="zpool_geoscribefs"

# Detach stale loop devices left over from previous container sessions.
# Images in /tmp are recreated with a new inode on each init, so old
# attachments point to deleted inodes and are invisible to `losetup -j`.
# Devices still in use by a live mount will fail with EBUSY; ignore those.
for dev in /dev/loop[0-9]*; do
    [ -b "$dev" ] || continue
    losetup -d "$dev" 2>/dev/null || true
done

echo "Mounting Btrfs subvolume ${BTRFS_SUBVOL_NAME}..."
mkdir -p "$BTRFS_MOUNT_POINT"

if mountpoint -q "$BTRFS_MOUNT_POINT"; then
    echo "Btrfs already mounted at $BTRFS_MOUNT_POINT"
else
    mount -o loop,subvol="${BTRFS_SUBVOL_NAME}",compress=zstd "$BTRFS_IMG_PATH" "$BTRFS_MOUNT_POINT"
    echo "Successfully mounted Btrfs to $BTRFS_MOUNT_POINT"
fi

# ZFS requires the file to be attached to a loop device for the host kernel to see it.
# Check if the image is already attached to a loop device.
if ! losetup -j "$ZFS_IMG_PATH" >/dev/null 2>&1; then
    echo "Attaching ZFS image to loop device..."
    ZFS_LOOP_DEV=$(losetup -f)
    losetup "$ZFS_LOOP_DEV" "$ZFS_IMG_PATH"
fi

# Import pool if it's not already present
if ! zpool list "$ZFS_POOL_NAME" >/dev/null 2>&1; then
    echo "Importing ZFS pool ${ZFS_POOL_NAME}..."
    zpool import "$ZFS_POOL_NAME"
fi
