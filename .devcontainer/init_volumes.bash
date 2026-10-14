#!/bin/bash
set -e

# Btrfs configuration
BTRFS_IMG_PATH="/tmp/btrfs.img"
BTRFS_SUBVOL_NAME="geoscribefs"
BTRFS_TEMP_MOUNT="/tmp/btrfs_init"

# ZFS configuration
ZFS_IMG_PATH="/tmp/zfs.img"
ZFS_POOL_NAME="zpool_geoscribefs"
ZFS_DATASET_NAME="geoscribefs"

echo "Creating and formatting Btrfs image..."
truncate -s 2G "$BTRFS_IMG_PATH"
mkfs.btrfs -f "$BTRFS_IMG_PATH"

echo "Creating initial Btrfs subvolume ${BTRFS_SUBVOL_NAME}..."
mkdir -p "$BTRFS_TEMP_MOUNT"
mount -o loop "$BTRFS_IMG_PATH" "$BTRFS_TEMP_MOUNT"
btrfs subvolume create "${BTRFS_TEMP_MOUNT}/${BTRFS_SUBVOL_NAME}"
umount "$BTRFS_TEMP_MOUNT"
rmdir "$BTRFS_TEMP_MOUNT"

echo "Btrfs initialization complete."

echo "Creating and formatting ZFS image..."
truncate -s 2G "$ZFS_IMG_PATH"

# Ensure existing pool is destroyed to allow re-initialization
if zpool list "$ZFS_POOL_NAME" >/dev/null 2>&1; then
    echo "Destroying existing ZFS pool ${ZFS_POOL_NAME}..."
    zpool destroy "$ZFS_POOL_NAME"
fi

# ZFS requires a block device. We create a loop device for the image file.
# This allows the host kernel's ZFS module to see the storage.
ZFS_LOOP_DEV=$(losetup -f)
losetup "$ZFS_LOOP_DEV" "$ZFS_IMG_PATH"

zpool create "$ZFS_POOL_NAME" "$ZFS_LOOP_DEV"

echo "ZFS initialization complete."
