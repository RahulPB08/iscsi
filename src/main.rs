mod dcv;

use std::io::{self, Write};
use std::process::{Command, Stdio};

fn main() {
    println!("\x1b[36;1m=========================================\x1b[0m");
    println!("\x1b[36;1m   Lustre & iSCSI Storage Orchestrator   \x1b[0m");
    println!("\x1b[36;1m=========================================\x1b[0m");

    // Check Privilege Level
    let root = is_root();
    if !root {
        println!("\x1b[33m[!] Warning: You are not running as root. We will use 'sudo' for administrative commands.\x1b[0m\n");
    } else {
        println!("\x1b[32m[✓] Running with root privileges.\x1b[0m\n");
    }

    loop {
        println!("\n\x1b[36;1m--- Main Menu ---\x1b[0m");
        println!("1. Create iSCSI Target (Local Target Server)");
        println!("2. Delete iSCSI Target (Local Target Server)");
        println!("3. Configure Lustre MGS Server");
        println!("4. Configure Lustre MDS Server");
        println!("5. Configure Lustre OSS Server (Object Storage Server / OST)");
        println!("6. Connect as Storage Client (Mount LustreFS)");
        println!("7. Unmount & Release Storage (Safe Teardown)");
        println!("8. Multi-Node Cluster Orchestrator (One-Shot Deploy/Teardown)");
        println!("9. Exit");
        println!("\x1b[36;1m-----------------\x1b[0m");
        let choice = prompt("Enter your choice (1-9): ");
        match choice.trim() {
            "1" => {
                if ensure_targetcli() {
                    setup_target();
                }
            }
            "2" => {
                if ensure_targetcli() {
                    delete_target();
                }
            }
            "3" => {
                if ensure_iscsi_initiator() {
                    let _ = dcv::setup_iscsi();
                    if let Err(e) = dcv::configure_mgs() {
                        println!("\x1b[31m[✗] Failed to configure MGS: {}\x1b[0m", e);
                    }
                }
            }
            "4" => {
                if ensure_iscsi_initiator() {
                    let _ = dcv::setup_iscsi();
                    if let Err(e) = dcv::configure_mds() {
                        println!("\x1b[31m[✗] Failed to configure MDS: {}\x1b[0m", e);
                    }
                }
            }
            "5" => {
                if ensure_iscsi_initiator() {
                    let _ = dcv::setup_iscsi();
                    if let Err(e) = dcv::configure_oss() {
                        println!("\x1b[31m[✗] Failed to configure OSS: {}\x1b[0m", e);
                    }
                }
            }
            "6" => {
                if let Err(e) = dcv::configure_client() {
                    println!("\x1b[31m[✗] Failed to mount Client: {}\x1b[0m", e);
                }
            }
            "7" => {
                unmount_and_release();
            }
            "8" => {
                dcv::run_orchestrator();
            }
            "9" => {
                println!("Exiting. Goodbye!");
                break;
            }
            _ => println!("\x1b[31m[✗] Invalid choice. Please enter a choice between 1 and 9.\x1b[0m"),
        }
    }
}

// Check and install targetcli if needed
fn ensure_targetcli() -> bool {
    if command_exists("targetcli") {
        return true;
    }
    println!("\n\x1b[33m[!] targetcli is not installed on this system.\x1b[0m");

    // Auto-detect package manager
    let has_dnf = command_exists("dnf");
    let has_apt = command_exists("apt-get");

    let pkg_mgr = if has_dnf {
        "dnf"
    } else if has_apt {
        "apt"
    } else {
        println!("\x1b[31m[✗] Neither dnf nor apt-get found on this system. Please install targetcli-fb manually.\x1b[0m");
        return false;
    };

    let install = prompt(&format!("Would you like to install targetcli-fb using {} now? (y/n) [y]: ", pkg_mgr));
    if install.to_lowercase() == "n" {
        println!("\x1b[31m[✗] targetcli-fb is required to proceed.\x1b[0m");
        return false;
    }

    if pkg_mgr == "dnf" {
        println!("\x1b[34m[i] Installing targetcli-fb via dnf...\x1b[0m");
        if run_command_with_log(
            "dnf install targetcli-fb",
            "dnf",
            &["install", "-y", "targetcli-fb"],
            true,
        ) {
            println!("\x1b[32m[✓] targetcli-fb installed successfully!\x1b[0m");
            true
        } else {
            println!("\x1b[31m[✗] Failed to install targetcli-fb.\x1b[0m");
            false
        }
    } else {
        println!("\x1b[34m[i] Updating package repository...\x1b[0m");
        if !run_command_with_log("apt-get update", "apt-get", &["update"], true) {
            println!("\x1b[31m[✗] Failed to update package repository.\x1b[0m");
            return false;
        }
        println!("\x1b[34m[i] Installing targetcli-fb via apt...\x1b[0m");
        if run_command_with_log(
            "apt-get install targetcli-fb",
            "apt-get",
            &["install", "-y", "targetcli-fb"],
            true,
        ) {
            println!("\x1b[32m[✓] targetcli-fb installed successfully!\x1b[0m");
            true
        } else {
            println!("\x1b[31m[✗] Failed to install targetcli-fb.\x1b[0m");
            false
        }
    }
}

