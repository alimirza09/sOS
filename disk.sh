#!/bin/bash
set -e

IMG=disk.img
SIZE=64M

qemu-img create "$IMG" "$SIZE"

parted "$IMG" --script -- mklabel msdos mkpart primary fat32 2048s 100%

OFFSET=$((2048 * 512))
LOOP=$(sudo losetup --find --show --offset $OFFSET "$IMG")

sudo mkfs.fat "$LOOP"

sudo losetup -d "$LOOP"

echo "Done $IMG is ready!"
