#!/usr/bin/env bash
set -euo pipefail

echo "======================================"
echo "Pod 3 (SD Card) Log Extraction Script"
echo "======================================"
echo ""
echo "⚠️  For Pod 3 with SD card only!"
echo ""

# Default values
IMG_FILE=""
OUTPUT_DIR=""

# Parse command-line arguments
usage() {
    cat <<EOF
Usage: $0 -i IMAGE_FILE [-o OUTPUT_DIR]

⚠️  Pod 3 (SD Card Version) ONLY
   This script is designed for Eight Sleep Pod 3 with removable SD card.

Required arguments:
  -i IMAGE_FILE    Path to the SD card image file

Optional arguments:
  -o OUTPUT_DIR    Directory to save extracted logs (default: ./pod3_logs_<timestamp>)
  -h               Show this help message

Description:
  This script extracts system logs from a Pod 3 SD card image to help debug
  boot issues, WiFi connection problems, and service failures.

Extracted logs include:
  - journalctl logs (system, ssh-early, NetworkManager, opensleep)
  - NetworkManager configuration and connection files
  - systemd service status
  - dmesg output
  - Various system configuration files

Example:
  $0 -i pod3_patched.img
  $0 -i pod3_patched.img -o ./debug_logs
EOF
    exit 1
}

while getopts "i:o:h" opt; do
    case $opt in
        i) IMG_FILE="$OPTARG" ;;
        o) OUTPUT_DIR="$OPTARG" ;;
        h) usage ;;
        *) usage ;;
    esac
done

# Validate required arguments
if [[ -z "$IMG_FILE" ]]; then
    echo "Error: Missing required argument -i IMAGE_FILE"
    echo ""
    usage
fi

# Validate image file exists
if [[ ! -f "$IMG_FILE" ]]; then
    echo "Error: Image file not found: $IMG_FILE"
    exit 1
fi

echo "[*] Image file: $IMG_FILE"

# Set default output directory if not provided
if [[ -z "$OUTPUT_DIR" ]]; then
    TIMESTAMP=$(date +%Y%m%d_%H%M%S)
    OUTPUT_DIR="./pod3_logs_${TIMESTAMP}"
fi

# Create output directory
mkdir -p "$OUTPUT_DIR"
echo "[*] Output directory: $OUTPUT_DIR"
echo ""

# Create temporary working directory
WORK_DIR=$(mktemp -d -t extract_logs_$(basename "$IMG_FILE").XXXX)
MOUNT_DIR="$WORK_DIR/mount"

echo "[*] Working directory: $WORK_DIR"

# --- Cleanup function ---
cleanup() {
    local exit_code=$?
    echo ""
    echo "[*] Cleaning up..."
    
    # Unmount if mounted
    if mountpoint -q "$MOUNT_DIR" 2>/dev/null; then
        echo "[*] Unmounting $MOUNT_DIR..."
        sudo umount "$MOUNT_DIR" 2>/dev/null || true
    fi
    
    # Detach loop device if attached
    if [[ ! -z "${LOOP:-}" ]] && losetup "$LOOP" 2>/dev/null; then
        echo "[*] Detaching loop device $LOOP..."
        sudo losetup -d "$LOOP" 2>/dev/null || true
    fi
    
    # Remove working directory
    if [[ -d "$WORK_DIR" ]]; then
        echo "[*] Removing work directory..."
        rm -rf "$WORK_DIR" 2>/dev/null || true
    fi
    
    if [[ $exit_code -ne 0 ]]; then
        echo "[-] Script failed with exit code $exit_code"
    else
        echo "[+] Cleanup complete"
    fi
    
    exit $exit_code
}

# Set trap to call cleanup on exit, interrupt, or termination
trap cleanup EXIT INT TERM

# --- Setup loop device (read-only) ---
echo "[*] Setting up loop device (read-only)..."
LOOP=$(sudo losetup -Prf --show "$IMG_FILE")
echo "[*] Loop device: $LOOP"

# --- Mount root partition ---
ROOT_PART="${LOOP}p1"
mkdir -p "$MOUNT_DIR"
sudo mount "$ROOT_PART" "$MOUNT_DIR"
echo "[*] Mounted root partition at $ROOT_PART"

echo ""
echo "======================================"
echo "Extracting Logs and Configuration"
echo "======================================"
echo ""
echo "[*] Note: Extracting logs from the mounted root partition"
echo "[*] These are the actual runtime logs from the Pod"
echo ""

