#!/bin/bash
# Step 1: Install dependencies and compile Lustre kernel
set -e

echo "===================================================="
echo " PHASE 1: Installing Dependencies & Compiling Kernel"
echo "===================================================="

# 1. Install base build tools
echo "[+] Installing core development tools..."
sudo dnf install -y git libtool flex bison wget

# 2. Install development libraries
echo "[+] Installing required development libraries..."
sudo dnf --enablerepo=devel install -y libmount-devel libyaml-devel libnl3-devel e2fsprogs-devel

# 3. Fetch and install matching Lustre-patched kernel packages
echo "[+] Downloading and installing Lustre-patched kernel modules..."
sudo dnf install -y \
https://build.whamcloud.com/job/lustre-master/arch=x86_64,build_type=server,distro=el8.9,ib_stack=inkernel/lastSuccessfulBuild/artifact/artifacts/RPMS/x86_64/kernel-4.18.0-513.18.1.el8_lustre.x86_64.rpm \
https://build.whamcloud.com/job/lustre-master/arch=x86_64,build_type=server,distro=el8.9,ib_stack=inkernel/lastSuccessfulBuild/artifact/artifacts/RPMS/x86_64/kernel-core-4.18.0-513.18.1.el8_lustre.x86_64.rpm \
https://build.whamcloud.com/job/lustre-master/arch=x86_64,build_type=server,distro=el8.9,ib_stack=inkernel/lastSuccessfulBuild/artifact/artifacts/RPMS/x86_64/kernel-devel-4.18.0-513.18.1.el8_lustre.x86_64.rpm \
https://build.whamcloud.com/job/lustre-master/arch=x86_64,build_type=server,distro=el8.9,ib_stack=inkernel/lastSuccessfulBuild/artifact/artifacts/RPMS/x86_64/kernel-headers-4.18.0-513.18.1.el8_lustre.x86_64.rpm \
https://build.whamcloud.com/job/lustre-master/arch=x86_64,build_type=server,distro=el8.9,ib_stack=inkernel/lastSuccessfulBuild/artifact/artifacts/RPMS/x86_64/kernel-modules-4.18.0-513.18.1.el8_lustre.x86_64.rpm \
https://build.whamcloud.com/job/lustre-master/arch=x86_64,build_type=server,distro=el8.9,ib_stack=inkernel/lastSuccessfulBuild/artifact/artifacts/RPMS/x86_64/kernel-modules-internal-4.18.0-513.18.1.el8_lustre.x86_64.rpm \
https://dl.fedoraproject.org/pub/epel/8/Everything/x86_64/Packages/p/p7zip-16.02-20.el8.x86_64.rpm \
https://dl.fedoraproject.org/pub/epel/8/Everything/x86_64/Packages/q/quilt-0.66-2.el8.noarch.rpm

# 4. Patch e2fsprogs repo configuration
echo "[+] Configuring Lustre-e2fsprogs repository..."
sudo tee -a /etc/dnf/dnf.conf << 'EOF'

[Lustre-e2fsprogs]
name=Lustre-e2fsprogs
baseurl=http://downloads.whamcloud.com/public/e2fsprogs/latest/el$releasever/
gpgcheck=0
enabled=1
EOF

# Update e2fsprogs with the patched version
sudo dnf update -y e2fsprogs

# 5. Clone Lustre source code repository
echo "[+] Cloning Lustre repository..."
sudo mkdir -p /usr/src/lustre-head
sudo chmod 777 /usr/src/lustre-head
git clone git@github.com:lustre/lustre-release.git /usr/src/lustre-head

# 6. Compile and install Lustre
echo "[+] Starting automated build compilation (this will take a while)..."
cd /usr/src/lustre-head
sh autogen.sh
./configure
sudo make -j$(nproc)
sudo make install

echo "===================================================="
echo " PHASE 1 COMPLETE! REBOOTING SYSTEM NOW...         "
echo " After reboot, please run script: 2_test_lustre.sh "
echo "===================================================="
sleep 5
sudo reboot