// Check and install iSCSI Initiator if needed
fn ensure_iscsi_initiator() -> bool {
    if command_exists("iscsiadm") {
        // Ensure services are started
        let service_name = if command_exists("dnf") { "iscsid" } else { "open-iscsi" };
        let _ = run_command_with_log(&format!("Enable {} service", service_name), "systemctl", &["enable", service_name], true);
        let _ = run_command_with_log(&format!("Start {} service", service_name), "systemctl", &["restart", service_name], true);
        return true;
    }
    println!("\n\x1b[33m[!] iSCSI initiator (iscsiadm) is not installed on this system.\x1b[0m");

    // Auto-detect package manager
    let has_dnf = command_exists("dnf");
    let has_apt = command_exists("apt-get");

    let pkg_mgr = if has_dnf {
        "dnf"
    } else if has_apt {
        "apt"
    } else {
        println!("\x1b[31m[✗] Neither dnf nor apt-get found. Please install iscsi initiator manually.\x1b[0m");
        return false;
    };

    let install = prompt(&format!("Would you like to install the iSCSI initiator using {} now? (y/n) [y]: ", pkg_mgr));
    if install.to_lowercase() == "n" {
        println!("\x1b[31m[✗] iSCSI initiator is required to proceed.\x1b[0m");
        return false;
    }

    if pkg_mgr == "dnf" {
        if run_command_with_log(
            "dnf install iscsi-initiator-utils",
            "dnf",
            &["install", "-y", "iscsi-initiator-utils"],
            true,
        ) {
            let _ = run_command_with_log("Enable iscsid service", "systemctl", &["enable", "iscsid"], true);
            let _ = run_command_with_log("Start iscsid service", "systemctl", &["restart", "iscsid"], true);
            true
        } else {
            println!("\x1b[31m[✗] Failed to install iscsi-initiator-utils.\x1b[0m");
            false
        }
    } else {
        if run_command_with_log(
            "apt-get install open-iscsi",
            "apt-get",
            &["install", "-y", "open-iscsi"],
            true,
        ) {
            let _ = run_command_with_log("Enable open-iscsi service", "systemctl", &["enable", "open-iscsi"], true);
            let _ = run_command_with_log("Start open-iscsi service", "systemctl", &["restart", "open-iscsi"], true);
            true
        } else {
            println!("\x1b[31m[✗] Failed to install open-iscsi.\x1b[0m");
            false
        }
    }
}