# --- Extract journal logs ---
echo "[*] Extracting journal logs..."
JOURNAL_DIR="$MOUNT_DIR/var/log/journal"
if sudo test -d "$JOURNAL_DIR"; then
    sudo cp -r "$JOURNAL_DIR" "$OUTPUT_DIR/journal" 2>/dev/null || true
    echo "  ✓ Journal logs extracted"
    
    # Attempt to decode journal logs with journalctl
    echo "[*] Decoding journal logs..."
    if command -v journalctl &> /dev/null; then
        # Full boot log
        sudo journalctl --directory="$OUTPUT_DIR/journal" --no-pager > "$OUTPUT_DIR/journal/decoded_full.log" 2>/dev/null || true
        
        # Service-specific logs
        sudo journalctl --directory="$OUTPUT_DIR/journal" --no-pager -u variscite-wifi.service > "$OUTPUT_DIR/journal/variscite-wifi.log" 2>/dev/null || true
        sudo journalctl --directory="$OUTPUT_DIR/journal" --no-pager -u wpa_supplicant@wlan0.service > "$OUTPUT_DIR/journal/wpa_supplicant.log" 2>/dev/null || true
        sudo journalctl --directory="$OUTPUT_DIR/journal" --no-pager -u ssh-early.service > "$OUTPUT_DIR/journal/ssh-early.log" 2>/dev/null || true
        sudo journalctl --directory="$OUTPUT_DIR/journal" --no-pager -u systemd-networkd.service > "$OUTPUT_DIR/journal/systemd-networkd.log" 2>/dev/null || true
        sudo journalctl --directory="$OUTPUT_DIR/journal" --no-pager -u opensleep.service > "$OUTPUT_DIR/journal/opensleep.log" 2>/dev/null || true
        
        # Boot messages
        sudo journalctl --directory="$OUTPUT_DIR/journal" --no-pager -b > "$OUTPUT_DIR/journal/decoded_boot.log" 2>/dev/null || true
        
        # Kernel messages
        sudo journalctl --directory="$OUTPUT_DIR/journal" --no-pager -k > "$OUTPUT_DIR/journal/decoded_kernel.log" 2>/dev/null || true
        
        # Priority: errors and warnings
        sudo journalctl --directory="$OUTPUT_DIR/journal" --no-pager -p err > "$OUTPUT_DIR/journal/decoded_errors.log" 2>/dev/null || true
        sudo journalctl --directory="$OUTPUT_DIR/journal" --no-pager -p warning > "$OUTPUT_DIR/journal/decoded_warnings.log" 2>/dev/null || true
        
        echo "  ✓ Journal logs decoded"
    else
        echo "  ⚠ journalctl not found - install systemd to decode journal logs"
    fi
else
    echo "  ℹ No persistent journal logs found"
fi

# --- Extract system logs ---
echo "[*] Extracting system logs..."
if sudo test -f "$MOUNT_DIR/var/log/syslog"; then
    sudo cp "$MOUNT_DIR/var/log/syslog" "$OUTPUT_DIR/syslog" 2>/dev/null || true
    echo "  ✓ syslog extracted"
fi

if sudo test -f "$MOUNT_DIR/var/log/messages"; then
    sudo cp "$MOUNT_DIR/var/log/messages" "$OUTPUT_DIR/messages" 2>/dev/null || true
    echo "  ✓ messages extracted"
fi

if sudo test -f "$MOUNT_DIR/var/log/kern.log"; then
    sudo cp "$MOUNT_DIR/var/log/kern.log" "$OUTPUT_DIR/kern.log" 2>/dev/null || true
    echo "  ✓ kern.log extracted"
fi

if sudo test -f "$MOUNT_DIR/var/log/daemon.log"; then
    sudo cp "$MOUNT_DIR/var/log/daemon.log" "$OUTPUT_DIR/daemon.log" 2>/dev/null || true
    echo "  ✓ daemon.log extracted"
fi

if sudo test -f "$MOUNT_DIR/var/log/boot.log"; then
    sudo cp "$MOUNT_DIR/var/log/boot.log" "$OUTPUT_DIR/boot.log" 2>/dev/null || true
    echo "  ✓ boot.log extracted"
fi

# --- Extract all log files ---
echo "[*] Extracting additional log files..."
if sudo test -d "$MOUNT_DIR/var/log"; then
    mkdir -p "$OUTPUT_DIR/var_log"
    sudo find "$MOUNT_DIR/var/log" -type f -name "*.log" -exec cp {} "$OUTPUT_DIR/var_log/" \; 2>/dev/null || true
    LOG_COUNT=$(ls "$OUTPUT_DIR/var_log" 2>/dev/null | wc -l)
    if [[ $LOG_COUNT -gt 0 ]]; then
        echo "  ✓ Extracted $LOG_COUNT additional log files"
    fi
