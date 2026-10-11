#!/bin/bash
set -e

IMG_PATH="/tmp/btrfs.img"
SUBVOL_NAME="geoscribefs"
MOUNT_POINT="/var/lib/geoscribefs/btrfs"

echo "Mounting Btrfs subvolume ${SUBVOL_NAME}..."
mkdir -p "$MOUNT_POINT"

# Check if already mounted to prevent errors
if mountpoint -q "$MOUNT_POINT"; then
    echo "Already mounted at $MOUNT_POINT"
else
    mount -o loop,subvol="${SUBVOL_NAME}",compress=zstd "$IMG_PATH" "$MOUNT_POINT"
    echo "Successfully mounted to $MOUNT_POINT"
fi
