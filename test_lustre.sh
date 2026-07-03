#!/bin/bash
# Step 2: Initialize loopback cluster and run verification tests
set -e

echo "===================================================="
echo " PHASE 2: Mounting Single-Node Cluster & Verifying  "
echo "===================================================="

TESTS_DIR="/usr/src/lustre-head/lustre/tests"

# 1. Enter the test directory
if [ -d "$TESTS_DIR" ]; then
    cd "$TESTS_DIR"
else
    echo "[-] Error: Lustre tests directory not found. Did Phase 1 complete?"
    exit 1
fi

# 2. Run the deployment loopback mount script
echo "[+] Running llmount.sh to spin up MGS, MDS, OSS, and Client..."
sudo ./llmount.sh

# 3. Verify filesystems are active
echo "[+] Checking current active mount targets in /mnt..."
cd /mnt
ls -l

# 4. Add the framework test configurations for optional test suites
echo "[+] Registering testing sandbox accounts (runas)..."
if ! getent group group500 >/dev/null; then
    sudo groupadd -g 500 group500
fi

if ! getent passwd runas >/dev/null; then
    sudo useradd -u 500 -g 500 runas
fi

echo "===================================================="
echo " ALL DONE! Lustre is deployed in single-node mode.  "
echo " Check mounts using: df -t lustre                  "
echo "===================================================="