fi

# --- Extract NetworkManager logs and config ---
echo "[*] Extracting NetworkManager configuration..."
NM_DIR="$MOUNT_DIR/etc/NetworkManager"
if sudo test -d "$NM_DIR"; then
    mkdir -p "$OUTPUT_DIR/NetworkManager"
    sudo cp -r "$NM_DIR/system-connections" "$OUTPUT_DIR/NetworkManager/" 2>/dev/null || true
    sudo cp "$NM_DIR/NetworkManager.conf" "$OUTPUT_DIR/NetworkManager/" 2>/dev/null || true
    
    # Also get NetworkManager state file
    if sudo test -f "$MOUNT_DIR/var/lib/NetworkManager/NetworkManager.state"; then
        sudo cp "$MOUNT_DIR/var/lib/NetworkManager/NetworkManager.state" "$OUTPUT_DIR/NetworkManager/" 2>/dev/null || true
    fi
    
    echo "  ✓ NetworkManager config extracted"
else
    echo "  ℹ NetworkManager not found"
fi

# --- Extract ConnMan configuration ---
echo "[*] Extracting ConnMan configuration..."
if sudo test -d "$MOUNT_DIR/var/lib/connman"; then
    mkdir -p "$OUTPUT_DIR/connman"
    sudo cp -r "$MOUNT_DIR/var/lib/connman" "$OUTPUT_DIR/connman/var_lib" 2>/dev/null || true
    echo "  ✓ ConnMan config extracted"
else
    echo "  ℹ ConnMan not found"
fi

if sudo test -d "$MOUNT_DIR/etc/connman"; then
    mkdir -p "$OUTPUT_DIR/connman"
    sudo cp -r "$MOUNT_DIR/etc/connman" "$OUTPUT_DIR/connman/etc" 2>/dev/null || true
fi

# --- Extract systemd-networkd configuration ---
echo "[*] Extracting systemd-networkd configuration..."
if sudo test -d "$MOUNT_DIR/etc/systemd/network"; then
    mkdir -p "$OUTPUT_DIR/systemd-networkd"
    sudo cp -r "$MOUNT_DIR/etc/systemd/network" "$OUTPUT_DIR/systemd-networkd/" 2>/dev/null || true
    echo "  ✓ systemd-networkd config extracted"
else
    echo "  ℹ systemd-networkd config not found"
fi

# --- Extract systemd service files ---
echo "[*] Extracting systemd service files..."
mkdir -p "$OUTPUT_DIR/systemd/services"
mkdir -p "$OUTPUT_DIR/systemd/enabled"

# Copy all custom services from /etc/systemd/system/
if sudo test -d "$MOUNT_DIR/etc/systemd/system"; then
    sudo find "$MOUNT_DIR/etc/systemd/system" -maxdepth 1 -name "*.service" -exec cp {} "$OUTPUT_DIR/systemd/services/" \; 2>/dev/null || true
fi

# Copy all system services from /lib/systemd/system/
if sudo test -d "$MOUNT_DIR/lib/systemd/system"; then
    mkdir -p "$OUTPUT_DIR/systemd/system_services"
    sudo find "$MOUNT_DIR/lib/systemd/system" -maxdepth 1 -name "*network*" -o -name "*wifi*" -o -name "*wpa*" -o -name "*connman*" 2>/dev/null | while read service; do
        sudo cp "$service" "$OUTPUT_DIR/systemd/system_services/" 2>/dev/null || true
    done
fi

# List all enabled services
if sudo test -d "$MOUNT_DIR/etc/systemd/system/multi-user.target.wants"; then
    sudo ls -la "$MOUNT_DIR/etc/systemd/system/multi-user.target.wants" > "$OUTPUT_DIR/systemd/enabled/multi-user.target.wants.txt" 2>/dev/null || true
fi

if sudo test -d "$MOUNT_DIR/etc/systemd/system/network-online.target.wants"; then
    sudo ls -la "$MOUNT_DIR/etc/systemd/system/network-online.target.wants" > "$OUTPUT_DIR/systemd/enabled/network-online.target.wants.txt" 2>/dev/null || true
fi

# List all systemd services
sudo find "$MOUNT_DIR/etc/systemd/system" -type l -o -type f 2>/dev/null | sudo xargs ls -la 2>/dev/null > "$OUTPUT_DIR/systemd/all_systemd_links.txt" || true