// Unmount and release storage
fn unmount_and_release() {
    println!("\n\x1b[36;1m=========================================\x1b[0m");
    println!("\x1b[36;1m       Unmount & Release Storage         \x1b[0m");
    println!("\x1b[36;1m=========================================\x1b[0m");

    // 1. Detect active Lustre mount points and unmount them
    let active_mounts = get_active_lustre_mounts();
    if active_mounts.is_empty() {
        println!("[i] No active Lustre filesystem mounts detected.");
    } else {
        println!("[i] Detected active Lustre mounts: {:?}", active_mounts);
        let confirm = prompt("Would you like to unmount all detected Lustre filesystems? (y/n) [y]: ");
        if confirm.to_lowercase() != "n" {
            for mnt in active_mounts {
                println!("[+] Unmounting {}...", mnt);
                if run_command_with_log(&format!("Unmount {}", mnt), "umount", &["-f", &mnt], true) {
                    println!("\x1b[32m[✓] Unmounted {} successfully.\x1b[0m", mnt);
                } else {
                    println!("\x1b[33m[!] Failed to unmount {}. You might need to manually stop processes using the mount.\x1b[0m", mnt);
                }
            }
        }
    }

    // 2. Disconnect/logout from active iSCSI targets
    if command_exists("iscsiadm") {
        // Check if there are active sessions
        let check_session = Command::new("sudo").args(&["iscsiadm", "-m", "session"]).output();
        let has_sessions = match check_session {
            Ok(output) => output.status.success() && !output.stdout.is_empty(),
            Err(_) => false,
        };

        if has_sessions {
            println!("\n[i] Active iSCSI sessions detected.");
            let confirm = prompt("Would you like to log out of all active iSCSI sessions? (y/n) [y]: ");
            if confirm.to_lowercase() != "n" {
                println!("[+] Logging out of active iSCSI sessions...");
                if run_command_with_log("iSCSI logout", "iscsiadm", &["-m", "node", "--logout"], true) {
                    println!("\x1b[32m[✓] Logged out of iSCSI sessions successfully.\x1b[0m");
                } else {
                    println!("\x1b[31m[✗] Failed to logout of iSCSI sessions.\x1b[0m");
                }
            }
        }
    }

    // 3. Optional local target deletion
    if command_exists("targetcli") {
        println!("\n[i] This system has targetcli installed.");
        let confirm = prompt("Would you like to delete a local iSCSI target and free storage now? (y/n) [n]: ");
        if confirm.to_lowercase() == "y" {
            delete_target();
        }
    }

    println!("\n\x1b[32;1m[✓] Unmount & Release routine complete!\x1b[0m");
}

fn get_active_lustre_mounts() -> Vec<String> {
    let mut mounts = Vec::new();
    if let Ok(content) = std::fs::read_to_string("/proc/mounts") {
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 && parts[2] == "lustre" {
                mounts.push(parts[1].to_string());
            }
        }
    }
    mounts
}

fn restart_target_service() -> bool {
    println!("\n\x1b[34m[i] Restarting target service...\x1b[0m");
    if run_command_with_log("systemctl restart target", "systemctl", &["restart", "target"], true) {
        return true;
    }
    println!("\x1b[33m[!] Note: 'target' service restart failed. Trying 'rtslib-fb-targetctl'...\x1b[0m");
    if run_command_with_log(
        "systemctl restart rtslib-fb-targetctl",
        "systemctl",
        &["restart", "rtslib-fb-targetctl"],
        true,
    ) {
        return true;
    }
    println!("\x1b[33m[!] Note: Trying 'targetcli' service...\x1b[0m");
    if run_command_with_log(
        "systemctl restart targetcli",
        "systemctl",
        &["restart", "targetcli"],
        true,
    ) {
        return true;
    }
    false
}

