#!/bin/bash
set -e

# CONFIGURATION
DISK_IMG="disk.img"
DISK_SIZE_MB=32          # adjust to your original disk size
KERNEL_BIN="./target/x86_64-sos/debug/bootimage-sos.bin"  # path to your compiled kernel
FAT_OFFSET=$((1024*1024)) # 1 MiB
MOUNT_DIR="/mnt"

# 1️⃣ Create a blank disk
echo "[1/5] Creating blank disk ($DISK_SIZE_MB MB)..."
dd if=/dev/zero of="$DISK_IMG" bs=1M count=$DISK_SIZE_MB

# 2️⃣ Write the raw kernel at the beginning
echo "[2/5] Writing kernel ($KERNEL_BIN) at start of disk..."
dd if="$KERNEL_BIN" of="$DISK_IMG" bs=512 conv=notrunc

# 3️⃣ Set up loop device for FAT partition
echo "[3/5] Setting up loop device at offset $FAT_OFFSET..."
LOOP_DEV=$(sudo losetup -fP --show -o $FAT_OFFSET "$DISK_IMG")
echo " -> Loop device: $LOOP_DEV"

# 4️⃣ Format FAT16 filesystem
echo "[4/5] Formatting FAT16 on loop device..."
sudo mkfs.fat -F16 "$LOOP_DEV"

# 5️⃣ Mount FAT16, copy files, unmount
echo "[5/5] Mounting FAT16 to copy files..."
sudo mount "$LOOP_DEV" "$MOUNT_DIR"

# Copy your programs (adjust path if needed)
echo " -> Copying programs..."
sudo cp -r programs/* "$MOUNT_DIR/"

echo "✅ disk.img recreated successfully!"