echo "  ✓ Systemd services extracted"

# --- Extract SSH configuration ---
echo "[*] Extracting SSH configuration..."
mkdir -p "$OUTPUT_DIR/ssh"
if sudo test -f "$MOUNT_DIR/etc/ssh/sshd_config"; then
    sudo cp "$MOUNT_DIR/etc/ssh/sshd_config" "$OUTPUT_DIR/ssh/" 2>/dev/null || true
fi
if sudo test -f "$MOUNT_DIR/etc/ssh/authorized_keys"; then
    sudo cp "$MOUNT_DIR/etc/ssh/authorized_keys" "$OUTPUT_DIR/ssh/" 2>/dev/null || true
fi
if sudo test -f "$MOUNT_DIR/home/rewt/.ssh/authorized_keys"; then
    sudo cp "$MOUNT_DIR/home/rewt/.ssh/authorized_keys" "$OUTPUT_DIR/ssh/rewt_authorized_keys" 2>/dev/null || true
fi
echo "  ✓ SSH config extracted"

# --- Extract network configuration ---
echo "[*] Extracting network configuration..."
mkdir -p "$OUTPUT_DIR/network"

# Basic network config
if sudo test -f "$MOUNT_DIR/etc/resolv.conf"; then
    sudo cp "$MOUNT_DIR/etc/resolv.conf" "$OUTPUT_DIR/network/" 2>/dev/null || true
fi
if sudo test -f "$MOUNT_DIR/etc/hosts"; then
    sudo cp "$MOUNT_DIR/etc/hosts" "$OUTPUT_DIR/network/" 2>/dev/null || true
fi
if sudo test -f "$MOUNT_DIR/etc/hostname"; then
    sudo cp "$MOUNT_DIR/etc/hostname" "$OUTPUT_DIR/network/" 2>/dev/null || true
fi

# Network interfaces
if sudo test -f "$MOUNT_DIR/etc/network/interfaces"; then
    sudo cp "$MOUNT_DIR/etc/network/interfaces" "$OUTPUT_DIR/network/" 2>/dev/null || true
fi
if sudo test -d "$MOUNT_DIR/etc/network/interfaces.d"; then
    sudo cp -r "$MOUNT_DIR/etc/network/interfaces.d" "$OUTPUT_DIR/network/" 2>/dev/null || true
fi

# wpa_supplicant
if sudo test -d "$MOUNT_DIR/etc/wpa_supplicant"; then
    sudo cp -r "$MOUNT_DIR/etc/wpa_supplicant" "$OUTPUT_DIR/network/" 2>/dev/null || true
    # List all wpa_supplicant config files for debugging
    sudo ls -la "$MOUNT_DIR/etc/wpa_supplicant/" > "$OUTPUT_DIR/network/wpa_supplicant_files.txt" 2>/dev/null || true
fi

# iwd (alternative to wpa_supplicant)
if sudo test -d "$MOUNT_DIR/var/lib/iwd"; then
    sudo cp -r "$MOUNT_DIR/var/lib/iwd" "$OUTPUT_DIR/network/" 2>/dev/null || true
fi

# Check for active network interfaces
echo "Network Interface Information:" > "$OUTPUT_DIR/network/interface_info.txt" 2>/dev/null || true
if sudo test -f "$MOUNT_DIR/sys/class/net/wlan0/address"; then
    echo "wlan0 MAC: $(sudo cat $MOUNT_DIR/sys/class/net/wlan0/address 2>/dev/null || echo 'N/A')" >> "$OUTPUT_DIR/network/interface_info.txt"
fi

echo "  ✓ Network config extracted"

# --- Extract opensleep logs if present ---
echo "[*] Extracting opensleep logs and configuration..."
if sudo test -d "$MOUNT_DIR/opt/opensleep"; then
    mkdir -p "$OUTPUT_DIR/opensleep"
    sudo cp "$MOUNT_DIR/opt/opensleep/config.ron" "$OUTPUT_DIR/opensleep/" 2>/dev/null || true
    # Look for opensleep log files
    if sudo test -f "$MOUNT_DIR/var/log/opensleep.log"; then
        sudo cp "$MOUNT_DIR/var/log/opensleep.log" "$OUTPUT_DIR/opensleep/" 2>/dev/null || true
    fi
    echo "  ✓ opensleep config and logs extracted"
else
    echo "  ℹ opensleep not installed"
fi