// Setup a new iSCSI Target
fn setup_target() {
    // Gather inputs
    let userid = prompt_non_empty("Enter User ID (used for target/initiator names): ");
    let password = prompt_password_valid("Enter Password (12-16 characters): ");
    let username = prompt_non_empty("Enter Username (used for image filename and backstore): ");
    let size_mb = prompt_size("Enter Disk Size in MB [default: 1000]: ", 1000);

    // Confirm Details
    println!("\n\x1b[36;1m--- Configuration Summary ---\x1b[0m");
    println!("User ID:       {}", userid);
    println!("Username:      {}", username);
    println!("Password:      {}", "*".repeat(password.len()));
    println!("Disk Size:     {} MB", size_mb);
    println!("\x1b[36;1m-----------------------------\x1b[0m");
    
    let confirm = prompt("Do you want to proceed with this configuration? (y/n) [y]: ");
    if confirm.to_lowercase() == "n" {
        println!("\x1b[31m[✗] Aborted by user.\x1b[0m");
        return;
    }

    // Step 1: sudo mkdir /var/lib/iscsi_disks
    let dir_path = "/var/lib/iscsi_disks";
    if !std::path::Path::new(dir_path).exists() {
        if !run_command_with_log(
            "Create iscsi_disks directory",
            "mkdir",
            &["-p", dir_path],
            true,
        ) {
            return;
        }
    } else {
        println!("\x1b[34m[i] Directory {} already exists.\x1b[0m", dir_path);
    }

    // Step 2: sudo chmod 755 /var/lib/iscsi_disks
    if !run_command_with_log(
        "Set permissions on directory to 755",
        "chmod",
        &["755", dir_path],
        true,
    ) {
        return;
    }

    // Step 3: sudo dd if=/dev/zero of=/var/lib/iscsi_disks/{username.img} bs=1M count={1000}
    let img_path = format!("{}/{}.img", dir_path, username);
    let mut create_img = true;
    if std::path::Path::new(&img_path).exists() {
        let overwrite = prompt(&format!(
            "\x1b[33m[!] Warning: Image file {} already exists. Overwrite? (y/n) [n]: \x1b[0m",
            img_path
        ));
        if overwrite.to_lowercase() != "y" {
            create_img = false;
            println!("\x1b[34m[i] Skipping image file creation, using existing file.\x1b[0m");
        }
    }

    if create_img {
        let of_arg = format!("of={}", img_path);
        let count_arg = format!("count={}", size_mb);
        if !run_command_with_log(
            &format!("Create image file via dd ({} MB)", size_mb),
            "dd",
            &["if=/dev/zero", &of_arg, "bs=1M", &count_arg],
            true,
        ) {
            return;
        }
    }

    // Step 4: sudo chmod 666 /var/lib/iscsi_disks/{username.img}
    if !run_command_with_log(
        "Set permissions on image file to 666",
        "chmod",
        &["666", &img_path],
        true,
    ) {
        return;
    }

    // Step 5: Configure targetcli
    let targetcli_script = format!(
        "cd /\n\
         /backstores/fileio create {username} /var/lib/iscsi_disks/{username}.img\n\
         /iscsi create iqn.2003-01.org.linux-iscsi.rahulbhosle.x8664:{userid}\n\
         cd /iscsi/iqn.2003-01.org.linux-iscsi.rahulbhosle.x8664:{userid}/tpg1\n\
         set attribute generate_node_acls=0\n\
         acls/ create iqn.1991-05.com.microsoft:{userid}\n\
         cd acls/iqn.1991-05.com.microsoft:{userid}\n\
         set auth userid={userid}\n\
         set auth password={password}\n\
         cd /iscsi/iqn.2003-01.org.linux-iscsi.rahulbhosle.x8664:{userid}/tpg1\n\
         luns/ create /backstores/fileio/{username}\n\
         cd /\n\
         saveconfig\n\
         exit\n",
        username = username,
        userid = userid,
        password = password
    );

    if !run_targetcli_script(&targetcli_script) {
        println!("\x1b[31m[✗] Targetcli configuration failed.\x1b[0m");
        return;
    }

    // Step 6: Restart target service
    if restart_target_service() {
        println!("\n\x1b[32;1m[✓] Setup completed successfully!\x1b[0m");
    } else {
        println!("\n\x1b[33;1m[!] Configuration completed, but failed to restart any target service. Please verify the target systemd service manually.\x1b[0m");
    }
}

// Delete target and free space
fn delete_target() {
    println!("\n\x1b[36;1m--- Delete iSCSI Target & Free Space ---\x1b[0m");
    let userid = prompt_non_empty("Enter User ID of target to delete: ");
    let username = prompt_non_empty("Enter Username (used for image filename and backstore): ");

    println!("\n\x1b[33;1m--- Deletion Warning ---\x1b[0m");
    println!("This will delete:");
    println!("1. iSCSI Target:  iqn.2003-01.org.linux-iscsi.rahulbhosle.x8664:{}", userid);
    println!("2. Backstore:     {}", username);
    println!("3. Disk Image:    /var/lib/iscsi_disks/{}.img (FREEING SPACE)", username);
    println!("\x1b[33;1m------------------------\x1b[0m");
    
    let confirm = prompt("\x1b[31;1mAre you absolutely sure you want to delete this target? (y/n) [n]: \x1b[0m");
    if confirm.to_lowercase() != "y" {
        println!("\x1b[31m[✗] Deletion aborted by user.\x1b[0m");
        return;
    }

    // Step 1: Run targetcli script to delete target and backstore
    let targetcli_script = format!(
        "cd /\n\
         /iscsi delete iqn.2003-01.org.linux-iscsi.rahulbhosle.x8664:{userid}\n\
         /backstores/fileio delete {username}\n\
         saveconfig\n\
         exit\n",
        userid = userid,
        username = username
    );

    println!("\x1b[34m[i] Removing target configuration from targetcli...\x1b[0m");
    let _ = run_targetcli_script(&targetcli_script);

    // Step 2: Delete image file to free space
    let img_path = format!("/var/lib/iscsi_disks/{}.img", username);
    if std::path::Path::new(&img_path).exists() {
        println!("\x1b[34m[i] Deleting image file {} to free space...\x1b[0m", img_path);
        if run_command_with_log(
            "Delete image file",
            "rm",
            &["-f", &img_path],
            true,
        ) {
            println!("\x1b[32m[✓] Successfully deleted image file and freed space.\x1b[0m");
        } else {
            println!("\x1b[31m[✗] Failed to delete image file.\x1b[0m");
        }
    } else {
        println!("\x1b[33m[!] Image file {} does not exist. Nothing to free.\x1b[0m", img_path);
    }

    // Step 3: Restart target service
    if restart_target_service() {
        println!("\n\x1b[32;1m[✓] Cleanup and deletion completed successfully!\x1b[0m");
    } else {
        println!("\n\x1b[33;1m[!] Cleanup completed, but failed to restart any target service. Please verify the target systemd service manually.\x1b[0m");
    }
}

