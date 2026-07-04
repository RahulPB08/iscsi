use std::io::{self, Write};
use std::process::Command;
use std::fs;
use std::thread;
use std::time::Duration;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NodeConfig {
    pub ip: String,
    pub user: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClusterConfig {
    pub fs_name: String,
    pub iscsi_target: NodeConfig,
    pub mgs_mds: NodeConfig,
    pub oss: NodeConfig,
    pub client: NodeConfig,
    pub mgs_size_mb: u64,
    pub mds_size_mb: u64,
    pub oss_size_mb: u64,
    pub mgs_mount: String,
    pub mds_mount: String,
    pub oss_mount: String,
    pub client_mount: String,
}

/// Safely captures text input from the user via the terminal
pub fn get_input(prompt: &str) -> String {
    print!("{}", prompt);
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).expect("Failed to read line");
    input.trim().to_string()
}

/// Helper function to execute shell commands locally or remotely via SSH
pub fn run_node_cmd(ip: &str, user: &str, cmd: &str) -> Result<String, String> {
    if ip == "localhost" || ip == "127.0.0.1" {
        // Run locally using bash
        let output = Command::new("bash")
            .arg("-c")
            .arg(cmd)
            .output()
            .map_err(|e| format!("Failed to execute local command: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    } else {
        // Run remotely via SSH
        let ssh_dest = format!("{}@{}", user, ip);
        let output = Command::new("ssh")
            .args(&[
                "-o", "StrictHostKeyChecking=no",
                "-o", "ConnectTimeout=10",
                &ssh_dest,
                cmd
            ])
            .output()
            .map_err(|e| format!("Failed to initiate SSH connection to {}: {}", ssh_dest, e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    }
}

/// Install package on a node
fn install_package(ip: &str, user: &str, pkg_apt: &str, pkg_dnf: &str) -> Result<(), String> {
    println!("[+] Installing package (apt: {}, dnf: {}) on {}...", pkg_apt, pkg_dnf, ip);
    let has_dnf = run_node_cmd(ip, user, "which dnf").is_ok();
    let has_apt = run_node_cmd(ip, user, "which apt-get").is_ok();

    if has_dnf {
        let _ = run_node_cmd(ip, user, &format!("sudo dnf install -y {}", pkg_dnf))?;
    } else if has_apt {
        let _ = run_node_cmd(ip, user, &format!("sudo apt-get update -y && sudo apt-get install -y {}", pkg_apt))?;
    } else {
        println!("[!] Warning: Neither dnf nor apt-get found on {}. Skipping package installation.", ip);
    }
    Ok(())
}

/// Enable and start iSCSI service
fn start_initiator_service(ip: &str, user: &str) -> Result<(), String> {
    println!("[+] Starting iSCSI initiator service on {}...", ip);
    let has_systemctl = run_node_cmd(ip, user, "which systemctl").is_ok();
    if !has_systemctl {
        println!("[!] Warning: systemctl not found on {}. Skipping service startup.", ip);
        return Ok(());
    }

    // Try starting iscsid (dnf-based) or open-iscsi (apt-based)
    if run_node_cmd(ip, user, "sudo systemctl enable iscsid && sudo systemctl restart iscsid").is_ok() {
        return Ok(());
    }
    if run_node_cmd(ip, user, "sudo systemctl enable open-iscsi && sudo systemctl restart open-iscsi").is_ok() {
        return Ok(());
    }

    println!("[!] Warning: Failed to restart iSCSI services via systemctl. Assuming already active.");
    Ok(())
}

/// Load LNET and Lustre kernel modules
fn configure_lnet(ip: &str, user: &str) -> Result<(), String> {
    println!("[+] Configuring LNET modules on {}...", ip);
    let _ = run_node_cmd(ip, user, "sudo modprobe lnet 2>/dev/null || true");
    let _ = run_node_cmd(ip, user, "sudo lnetctl lnet configure 2>/dev/null || true");
    let _ = run_node_cmd(ip, user, "sudo modprobe lustre 2>/dev/null || true");
    Ok(())
}

/// Polls for a block device to appear
fn wait_for_device(ip: &str, user: &str, dev_path: &str, timeout_secs: u64) -> Result<(), String> {
    println!("[+] Waiting for block device {} to appear on {}...", dev_path, ip);
    for i in 1..=timeout_secs {
        let check_cmd = format!("test -e {} || test -b {}", dev_path, dev_path);
        if run_node_cmd(ip, user, &check_cmd).is_ok() {
            println!("[✓] Device {} detected on {} after {} seconds.", dev_path, ip, i);
            return Ok(());
        }
        thread::sleep(Duration::from_secs(1));
    }
    Err(format!(
        "Device {} did not appear on {} within {} seconds.\n\
         Please make sure the target has successfully registered LUNs and you are logged into iSCSI.",
        dev_path, ip, timeout_secs
    ))
}

/// Load configuration file if it exists
fn load_config() -> Option<ClusterConfig> {
    if let Ok(content) = fs::read_to_string("cluster_config.json") {
        if let Ok(config) = serde_json::from_str::<ClusterConfig>(&content) {
            return Some(config);
        }
    }
    None
}

/// Save configuration to file
fn save_config(config: &ClusterConfig) {
    if let Ok(content) = serde_json::to_string_pretty(config) {
        let _ = fs::write("cluster_config.json", content);
    }
}

/// Prompts with a default value
fn prompt_with_default(msg: &str, default: &str) -> String {
    print!("{} [{}]: ", msg, default);
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let trimmed = input.trim();
    if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    }
}

/// Prompts for number with default value
fn prompt_num_with_default(msg: &str, default: u64) -> u64 {
    loop {
        let val_str = prompt_with_default(msg, &default.to_string());
        if let Ok(val) = val_str.parse::<u64>() {
            return val;
        }
        println!("\x1b[31m[✗] Invalid number. Please try again.\x1b[0m");
    }
}

/// Setup new configurations interactively
fn prompt_new_config() -> ClusterConfig {
    println!("\n[i] Enter the node details for configuration. (Use 'localhost' or '127.0.0.1' for the local system)");
    
    let fs_name = prompt_with_default("Enter Lustre filesystem name", "lustre");
    
    println!("\n--- iSCSI Target Node Configuration ---");
    let iscsi_ip = prompt_with_default("iSCSI Target IP Address", "127.0.0.1");
    let iscsi_user = prompt_with_default("iSCSI Target SSH Username", "root");
    
    println!("\n--- MGS/MDS Node Configuration (Combined) ---");
    let mgs_mds_ip = prompt_with_default("MGS/MDS IP Address", "127.0.0.1");
    let mgs_mds_user = prompt_with_default("MGS/MDS SSH Username", "root");
    let mgs_mount = prompt_with_default("MGS Target Mount Path", "/mnt/mgs");
    let mgs_size_mb = prompt_num_with_default("MGS Disk Allocation Size (MB)", 2048);
    let mds_mount = prompt_with_default("MDT Target Mount Path", "/mnt/mdt");
    let mds_size_mb = prompt_num_with_default("MDT Disk Allocation Size (MB)", 10240);

    println!("\n--- OSS Node Configuration ---");
    let oss_ip = prompt_with_default("OSS IP Address", "127.0.0.1");
    let oss_user = prompt_with_default("OSS SSH Username", "root");
    let oss_mount = prompt_with_default("OST Target Mount Path", "/mnt/ost");
    let oss_size_mb = prompt_num_with_default("OST Disk Allocation Size (MB)", 20480);

    println!("\n--- Client Node Configuration ---");
    let client_ip = prompt_with_default("Client IP Address", "127.0.0.1");
    let client_user = prompt_with_default("Client SSH Username", "root");
    let client_mount = prompt_with_default("Client Mount Path", "/mnt/lustre");

    ClusterConfig {
        fs_name,
        iscsi_target: NodeConfig { ip: iscsi_ip, user: iscsi_user },
        mgs_mds: NodeConfig { ip: mgs_mds_ip, user: mgs_mds_user },
        oss: NodeConfig { ip: oss_ip, user: oss_user },
        client: NodeConfig { ip: client_ip, user: client_user },
        mgs_size_mb,
        mds_size_mb,
        oss_size_mb,
        mgs_mount,
        mds_mount,
        oss_mount,
        client_mount,
    }
}

/// Phase 1: Connects to the iSCSI target to bring the remote storage blocks online (Single-Node setup)
pub fn setup_iscsi() -> Result<(), String> {
    println!("\n=== [ Step 1: Connect to iSCSI Storage Shared Array ] ===");
    let target_ip = get_input("Enter iSCSI Target Storage IP: ");

    println!("[+] Discovering available targets on {}...", target_ip);
    let discover_cmd = format!("sudo iscsiadm -m discovery -t sendtargets -p {}", target_ip);
    run_node_cmd("localhost", "root", &discover_cmd)?;

    println!("[+] Logging into discovered targets to expose raw block drives...");
    run_node_cmd("localhost", "root", "sudo iscsiadm -m node --login")?;

    println!("[✓] iSCSI Target session established successfully.");
    Ok(())
}

/// Option 1: Configure this machine as the dedicated Management Server (MGS)
pub fn configure_mgs() -> Result<(), String> {
    println!("\n=== [ Setup Role: Management Server (MGS) ] ===");
    let fs_name = get_input("Enter Lustre Filesystem Name (e.g., lustre): ");
    let device = get_input("Enter iSCSI block device path for MGS (e.g., /dev/sdb): ");
    let mount_point = get_input("Enter local mount path (e.g., /mnt/mgs): ");

    println!("[+] Formatting device {} as Lustre MGS...", device);
    let format_cmd = format!("sudo mkfs.lustre --reformat --fsname={} --mgs {}", fs_name, device);
    run_node_cmd("localhost", "root", &format_cmd)?;

    println!("[+] Creating mount directory and mounting MGS...");
    let _ = Command::new("sudo").args(&["mkdir", "-p", &mount_point]).status();
    let mount_cmd = format!("sudo mount -t lustre {} {}", device, mount_point);
    run_node_cmd("localhost", "root", &mount_cmd)?;

    println!("[✓] MGS is online and active at {}!", mount_point);
    Ok(())
}

/// Option 2: Configure this machine as the Metadata Server (MDS)
pub fn configure_mds() -> Result<(), String> {
    println!("\n=== [ Setup Role: Metadata Server (MDS/MDT) ] ===");
    let fs_name = get_input("Enter Lustre Filesystem Name: ");
    let mgs_ip = get_input("Enter the remote MGS Node IP address: ");
    let device = get_input("Enter iSCSI block device path for MDT (e.g., /dev/sdc): ");
    let mount_point = get_input("Enter local mount path (e.g., /mnt/mdt): ");

    println!("[+] Formatting device {} as Lustre MDT...", device);
    let mgs_nid = format!("{}@tcp", mgs_ip);
    let format_cmd = format!("sudo mkfs.lustre --reformat --fsname={} --mdt --mgsnode={} --index=0 {}", fs_name, mgs_nid, device);
    run_node_cmd("localhost", "root", &format_cmd)?;

    println!("[+] Creating mount directory and mounting MDS...");
    let _ = Command::new("sudo").args(&["mkdir", "-p", &mount_point]).status();
    let mount_cmd = format!("sudo mount -t lustre {} {}", device, mount_point);
    run_node_cmd("localhost", "root", &mount_cmd)?;

    println!("[✓] MDS target index 0 registered and online at {}!", mount_point);
    Ok(())
}

/// Option 3: Configure this machine as an Object Storage Server (OSS)
pub fn configure_oss() -> Result<(), String> {
    println!("\n=== [ Setup Role: Object Storage Server (OSS/OST) ] ===");
    let fs_name = get_input("Enter Lustre Filesystem Name: ");
    let mgs_ip = get_input("Enter the remote MGS Node IP address: ");
    let device = get_input("Enter iSCSI block device path for OST (e.g., /dev/sdd): ");
    let ost_index = get_input("Enter OST Index number (e.g., 0, 1, 2): ");
    let mount_point = get_input("Enter local mount path (e.g., /mnt/ost0): ");

    println!("[+] Formatting device {} as Lustre OST index {}...", device, ost_index);
    let mgs_nid = format!("{}@tcp", mgs_ip);
    let format_cmd = format!("sudo mkfs.lustre --reformat --fsname={} --ost --mgsnode={} --index={} {}", fs_name, mgs_nid, ost_index, device);
    run_node_cmd("localhost", "root", &format_cmd)?;

    println!("[+] Creating mount directory and mounting OSS Target...");
    let _ = Command::new("sudo").args(&["mkdir", "-p", &mount_point]).status();
    let mount_cmd = format!("sudo mount -t lustre {} {}", device, mount_point);
    run_node_cmd("localhost", "root", &mount_cmd)?;

    println!("[✓] OSS Target index {} registered and online at {}!", ost_index, mount_point);
    Ok(())
}

/// Option 4: Configure this machine as a Lustre Client to access the storage pool
pub fn configure_client() -> Result<(), String> {
    println!("\n=== [ Setup Role: Lustre Client Mount ] ===");
    let mgs_ip = get_input("Enter the remote MGS Node IP address: ");
    let fs_name = get_input("Enter Lustre Filesystem Name: ");
    let mount_point = get_input("Enter Client target mount folder (e.g., /mnt/lustre): ");

    println!("[+] Creating mount directory and mounting client cluster access point...");
    let _ = Command::new("sudo").args(&["mkdir", "-p", &mount_point]).status();
    let mount_cmd = format!("sudo mount -t lustre {}@tcp:/{} {}", mgs_ip, fs_name, mount_point);
    run_node_cmd("localhost", "root", &mount_cmd)?;

    println!("[✓] Success! Lustre cluster is fully accessible via {}!", mount_point);
    Ok(())
}


/// High-level function to orchestrate the entire cluster deployment
pub fn orchestrate_cluster(config: &ClusterConfig) -> Result<(), String> {
    println!("\n\x1b[36;1m=====================================================\x1b[0m");
    println!("\x1b[36;1m         STARTING CLUSTER ORCHESTRATION              \x1b[0m");
    println!("\x1b[36;1m=====================================================\x1b[0m");

    // --- STEP 1: Prerequisites & Package Installation ---
    println!("\n\x1b[34;1m--- [Phase 1: Package Pre-requisites] ---\x1b[0m");
    
    // Install targetcli on iSCSI Target node
    println!("[i] Configuring iSCSI Target Node packages...");
    install_package(&config.iscsi_target.ip, &config.iscsi_target.user, "targetcli-fb", "targetcli-fb")?;
    
    // Start target service on iSCSI Target node
    let target_srv_cmd = "sudo systemctl enable target && sudo systemctl restart target";
    let _ = run_node_cmd(&config.iscsi_target.ip, &config.iscsi_target.user, target_srv_cmd);

    // Install initiator packages on MGS/MDS and OSS nodes
    for node in &[&config.mgs_mds, &config.oss] {
        println!("[i] Configuring iSCSI Initiator Node packages on {}...", node.ip);
        install_package(&node.ip, &node.user, "open-iscsi", "iscsi-initiator-utils")?;
        start_initiator_service(&node.ip, &node.user)?;
    }

    // --- STEP 2: Configure iSCSI Target ---
    println!("\n\x1b[34;1m--- [Phase 2: iSCSI Target Setup] ---\x1b[0m");
    let target_ip = &config.iscsi_target.ip;
    let target_user = &config.iscsi_target.user;
    
    // Create directory for iSCSI disk images
    let dir_path = "/var/lib/iscsi_disks";
    let mkdir_cmd = format!("sudo mkdir -p {} && sudo chmod 755 {}", dir_path, dir_path);
    run_node_cmd(target_ip, target_user, &mkdir_cmd)?;

    // Create MGS image
    println!("[+] Allocating MGS storage image ({} MB)...", config.mgs_size_mb);
    let mgs_img = format!("{}/lustre-mgs.img", dir_path);
    let dd_mgs = format!("sudo dd if=/dev/zero of={} bs=1M count={} && sudo chmod 666 {}", mgs_img, config.mgs_size_mb, mgs_img);
    run_node_cmd(target_ip, target_user, &dd_mgs)?;

    // Create MDS image
    println!("[+] Allocating MDS/MDT storage image ({} MB)...", config.mds_size_mb);
    let mds_img = format!("{}/lustre-mdt.img", dir_path);
    let dd_mds = format!("sudo dd if=/dev/zero of={} bs=1M count={} && sudo chmod 666 {}", mds_img, config.mds_size_mb, mds_img);
    run_node_cmd(target_ip, target_user, &dd_mds)?;

    // Create OSS/OST image
    println!("[+] Allocating OSS/OST storage image ({} MB)...", config.oss_size_mb);
    let ost_img = format!("{}/lustre-ost.img", dir_path);
    let dd_ost = format!("sudo dd if=/dev/zero of={} bs=1M count={} && sudo chmod 666 {}", ost_img, config.oss_size_mb, ost_img);
    run_node_cmd(target_ip, target_user, &dd_ost)?;

    // Configure targetcli with 3 LUNs (0: MGS, 1: MDT, 2: OST)
    println!("[+] Applying targetcli storage mapping configurations...");
    let target_iqn = "iqn.2003-01.org.linux-iscsi.rahulbhosle.x8664:lustre-target";
    let targetcli_script = format!(
        "cd /\n\
         /backstores/fileio create lustre-mgs /var/lib/iscsi_disks/lustre-mgs.img\n\
         /backstores/fileio create lustre-mdt /var/lib/iscsi_disks/lustre-mdt.img\n\
         /backstores/fileio create lustre-ost /var/lib/iscsi_disks/lustre-ost.img\n\
         /iscsi create {target_iqn}\n\
         cd /iscsi/{target_iqn}/tpg1\n\
         set attribute generate_node_acls=1\n\
         set attribute demo_mode_write_protect=0\n\
         set attribute cache_dynamic_acls=1\n\
         luns/ create /backstores/fileio/lustre-mgs\n\
         luns/ create /backstores/fileio/lustre-mdt\n\
         luns/ create /backstores/fileio/lustre-ost\n\
         cd /\n\
         saveconfig\n\
         exit\n",
        target_iqn = target_iqn
    );

    // Run targetcli script on target node
    let run_tcli_cmd = format!("echo -e '{}' | sudo targetcli", targetcli_script.replace("\n", "\\n"));
    run_node_cmd(target_ip, target_user, &run_tcli_cmd)?;
    
    // Restart target service to make sure it is updated
    let _ = run_node_cmd(target_ip, target_user, "sudo systemctl restart target");

    // --- STEP 3: Connect Initiator Nodes ---
    println!("\n\x1b[34;1m--- [Phase 3: Connecting Initiator Nodes] ---\x1b[0m");
    let target_ip_resolv = if target_ip == "localhost" || target_ip == "127.0.0.1" {
        "127.0.0.1"
    } else {
        target_ip
    };

    for node in &[&config.mgs_mds, &config.oss] {
        println!("[+] Node {} discovering target {}...", node.ip, target_ip_resolv);
        let discover_cmd = format!("sudo iscsiadm -m discovery -t sendtargets -p {}", target_ip_resolv);
        let login_cmd = "sudo iscsiadm -m node --login";
        
        let _ = run_node_cmd(&node.ip, &node.user, &discover_cmd);
        run_node_cmd(&node.ip, &node.user, login_cmd)?;
    }

    // --- STEP 4: Format and Mount Lustre Targets ---
    println!("\n\x1b[34;1m--- [Phase 4: Lustre Formatting and Mounting] ---\x1b[0m");
    
    // Deterministic SCSI paths for LUNs
    let mgs_dev = format!("/dev/disk/by-path/ip-{}:3260-iscsi-{}-lun-0", target_ip_resolv, target_iqn);
    let mds_dev = format!("/dev/disk/by-path/ip-{}:3260-iscsi-{}-lun-1", target_ip_resolv, target_iqn);
    let oss_dev = format!("/dev/disk/by-path/ip-{}:3260-iscsi-{}-lun-2", target_ip_resolv, target_iqn);

    // MGS/MDS Node - MGS Setup
    println!("\n[+] Setting up MGS on combined MGS/MDS Node ({})", config.mgs_mds.ip);
    wait_for_device(&config.mgs_mds.ip, &config.mgs_mds.user, &mgs_dev, 15)?;
    configure_lnet(&config.mgs_mds.ip, &config.mgs_mds.user)?;
    let format_mgs_cmd = format!("sudo mkfs.lustre --reformat --fsname={} --mgs {}", config.fs_name, mgs_dev);
    println!("[+] Formatting MGS device...");
    run_node_cmd(&config.mgs_mds.ip, &config.mgs_mds.user, &format_mgs_cmd)?;
    let mount_mgs_cmd = format!("sudo mkdir -p {} && sudo mount -t lustre {} {}", config.mgs_mount, mgs_dev, config.mgs_mount);
    println!("[+] Mounting MGS at {}...", config.mgs_mount);
    run_node_cmd(&config.mgs_mds.ip, &config.mgs_mds.user, &mount_mgs_cmd)?;
    println!("[✓] MGS configured and mounted successfully.");

    // MGS/MDS Node - MDS Setup
    println!("\n[+] Setting up MDS/MDT on combined MGS/MDS Node ({})", config.mgs_mds.ip);
    wait_for_device(&config.mgs_mds.ip, &config.mgs_mds.user, &mds_dev, 15)?;
    let mgs_nid = format!("{}@tcp", config.mgs_mds.ip);
    let format_mds_cmd = format!("sudo mkfs.lustre --reformat --fsname={} --mdt --mgsnode={} --index=0 {}", config.fs_name, mgs_nid, mds_dev);
    println!("[+] Formatting MDT device...");
    run_node_cmd(&config.mgs_mds.ip, &config.mgs_mds.user, &format_mds_cmd)?;
    let mount_mds_cmd = format!("sudo mkdir -p {} && sudo mount -t lustre {} {}", config.mds_mount, mds_dev, config.mds_mount);
    println!("[+] Mounting MDT at {}...", config.mds_mount);
    run_node_cmd(&config.mgs_mds.ip, &config.mgs_mds.user, &mount_mds_cmd)?;
    println!("[✓] MDS configured and mounted successfully.");

    // OSS Node Setup
    println!("\n[+] Setting up OSS Node ({})", config.oss.ip);
    wait_for_device(&config.oss.ip, &config.oss.user, &oss_dev, 15)?;
    configure_lnet(&config.oss.ip, &config.oss.user)?;
    let format_ost_cmd = format!("sudo mkfs.lustre --reformat --fsname={} --ost --mgsnode={} --index=0 {}", config.fs_name, mgs_nid, oss_dev);
    println!("[+] Formatting OST device...");
    run_node_cmd(&config.oss.ip, &config.oss.user, &format_ost_cmd)?;
    let mount_ost_cmd = format!("sudo mkdir -p {} && sudo mount -t lustre {} {}", config.oss_mount, oss_dev, config.oss_mount);
    println!("[+] Mounting OST at {}...", config.oss_mount);
    run_node_cmd(&config.oss.ip, &config.oss.user, &mount_ost_cmd)?;
    println!("[✓] OSS configured and mounted successfully.");

    // --- STEP 5: Mount Client Node ---
    println!("\n\x1b[34;1m--- [Phase 5: Mount Client Node] ---\x1b[0m");
    println!("[+] Setting up Client Node ({})", config.client.ip);
    configure_lnet(&config.client.ip, &config.client.user)?;
    let mount_client_cmd = format!("sudo mkdir -p {} && sudo mount -t lustre {}@tcp:/{} {}", config.client_mount, config.mgs_mds.ip, config.fs_name, config.client_mount);
    println!("[+] Mounting Lustre client filesystem...");
    run_node_cmd(&config.client.ip, &config.client.user, &mount_client_cmd)?;
    println!("[✓] Lustre filesystem is mounted and active at {}!", config.client_mount);

    // Run quick verification test on Client Node
    println!("\n[+] Verifying write operations on Lustre client filesystem...");
    let test_file = format!("{}/orchestrator_test.txt", config.client_mount);
    let verify_cmd = format!("echo 'Lustre cluster setup completely verified by Antigravity orchestrator!' | sudo tee {} && cat {}", test_file, test_file);
    if let Ok(res) = run_node_cmd(&config.client.ip, &config.client.user, &verify_cmd) {
        println!("[✓] Verification Successful! File write and read contents: {}", res.trim());
    } else {
        println!("[!] Warning: Write verification test failed. Check directory permissions.");
    }

    println!("\n\x1b[32;1m=====================================================\x1b[0m");
    println!("\x1b[32;1m       LUSTRE CLUSTER DEPLOYED SUCCESSFULLY!         \x1b[0m");
    println!("\x1b[32;1m=====================================================\x1b[0m");
    Ok(())
}

/// High-level function to teardown (backoff) the entire cluster deployment
pub fn teardown_cluster(config: &ClusterConfig) -> Result<(), String> {
    println!("\n\x1b[31;1m=====================================================\x1b[0m");
    println!("\x1b[31;1m         STARTING CLUSTER TEARDOWN (BACKOFF)          \x1b[0m");
    println!("\x1b[31;1m=====================================================\x1b[0m");

    // --- STEP 1: Unmount Client ---
    println!("\n\x1b[34;1m--- [Phase 1: Unmounting Client] ---\x1b[0m");
    println!("[+] Unmounting Lustre client at {}...", config.client_mount);
    let umount_client_cmd = format!("sudo umount -f {} 2>/dev/null || true", config.client_mount);
    let _ = run_node_cmd(&config.client.ip, &config.client.user, &umount_client_cmd);

    // --- STEP 2: Unmount OST ---
    println!("\n\x1b[34;1m--- [Phase 2: Unmounting OST on OSS] ---\x1b[0m");
    println!("[+] Unmounting OST at {}...", config.oss_mount);
    let umount_ost_cmd = format!("sudo umount -f {} 2>/dev/null || true", config.oss_mount);
    let _ = run_node_cmd(&config.oss.ip, &config.oss.user, &umount_ost_cmd);

    // --- STEP 3: Unmount MDT & MGS ---
    println!("\n\x1b[34;1m--- [Phase 3: Unmounting MDT and MGS on MGS/MDS Node] ---\x1b[0m");
    println!("[+] Unmounting MDT at {}...", config.mds_mount);
    let umount_mdt_cmd = format!("sudo umount -f {} 2>/dev/null || true", config.mds_mount);
    let _ = run_node_cmd(&config.mgs_mds.ip, &config.mgs_mds.user, &umount_mdt_cmd);

    println!("[+] Unmounting MGS at {}...", config.mgs_mount);
    let umount_mgs_cmd = format!("sudo umount -f {} 2>/dev/null || true", config.mgs_mount);
    let _ = run_node_cmd(&config.mgs_mds.ip, &config.mgs_mds.user, &umount_mgs_cmd);

    // --- STEP 4: Disconnect iSCSI Initiator nodes ---
    println!("\n\x1b[34;1m--- [Phase 4: Logging out of iSCSI Targets] ---\x1b[0m");
    let target_iqn = "iqn.2003-01.org.linux-iscsi.rahulbhosle.x8664:lustre-target";
    let logout_cmd = format!("sudo iscsiadm -m node -T {} --logout 2>/dev/null || true", target_iqn);
    let cleanup_node_cmd = format!("sudo iscsiadm -m node -o delete -T {} 2>/dev/null || true", target_iqn);

    for node in &[&config.mgs_mds, &config.oss] {
        println!("[+] Disconnecting iSCSI target on node {}...", node.ip);
        let _ = run_node_cmd(&node.ip, &node.user, &logout_cmd);
        let _ = run_node_cmd(&node.ip, &node.user, &cleanup_node_cmd);
    }

    // --- STEP 5: Delete iSCSI Target configurations and images ---
    println!("\n\x1b[34;1m--- [Phase 5: Cleaning up iSCSI Target & Disk Images] ---\x1b[0m");
    let target_ip = &config.iscsi_target.ip;
    let target_user = &config.iscsi_target.user;

    // targetcli teardown script
    let targetcli_teardown = format!(
        "cd /\n\
         /iscsi delete {target_iqn}\n\
         /backstores/fileio delete lustre-mgs\n\
         /backstores/fileio delete lustre-mdt\n\
         /backstores/fileio delete lustre-ost\n\
         saveconfig\n\
         exit\n",
        target_iqn = target_iqn
    );

    println!("[+] Deleting iSCSI target configurations...");
    let run_tcli_teardown = format!("echo -e '{}' | sudo targetcli 2>/dev/null || true", targetcli_teardown.replace("\n", "\\n"));
    let _ = run_node_cmd(target_ip, target_user, &run_tcli_teardown);

    // Delete image files
    println!("[+] Deleting disk image files from target storage...");
    let rm_images_cmd = "sudo rm -f /var/lib/iscsi_disks/lustre-mgs.img /var/lib/iscsi_disks/lustre-mdt.img /var/lib/iscsi_disks/lustre-ost.img";
    let _ = run_node_cmd(target_ip, target_user, rm_images_cmd);

    // Restart target service
    let _ = run_node_cmd(target_ip, target_user, "sudo systemctl restart target 2>/dev/null || true");

    println!("\n\x1b[32;1m=====================================================\x1b[0m");
    println!("\x1b[32;1m         CLUSTER TEARDOWN COMPLETE! (BACKED OFF)     \x1b[0m");
    println!("\x1b[32;1m=====================================================\x1b[0m");
    Ok(())
}

/// Orchestrator interactive launcher
pub fn run_orchestrator() {
    println!("\n\x1b[36;1m=====================================================\x1b[0m");
    println!("\x1b[36;1m        Lustre Cluster Orchestration Tool            \x1b[0m");
    println!("\x1b[36;1m=====================================================\x1b[0m");

    let mut config = if let Some(cfg) = load_config() {
        println!("[i] Existing cluster configuration loaded from cluster_config.json.");
        let use_existing = prompt_with_default("Use this existing configuration? (y/n)", "y");
        if use_existing.to_lowercase() == "y" {
            cfg
        } else {
            prompt_new_config()
        }
    } else {
        prompt_new_config()
    };

    save_config(&config);

    println!("\n\x1b[36;1m--- Configuration Summary ---\x1b[0m");
    println!("Filesystem Name:   {}", config.fs_name);
    println!("iSCSI Target:      {}@{}", config.iscsi_target.user, config.iscsi_target.ip);
    println!("MGS/MDS Node:      {}@{} (MGS Mount: {}, MDT Mount: {}, MGS Size: {} MB, MDT Size: {} MB)",
             config.mgs_mds.user, config.mgs_mds.ip, config.mgs_mount, config.mds_mount, config.mgs_size_mb, config.mds_size_mb);
    println!("OSS Node:          {}@{} (OST Mount: {}, OST Size: {} MB)", config.oss.user, config.oss.ip, config.oss_mount, config.oss_size_mb);
    println!("Client Node:       {}@{} (Client Mount: {})", config.client.user, config.client.ip, config.client_mount);
    println!("\x1b[36;1m-----------------------------\x1b[0m");

    loop {
        println!("\nSelect Orchestration Action:");
        println!("1) Start Cluster Deployment (One-Shot Setup)");
        println!("2) Cluster Teardown / Backoff (Cleanup)");
        println!("3) Re-configure Node Details");
        println!("4) Back to Main Menu");
        
        let choice = prompt_with_default("Enter choice (1-4)", "1");
        match choice.as_str() {
            "1" => {
                if let Err(e) = orchestrate_cluster(&config) {
                    eprintln!("\n\x1b[31;1m[✗] Orchestration failed: {}\x1b[0m", e);
                }
            }
            "2" => {
                let confirm = prompt_with_default("Are you sure you want to teardown the cluster? (y/n)", "n");
                if confirm.to_lowercase() == "y" {
                    if let Err(e) = teardown_cluster(&config) {
                        eprintln!("\n\x1b[31;1m[✗] Teardown failed: {}\x1b[0m", e);
                    }
                }
            }
            "3" => {
                config = prompt_new_config();
                save_config(&config);
            }
            "4" => break,
            _ => println!("Invalid selection."),
        }
    }
}