# --- Extract Eight Sleep services and scripts ---
echo "[*] Extracting Eight Sleep configuration..."
mkdir -p "$OUTPUT_DIR/eightsleep"

# Look for Eight Sleep services
if sudo test -d "$MOUNT_DIR/lib/systemd/system"; then
    sudo find "$MOUNT_DIR/lib/systemd/system" -name "*capybara*" -o -name "*eight*" -o -name "*variscite*" 2>/dev/null | while read service; do
        sudo cp "$service" "$OUTPUT_DIR/eightsleep/" 2>/dev/null || true
    done
fi

# Look for Eight Sleep scripts and binaries
if sudo test -d "$MOUNT_DIR/opt"; then
    sudo ls -la "$MOUNT_DIR/opt" > "$OUTPUT_DIR/eightsleep/opt_contents.txt" 2>/dev/null || true
fi

if sudo test -d "$MOUNT_DIR/usr/local/bin"; then
    sudo ls -la "$MOUNT_DIR/usr/local/bin" > "$OUTPUT_DIR/eightsleep/usr_local_bin_contents.txt" 2>/dev/null || true
fi

# Look for any WiFi-related scripts
if sudo test -d "$MOUNT_DIR/etc/init.d"; then
    sudo ls -la "$MOUNT_DIR/etc/init.d" > "$OUTPUT_DIR/eightsleep/init_d_contents.txt" 2>/dev/null || true
    sudo find "$MOUNT_DIR/etc/init.d" -name "*network*" -o -name "*wifi*" 2>/dev/null | while read script; do
        sudo cp "$script" "$OUTPUT_DIR/eightsleep/" 2>/dev/null || true
    done
fi

# Extract variscite-wifi script (important for WiFi setup)
if sudo test -d "$MOUNT_DIR/etc/wifi"; then
    sudo ls -la "$MOUNT_DIR/etc/wifi" > "$OUTPUT_DIR/eightsleep/etc_wifi_contents.txt" 2>/dev/null || true
    if sudo test -f "$MOUNT_DIR/etc/wifi/variscite-wifi"; then
        sudo cp "$MOUNT_DIR/etc/wifi/variscite-wifi" "$OUTPUT_DIR/eightsleep/" 2>/dev/null || true
        echo "  ✓ variscite-wifi script extracted"
    fi
fi

echo "  ✓ Eight Sleep config extracted"

# --- Extract installed packages list ---
echo "[*] Extracting package information..."
mkdir -p "$OUTPUT_DIR/packages"

# dpkg (Debian/Ubuntu)
if sudo test -f "$MOUNT_DIR/var/lib/dpkg/status"; then
    sudo grep "^Package:\|^Status:" "$MOUNT_DIR/var/lib/dpkg/status" | grep -B1 "install ok installed" | grep "^Package:" | awk '{print $2}' | sort > "$OUTPUT_DIR/packages/dpkg_installed.txt" 2>/dev/null || true
fi

# opkg (embedded systems)
if sudo test -f "$MOUNT_DIR/usr/lib/opkg/status"; then
    sudo grep "^Package:" "$MOUNT_DIR/usr/lib/opkg/status" | awk '{print $2}' | sort > "$OUTPUT_DIR/packages/opkg_installed.txt" 2>/dev/null || true
fi

# Check for specific network managers
echo "Checking for network management tools:" > "$OUTPUT_DIR/packages/network_tools.txt"
for tool in NetworkManager connman systemd-networkd wpa_supplicant iwd nmcli connmanctl; do
    if sudo test -f "$MOUNT_DIR/usr/bin/$tool" || sudo test -f "$MOUNT_DIR/usr/sbin/$tool" || sudo test -f "$MOUNT_DIR/bin/$tool" || sudo test -f "$MOUNT_DIR/sbin/$tool"; then
        echo "  ✓ $tool found" >> "$OUTPUT_DIR/packages/network_tools.txt"
    else
        echo "  ✗ $tool not found" >> "$OUTPUT_DIR/packages/network_tools.txt"
    fi
done

echo "  ✓ Package info extracted"

# --- Create system information file ---
echo "[*] Creating system information file..."
cat > "$OUTPUT_DIR/system_info.txt" <<EOF
Pod 3 System Log Extraction
===========================
Extraction Date: $(date)
Image File: $IMG_FILE
Extracted By: $(whoami)@$(hostname)

Image Information:
- Size: $(ls -lh "$IMG_FILE" | awk '{print $5}')

Root Partition Information:
- Mount Point: $MOUNT_DIR
- Loop Device: $LOOP

