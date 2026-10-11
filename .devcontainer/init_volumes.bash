#!/bin/bash
set -e

IMG_PATH="/tmp/btrfs.img"
SUBVOL_NAME="geoscribefs"
TEMP_MOUNT="/tmp/btrfs_init"

echo "Creating and formatting Btrfs image..."
truncate -s 2G "$IMG_PATH"
mkfs.btrfs -f "$IMG_PATH"

echo "Creating initial subvolume ${SUBVOL_NAME}..."
mkdir -p "$TEMP_MOUNT"
mount -o loop "$IMG_PATH" "$TEMP_MOUNT"
btrfs subvolume create "${TEMP_MOUNT}/${SUBVOL_NAME}"
umount "$TEMP_MOUNT"
rmdir "$TEMP_MOUNT"

echo "Btrfs initialization complete."