// Check if running as root (UID == 0)
fn is_root() -> bool {
    if let Ok(output) = Command::new("id").arg("-u").output() {
        let uid_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        uid_str == "0"
    } else {
        false
    }
}

// Check if a CLI command exists on the path
fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

// Prompt for standard input
fn prompt(msg: &str) -> String {
    print!("{}", msg);
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

// Prompt and require non-empty input
fn prompt_non_empty(msg: &str) -> String {
    loop {
        let res = prompt(msg);
        if !res.is_empty() {
            return res;
        }
        println!("\x1b[31m[✗] Error: Field cannot be empty.\x1b[0m");
    }
}

// Prompt and validate password length (12-16 characters)
fn prompt_password_valid(msg: &str) -> String {
    loop {
        let pwd = prompt(msg);
        if pwd.len() >= 12 && pwd.len() <= 16 {
            return pwd;
        }
        println!(
            "\x1b[31m[✗] Error: Password must be between 12 and 16 characters long (current length: {}).\x1b[0m",
            pwd.len()
        );
    }
}

// Prompt for size (positive integer)
fn prompt_size(msg: &str, default: u64) -> u64 {
    loop {
        let res = prompt(msg);
        if res.is_empty() {
            return default;
        }
        match res.parse::<u64>() {
            Ok(val) if val > 0 => return val,
            _ => println!("\x1b[31m[✗] Error: Please enter a valid positive integer.\x1b[0m"),
        }
    }
}

// Helper to run a command and print nicely formatted log status
fn run_command_with_log(
    name: &str,
    program: &str,
    args: &[&str],
    run_as_root: bool,
) -> bool {
    let mut cmd = if run_as_root && !is_root() {
        let mut c = Command::new("sudo");
        c.arg(program);
        c
    } else {
        Command::new(program)
    };
    cmd.args(args);

    match cmd.status() {
        Ok(status) if status.success() => {
            println!("\x1b[32m[✓] Success: {}\x1b[0m", name);
            true
        }
        Ok(status) => {
            println!(
                "\x1b[31m[✗] Failed: {} (exit code: {})\x1b[0m",
                name,
                status.code().unwrap_or(-1)
            );
            false
        }
        Err(e) => {
            println!("\x1b[31m[✗] Failed to execute {}: {}\x1b[0m", name, e);
            false
        }
    }
}

// Run targetcli shell script using stdin redirection
fn run_targetcli_script(script: &str) -> bool {
    let mut cmd = if !is_root() {
        let mut c = Command::new("sudo");
        c.arg("targetcli");
        c
    } else {
        Command::new("targetcli")
    };

    let mut child = match cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            println!("\x1b[31m[✗] Failed to start targetcli: {}\x1b[0m", e);
            return false;
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(script.as_bytes()) {
            println!("\x1b[31m[✗] Failed to write commands to targetcli: {}\x1b[0m", e);
            return false;
        }
    }

    match child.wait_with_output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            
            // Print output for visibility
            if !stdout.is_empty() {
                println!("\n--- targetcli stdout ---");
                println!("{}", stdout);
            }
            if !stderr.is_empty() {
                println!("\n--- targetcli stderr ---");
                println!("{}", stderr);
            }

            if output.status.success() {
                println!("\x1b[32m[✓] Success: targetcli configuration applied.\x1b[0m");
                true
            } else {
                println!(
                    "\x1b[31m[✗] targetcli returned non-zero status: {}\x1b[0m",
                    output.status
                );
                false
            }
        }
        Err(e) => {
            println!("\x1b[31m[✗] Error waiting for targetcli: {}\x1b[0m", e);
            false
        }
    }
}