Files Extracted:
- Journal logs: $(test -d "$OUTPUT_DIR/journal" && echo "Yes" || echo "No")
- System logs: $(test -f "$OUTPUT_DIR/syslog" && echo "Yes" || echo "No")
- Boot logs: $(test -f "$OUTPUT_DIR/boot.log" && echo "Yes" || echo "No")
- NetworkManager config: $(test -d "$OUTPUT_DIR/NetworkManager" && echo "Yes" || echo "No")
- ConnMan config: $(test -d "$OUTPUT_DIR/connman" && echo "Yes" || echo "No")
- systemd-networkd config: $(test -d "$OUTPUT_DIR/systemd-networkd" && echo "Yes" || echo "No")
- Systemd services: $(test -d "$OUTPUT_DIR/systemd" && echo "Yes" || echo "No")
- SSH config: $(test -d "$OUTPUT_DIR/ssh" && echo "Yes" || echo "No")
- Network config: $(test -d "$OUTPUT_DIR/network" && echo "Yes" || echo "No")
- Eight Sleep config: $(test -d "$OUTPUT_DIR/eightsleep" && echo "Yes" || echo "No")
- Package info: $(test -d "$OUTPUT_DIR/packages" && echo "Yes" || echo "No")
- opensleep config: $(test -d "$OUTPUT_DIR/opensleep" && echo "Yes" || echo "No")

Network Management Tools Found:
$(cat "$OUTPUT_DIR/packages/network_tools.txt" 2>/dev/null || echo "N/A")

WiFi Configuration Files Found:
$(if sudo test -f "$MOUNT_DIR/etc/wpa_supplicant/wpa_supplicant-wlan0.conf"; then echo "  ✓ wpa_supplicant-wlan0.conf"; fi)
$(if sudo test -f "$MOUNT_DIR/etc/systemd/network/25-wlan0.network"; then echo "  ✓ 25-wlan0.network"; fi)
$(if sudo test -f "$MOUNT_DIR/etc/wifi/variscite-wifi"; then echo "  ✓ variscite-wifi script"; fi)

Custom Services Found:
$(if sudo test -f "$MOUNT_DIR/etc/systemd/system/ssh-early.service"; then echo "  ✓ ssh-early.service"; fi)
$(if sudo test -f "$MOUNT_DIR/etc/systemd/system/disable-eightsleep-services.service"; then echo "  ✓ disable-eightsleep-services.service"; fi)
$(if sudo test -L "$MOUNT_DIR/etc/systemd/system/multi-user.target.wants/wpa_supplicant@wlan0.service"; then echo "  ✓ wpa_supplicant@wlan0.service (enabled)"; fi)

EOF

# --- List NetworkManager connections ---
echo "[*] Listing NetworkManager connections..."
if sudo test -d "$MOUNT_DIR/etc/NetworkManager/system-connections"; then
    echo "" >> "$OUTPUT_DIR/system_info.txt"
    echo "NetworkManager Connections:" >> "$OUTPUT_DIR/system_info.txt"
    sudo ls -la "$MOUNT_DIR/etc/NetworkManager/system-connections" >> "$OUTPUT_DIR/system_info.txt" 2>/dev/null || true
fi

# --- Fix permissions on extracted files ---
echo "[*] Fixing file permissions..."
sudo chown -R $(id -u):$(id -g) "$OUTPUT_DIR"
sudo chmod -R u+rw "$OUTPUT_DIR"

echo ""
echo "======================================"
echo "Extraction Complete!"
echo "======================================"
echo ""
echo "[+] Logs extracted to: $OUTPUT_DIR"
echo ""
echo "To view the extracted data:"
echo "  - System info: cat $OUTPUT_DIR/system_info.txt"
echo "  - Network tools: cat $OUTPUT_DIR/packages/network_tools.txt"
echo "  - Installed packages: cat $OUTPUT_DIR/packages/*_installed.txt"
echo "  - Network config: ls -la $OUTPUT_DIR/network/"
echo "  - Systemd services: cat $OUTPUT_DIR/systemd/services/*.service"
echo "  - Enabled services: cat $OUTPUT_DIR/systemd/enabled/*.txt"
echo "  - Eight Sleep config: ls -la $OUTPUT_DIR/eightsleep/"
echo "  - SSH config: cat $OUTPUT_DIR/ssh/sshd_config"
echo ""
echo "For journal logs (if present):"
echo "  journalctl --directory=$OUTPUT_DIR/journal --no-pager"
echo ""
