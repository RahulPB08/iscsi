use std::io::{self, Write};
use std::process::Command;

/// Safely captures text input from the user via the terminal
fn get_input(prompt: &str) -> String {
    print!("{}", prompt);
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).expect("Failed to read line");
    input.trim().to_string()
}

/// Helper function to execute shell commands and capture standard error if they fail
fn run_command(cmd: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to initiate '{}': {}", cmd, e))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

/// Phase 1: Logs into the iSCSI target to bring the remote storage blocks online
fn setup_iscsi() -> Result<(), String> {
    println!("\n=== [ Step 1: Connect to iSCSI Storage Shared Array ] ===");
    let target_ip = get_input("Enter iSCSI Target Storage IP: ");

    println!("[+] Discovering available targets on {}...", target_ip);
    run_command("iscsiadm", &["-m", "discovery", "-t", "sendtargets", "-p", &target_ip])?;

    println!("[+] Logging into discovered targets to expose raw block drives...");
    run_command("iscsiadm", &["-m", "node", "--login"])?;

    println!("[✓] iSCSI Target session established successfully.");
    Ok(())
}

/// Option 1: Configure this machine as the dedicated Management Server (MGS)
fn configure_mgs() -> Result<(), String> {
    println!("\n=== [ Setup Role: Management Server (MGS) ] ===");
    let fs_name = get_input("Enter Lustre Filesystem Name (e.g., lustre): ");
    let device = get_input("Enter iSCSI block device path for MGS (e.g., /dev/sdb): ");
    let mount_point = get_input("Enter local mount path (e.g., /mnt/mgs): ");

    println!("[+] Formatting device {} as Lustre MGS...", device);
    run_command("mkfs.lustre", &["--fsname", &fs_name, "--mgs", &device])?;

    println!("[+] Creating mount directory and mounting MGS...");
    let _ = Command::new("mkdir").args(&["-p", &mount_point]).status();
    run_command("mount", &["-t", "lustre", &device, &mount_point])?;

    println!("[✓] MGS is online and active at {}!", mount_point);
    Ok(())
}

/// Option 2: Configure this machine as the Metadata Server (MDS)
fn configure_mds() -> Result<(), String> {
    println!("\n=== [ Setup Role: Metadata Server (MDS/MDT) ] ===");
    let fs_name = get_input("Enter Lustre Filesystem Name: ");
    let mgs_ip = get_input("Enter the remote MGS Node IP address: ");
    let device = get_input("Enter iSCSI block device path for MDT (e.g., /dev/sdc): ");
    let mount_point = get_input("Enter local mount path (e.g., /mnt/mdt): ");

    println!("[+] Formatting device {} as Lustre MDT...", device);
    run_command("mkfs.lustre", &[
        "--fsname", &fs_name,
        "--mdt",
        &format!("--mgsnode={}@tcp0", mgs_ip),
        "--index=0",
        &device
    ])?;

    println!("[+] Creating mount directory and mounting MDS...");
    let _ = Command::new("mkdir").args(&["-p", &mount_point]).status();
    run_command("mount", &["-t", "lustre", &device, &mount_point])?;

    println!("[✓] MDS target index 0 registered and online at {}!", mount_point);
    Ok(())
}

/// Option 3: Configure this machine as an Object Storage Server (OSS)
fn configure_oss() -> Result<(), String> {
    println!("\n=== [ Setup Role: Object Storage Server (OSS/OST) ] ===");
    let fs_name = get_input("Enter Lustre Filesystem Name: ");
    let mgs_ip = get_input("Enter the remote MGS Node IP address: ");
    let device = get_input("Enter iSCSI block device path for OST (e.g., /dev/sdd): ");
    let ost_index = get_input("Enter OST Index number (e.g., 0, 1, 2): ");
    let mount_point = get_input("Enter local mount path (e.g., /mnt/ost0): ");

    println!("[+] Formatting device {} as Lustre OST index {}...", device, ost_index);
    run_command("mkfs.lustre", &[
        "--fsname", &fs_name,
        "--ost",
        &format!("--mgsnode={}@tcp0", mgs_ip),
        &format!("--index={}", ost_index),
        &device
    ])?;

    println!("[+] Creating mount directory and mounting OSS Target...");
    let _ = Command::new("mkdir").args(&["-p", &mount_point]).status();
    run_command("mount", &["-t", "lustre", &device, &mount_point])?;

    println!("[✓] OSS Target index {} registered and online at {}!", ost_index, mount_point);
    Ok(())
}

/// Option 4: Configure this machine as a Lustre Client to access the storage pool
fn configure_client() -> Result<(), String> {
    println!("\n=== [ Setup Role: Lustre Client Mount ] ===");
    let mgs_ip = get_input("Enter the remote MGS Node IP address: ");
    let fs_name = get_input("Enter Lustre Filesystem Name: ");
    let mount_point = get_input("Enter Client target mount folder (e.g., /mnt/lustre): ");

    println!("[+] Creating mount directory and mounting client cluster access point...");
    let _ = Command::new("mkdir").args(&["-p", &mount_point]).status();
    run_command("mount", &[
        "-t", "lustre",
        &format!("{}@tcp0:/{}", mgs_ip, fs_name),
        &mount_point
    ])?;

    println!("[✓] Success! Lustre cluster is fully accessible via {}!", mount_point);
    Ok(())
}

fn main() {
    println!("=======================================================");
    println!("     Interactive iSCSI + Lustre Cluster Installer      ");
    println!("=======================================================");

    // All architectures must connect to the iSCSI storage plane first
    if let Err(e) = setup_iscsi() {
        eprintln!("[!] Initial iSCSI plane failure: {}\nEnsure target service is running.", e);
        return;
    }

    // Prompt user for role assignment
    println!("\nSelect the role this node will execute in the cluster:");
    println!("1) Dedicated Management Server (MGS)");
    println!("2) Dedicated Metadata Server (MDS / MDT)");
    println!("3) Object Storage Server (OSS / OST)");
    println!("4) Mount Node Cluster Access Point (Lustre Client)");
    
    let choice = get_input("\nEnter choice (1-4): ");

    let result = match choice.as_str() {
        "1" => configure_mgs(),
        "2" => configure_mds(),
        "3" => configure_oss(),
        "4" => configure_client(),
        _ => Err("Invalid selection criteria received. Exiting...".to_string()),
    };

    match result {
        Ok(_) => println!("\n[✓] Role configuration routine executed completely."),
        Err(e) => eprintln!("\n[!] Configuration aborted due to error: {}", e),
    }
}