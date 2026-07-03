#!/bin/bash

# --- Color Definitions for Output Formatting ---
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== iSCSI Target Installation Script ===${NC}"

# 1. Root Privileges Check
if [[ $EUID -ne 0 ]]; then
   echo -e "${RED}Error: This script must be run as root. Please use: sudo ./install_iscsi.sh${NC}" 
   exit 1
fi

# 2. Update Package Index
echo -e "\n${GREEN}[1/3] Updating package repositories...${NC}"
apt-get update -y

# 3. Install targetcli-fb Package
echo -e "\n${GREEN}[2/3] Installing targetcli-fb...${NC}"
apt-get install targetcli-fb -y

# Verify if installation succeeded
if [ $? -eq 0 ]; then
    echo -e "${GREEN}targetcli-fb installed successfully!${NC}"
else
    echo -e "${RED}Error: Package installation failed.${NC}"
    exit 1
fi

# 4. Enable and Start the Target Kernel Service
echo -e "\n${GREEN}[3/3] Activating and starting the 'target' background service...${NC}"
systemctl enable target
systemctl restart target

# Verify service runtime status
if systemctl is-active --quiet target; then
    echo -e "${GREEN}Service 'target' is active and running cleanly!${NC}"
else
    echo -e "${RED}Error: Failed to start the 'target' system service.${NC}"
    exit 1
fi

echo -e "\n${GREEN}=====================================================${NC}"
echo -e "${GREEN} SUCCESS: iSCSI Target Engine is ready to go!${NC}"
echo -e " You can now run 'sudo targetcli' or use your Rust automation tool."
echo -e "${GREEN}=====================================================${NC}"