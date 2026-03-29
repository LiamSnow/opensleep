#!/usr/bin/env bash
set -euo pipefail

echo "======================================"
echo "Pod 3 (SD Card) Image Patching Script"
echo "======================================"
echo ""
echo "⚠️  IMPORTANT: This script is for Pod 3 with SD card only!"
echo "   - Does NOT work with Pod 1, 2, 4, or 5"
echo "   - Does NOT work with Pod 3 without SD card (eMMC version)"
echo ""
echo "Note: This script requires sudo privileges for:"
echo "  - Mounting disk images (losetup, mount)"
echo "  - Extracting and modifying system files"
echo "  - Setting proper file ownership and permissions"
echo ""

# Default values
IMG_FILE=""
PUBKEY_FILE=""
SSID=""
PSK=""
PASSWORD=""
DISABLE_SERVICES=false
REMOVE_EIGHTSLEEP_KEYS=false
OPENSLEEP_BINARY=""
OPENSLEEP_SERVICE=""
OPENSLEEP_CONFIG=""
OUTPUT_FILE=""
WORK_DIR_CUSTOM=""
VALIDATION_SCRIPT=""
MAC_ADDRESS=""

# Parse command-line arguments
usage() {
    cat <<EOF
Usage: $0 -i IMAGE_FILE -k PUBKEY_FILE [-o OUTPUT_FILE] [-w WORK_DIR] [-s SSID] [-p PSK] [-P PASSWORD] [-m MAC_ADDRESS] [-R] [-d] [-b BINARY] [-S SERVICE] [-c CONFIG] [-v VALIDATION_SCRIPT]

⚠️  Pod 3 (SD Card Version) ONLY
   This script is designed for Eight Sleep Pod 3 with removable SD card.
   It does NOT work with Pod 1, 2, 4, 5, or Pod 3 eMMC (non-SD) versions.

Required arguments:
  -i IMAGE_FILE    Path to the SD card image file
  -k PUBKEY_FILE   Path to the SSH public key file

Optional arguments:
  -o OUTPUT_FILE   Path for the patched output image (default: <original>-patched.img)
  -w WORK_DIR      Custom working directory (must be empty, needs 16GB free space)
  -s SSID          WiFi network SSID
  -p PSK           WiFi network password/passphrase
  -P PASSWORD      Password for the rewt user
  -m MAC_ADDRESS   Set persistent MAC address for wlan0 (format: AA:BB:CC:DD:EE:FF)
                   Prevents MAC from changing on factory reset
  -R               [DISABLED] Remove Eight Sleep SSH keys (currently causes SSH connection failure)
                   This feature is being debugged and will be re-enabled in a future update.
  -d               Disable Eight Sleep services on first boot
                   For saftey, it waits for a wifi connection before disabling, this ensures the pairing and reset options are not interfered with.
                   WARNING: This prevents normal Eight Sleep app pairing.
                   To restore Eight Sleep functionality, you must reflash the original image.
  -b BINARY        Path to opensleep binary to install
  -S SERVICE       Path to opensleep.service file to install
  -c CONFIG        Path to config.ron file to install
  -v VALIDATION    Path to validation script to include in image (default: auto-detect in same directory)
  -h               Show this help message

Example:
  $0 -i sdcard.img -k ~/.ssh/id_rsa.pub -s "MyWiFi" -p "password123" -P "userpass" -d
  $0 -i sdcard.img -o sdcard-patched.img -w /mnt/bigdrive/temp -k ~/.ssh/id_rsa.pub -s "MyWiFi" -p "pass" -b ./opensleep -S ./opensleep.service -c ./config.ron -d
  $0 -i sdcard.img -k ~/.ssh/id_rsa.pub -s "MyWiFi" -p "pass" -v ./custom_validation.sh
  $0 -i sdcard.img -k ~/.ssh/id_rsa.pub -s "MyWiFi" -p "pass" -m "02:11:22:33:44:55"
  $0 -i sdcard.img -k ~/.ssh/id_rsa.pub -s "MyWiFi" -p "pass" -R -d
EOF
    exit 1
}

while getopts "i:k:o:w:s:p:P:b:S:c:v:m:dh" opt; do
    case $opt in
        i) IMG_FILE="$OPTARG" ;;
        k) PUBKEY_FILE="$OPTARG" ;;
        o) OUTPUT_FILE="$OPTARG" ;;
        w) WORK_DIR_CUSTOM="$OPTARG" ;;
        s) SSID="$OPTARG" ;;
        p) PSK="$OPTARG" ;;
        P) PASSWORD="$OPTARG" ;;
        b) OPENSLEEP_BINARY="$OPTARG" ;;
        S) OPENSLEEP_SERVICE="$OPTARG" ;;
        c) OPENSLEEP_CONFIG="$OPTARG" ;;
        v) VALIDATION_SCRIPT="$OPTARG" ;;
        m) MAC_ADDRESS="$OPTARG" ;;
        R) echo "ERROR: -R flag is temporarily disabled due to SSH connection issues"
           echo "This feature is being debugged and will be re-enabled in a future update"
           exit 1 ;;
        d) DISABLE_SERVICES=true ;;
        h) usage ;;
        *) usage ;;
    esac
done

# Validate required arguments
if [[ -z "$IMG_FILE" ]] || [[ -z "$PUBKEY_FILE" ]]; then
    echo ""
    echo "Error: Missing required arguments"
    echo ""
    usage
fi

echo "[*] Validating input files..."

# Validate files exist
if [[ ! -f "$IMG_FILE" ]]; then
    echo "Error: Image file not found: $IMG_FILE"
    exit 1
fi
echo "  ✓ Image file found: $IMG_FILE"

if [[ ! -f "$PUBKEY_FILE" ]]; then
    echo "Error: Public key file not found: $PUBKEY_FILE"
    exit 1
fi
echo "  ✓ Public key file found: $PUBKEY_FILE"

# Validate opensleep files if provided
if [[ ! -z "$OPENSLEEP_BINARY" ]] && [[ ! -f "$OPENSLEEP_BINARY" ]]; then
    echo "Error: opensleep binary not found: $OPENSLEEP_BINARY"
    exit 1
fi

if [[ ! -z "$OPENSLEEP_SERVICE" ]] && [[ ! -f "$OPENSLEEP_SERVICE" ]]; then
    echo "Error: opensleep.service file not found: $OPENSLEEP_SERVICE"
    exit 1
fi

if [[ ! -z "$OPENSLEEP_CONFIG" ]] && [[ ! -f "$OPENSLEEP_CONFIG" ]]; then
    echo "Error: config.ron file not found: $OPENSLEEP_CONFIG"
    exit 1
fi

if [[ ! -z "$OPENSLEEP_BINARY" ]]; then
    echo "  ✓ opensleep binary found: $OPENSLEEP_BINARY"
    echo "  ✓ opensleep.service found: $OPENSLEEP_SERVICE"
    echo "  ✓ config.ron found: $OPENSLEEP_CONFIG"
fi

# Auto-detect validation script if not specified
if [[ -z "$VALIDATION_SCRIPT" ]]; then
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    AUTO_VALIDATION="$SCRIPT_DIR/full_patch_workflow_post_ssh_validation.sh"
    if [[ -f "$AUTO_VALIDATION" ]]; then
        VALIDATION_SCRIPT="$AUTO_VALIDATION"
        echo "  ✓ Auto-detected validation script: $VALIDATION_SCRIPT"
    fi
elif [[ ! -f "$VALIDATION_SCRIPT" ]]; then
    echo "Error: Validation script not found: $VALIDATION_SCRIPT"
    exit 1
else
    echo "  ✓ Validation script found: $VALIDATION_SCRIPT"
fi

# Validate MAC address format if provided
if [[ ! -z "$MAC_ADDRESS" ]]; then
    if [[ ! "$MAC_ADDRESS" =~ ^([0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}$ ]]; then
        echo "Error: Invalid MAC address format: $MAC_ADDRESS"
        echo "Expected format: AA:BB:CC:DD:EE:FF (e.g., 02:11:22:33:44:55)"
        exit 1
    fi
    echo "  ✓ MAC address validated: $MAC_ADDRESS"
fi

echo ""

# Check if all three opensleep files are provided together
OPENSLEEP_COUNT=0
[[ ! -z "$OPENSLEEP_BINARY" ]] && ((OPENSLEEP_COUNT++)) || true
[[ ! -z "$OPENSLEEP_SERVICE" ]] && ((OPENSLEEP_COUNT++)) || true
[[ ! -z "$OPENSLEEP_CONFIG" ]] && ((OPENSLEEP_COUNT++)) || true

if [[ $OPENSLEEP_COUNT -gt 0 ]] && [[ $OPENSLEEP_COUNT -lt 3 ]]; then
    echo "Error: All three opensleep files must be provided together (-b, -S, -c)"
    exit 1
fi

echo "[*] Setting up output path..."

# Set default output file if not provided
if [[ -z "$OUTPUT_FILE" ]]; then
    # Get the absolute path of the input image
    IMG_ABSOLUTE=$(realpath "$IMG_FILE")
    IMG_DIR=$(dirname "$IMG_ABSOLUTE")
    IMG_NAME=$(basename "$IMG_ABSOLUTE")
    
    # Remove extension and add -patched
    IMG_BASE="${IMG_NAME%.*}"
    IMG_EXT="${IMG_NAME##*.}"
    
    # Create output path in same directory as input
    OUTPUT_FILE="${IMG_DIR}/${IMG_BASE}-patched.${IMG_EXT}"
fi

echo "  ✓ Output will be: $OUTPUT_FILE"

# Check if output file already exists
if [[ -f "$OUTPUT_FILE" ]]; then
    echo "Error: Output file already exists: $OUTPUT_FILE"
    echo "Please remove it or specify a different output path with -o"
    exit 1
fi

echo ""
echo "[*] Creating working directory..."

# Handle custom or temporary working directory
if [[ ! -z "$WORK_DIR_CUSTOM" ]]; then
    # Validate custom working directory
    if [[ ! -d "$WORK_DIR_CUSTOM" ]]; then
        echo "Error: Custom working directory does not exist: $WORK_DIR_CUSTOM"
        exit 1
    fi
    
    # Check if directory is empty
    if [[ -n "$(ls -A "$WORK_DIR_CUSTOM" 2>/dev/null)" ]]; then
        echo "Error: Custom working directory is not empty: $WORK_DIR_CUSTOM"
        echo "Please provide an empty directory"
        exit 1
    fi
    
    WORK_DIR="$WORK_DIR_CUSTOM"
    echo "  ✓ Using custom working directory: $WORK_DIR"
else
    WORK_DIR=$(mktemp -d -t work_$(basename "$IMG_FILE").XXXX)
    echo "  ✓ Created temporary working directory: $WORK_DIR"
fi

# Check available space (need at least 16GB)
REQUIRED_SPACE_GB=16
REQUIRED_SPACE_KB=$((REQUIRED_SPACE_GB * 1024 * 1024))
AVAILABLE_SPACE_KB=$(df -k "$WORK_DIR" | tail -1 | awk '{print $4}')
AVAILABLE_SPACE_GB=$((AVAILABLE_SPACE_KB / 1024 / 1024))

echo "[*] Checking available disk space..."
echo "  Available: ${AVAILABLE_SPACE_GB}GB"
echo "  Required:  ${REQUIRED_SPACE_GB}GB"

if [[ $AVAILABLE_SPACE_KB -lt $REQUIRED_SPACE_KB ]]; then
    echo "Error: Insufficient disk space in working directory"
    echo "  Location: $WORK_DIR"
    echo "  Available: ${AVAILABLE_SPACE_GB}GB"
    echo "  Required: ${REQUIRED_SPACE_GB}GB"
    echo "Please free up space or use -w to specify a different working directory"
    exit 1
fi

echo "  ✓ Sufficient space available"

WORKING_IMG="$WORK_DIR/working_image.img"
EXTRACT_DIR="$WORK_DIR/rootfs_extract"
MOUNT_DIR="$WORK_DIR/mount"

echo "[*] Original image: $IMG_FILE"
echo "[*] Output will be saved to: $OUTPUT_FILE"

# --- Copy the original image ---
echo "[*] Copying original image to working directory..."
cp "$IMG_FILE" "$WORKING_IMG"
echo "[+] Image copied successfully"

# --- Cleanup function ---
cleanup() {
    local exit_code=$?
    echo "[*] Cleaning up..."
    
    # Unmount if mounted
    if mountpoint -q "$MOUNT_DIR" 2>/dev/null; then
        echo "[*] Unmounting $MOUNT_DIR..."
        sudo umount "$MOUNT_DIR" 2>/dev/null || true
    fi
    
    # Detach loop device if attached
    if [[ ! -z "$LOOP" ]] && losetup "$LOOP" 2>/dev/null; then
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
    fi
    
    exit $exit_code
}

# Set trap to call cleanup on exit, interrupt, or termination
trap cleanup EXIT INT TERM

# --- Check if extraction directory already exists with contents ---
if [[ -d "$EXTRACT_DIR" ]] && [[ -n "$(ls -A "$EXTRACT_DIR" 2>/dev/null)" ]]; then
    echo "[-] Error: Extraction directory already exists with contents: $EXTRACT_DIR"
    echo "[-] This may indicate a previous run was interrupted or there's a conflict."
    echo "[-] Please remove the directory manually if you're sure it's safe to do so:"
    echo "[-]   rm -rf \"$EXTRACT_DIR\""
    exit 1
fi

# --- Setup loop device ---
LOOP=$(sudo losetup -Pf --show "$WORKING_IMG")
echo "[*] Loop device: $LOOP"

# --- Mount root partition ---
ROOT_PART="${LOOP}p1"
mkdir -p "$MOUNT_DIR"
sudo mount "$ROOT_PART" "$MOUNT_DIR"
echo "[*] Mounted root partition at $ROOT_PART"

# --- Extract rootfs.tar.gz ---
ROOTFS_TAR_GZ=$(sudo find "$MOUNT_DIR" -name "rootfs.tar.gz" 2>/dev/null | head -n1)
if [[ -z "$ROOTFS_TAR_GZ" ]]; then
    echo "[-] rootfs.tar.gz not found!"
    exit 1
fi

# Ungzip the rootfs.tar.gz
echo "[*] Extracting $ROOTFS_TAR_GZ..."
ROOTFS_TAR="$WORK_DIR/rootfs.tar"
sudo gunzip -c "$ROOTFS_TAR_GZ" > "$ROOTFS_TAR"

# Extract only what we need to modify
mkdir -p "$EXTRACT_DIR"
sudo tar -xf "$ROOTFS_TAR" -C "$EXTRACT_DIR" ./etc/ssh/authorized_keys ./home/rewt ./etc/shadow 2>/dev/null || true

# --- Detect rewt UID/GID ---
REWT_HOME="$EXTRACT_DIR/home/rewt"
if [[ -d "$REWT_HOME" ]]; then
    REWT_UID=$(sudo stat -c "%u" "$REWT_HOME")
    REWT_GID=$(sudo stat -c "%g" "$REWT_HOME")
    echo "[+] rewt detected, UID=$REWT_UID GID=$REWT_GID"
else
    # Default to common embedded Linux UID/GID
    REWT_UID=1001
    REWT_GID=1001
    echo "[+] Using default UID=$REWT_UID GID=$REWT_GID"
fi

# Create a staging directory for files to append
STAGING_DIR="$WORK_DIR/staging"
mkdir -p "$STAGING_DIR"

# --- Append new public key ---
echo "[*] Preparing SSH authorized_keys..."
AK_FILE="$STAGING_DIR/etc/ssh/authorized_keys"
sudo mkdir -p "$(dirname "$AK_FILE")"

# Copy existing keys if they exist, otherwise create new
if sudo tar -xf "$ROOTFS_TAR" -C "$STAGING_DIR" ./etc/ssh/authorized_keys 2>/dev/null; then
    echo "  ✓ Existing authorized_keys found"
else
    sudo mkdir -p "$STAGING_DIR/etc/ssh"
    sudo touch "$AK_FILE"
fi

# Remove Eight Sleep keys if requested
if [[ "$REMOVE_EIGHTSLEEP_KEYS" == true ]]; then
    echo "[*] Removing Eight Sleep SSH keys..."
    echo "[!] WARNING: This will prevent Eight Sleep mobile app pairing!"
    echo "[!] Eight Sleep remote access will be disabled"
    
    # Create new authorized_keys with only the user's key
    # Copy the user's public key
    sudo cp "$PUBKEY_FILE" "$AK_FILE"
    
    # Ensure file ends with newline (SSH keys should end with newline)
    sudo bash -c "echo '' >> '$AK_FILE'"
    
    echo "  ✓ Eight Sleep keys removed, only your key remains"
else
    # Append new public key to existing keys
    sudo bash -c "cat '$PUBKEY_FILE' >> '$AK_FILE'"
    echo "  ✓ Your key appended to existing keys"
fi

sudo chmod 600 "$AK_FILE"
sudo chown "$REWT_UID:$REWT_GID" "$AK_FILE"

# --- Set password if provided ---
if [[ ! -z "$PASSWORD" ]]; then
    echo "[*] Setting password for rewt..."
    echo "[!] WARNING: Password modification is experimental and may cause SSH issues"
    echo "[!] If SSH stops working, re-patch without -P flag"
    
    # Extract shadow and sshd_config files
    if ! sudo tar -xf "$ROOTFS_TAR" -C "$STAGING_DIR" ./etc/shadow ./etc/ssh/sshd_config 2>/dev/null; then
        echo "[-] WARNING: Failed to extract /etc/shadow or /etc/ssh/sshd_config from tar"
        echo "[-] Skipping password modification to avoid breaking SSH"
        echo "[-] You can still use SSH key authentication"
    else
        SHADOW_FILE="$STAGING_DIR/etc/shadow"
        SSHD_CONFIG="$STAGING_DIR/etc/ssh/sshd_config"
        
        PASSWORD_SET=false
        
        if [[ -f "$SHADOW_FILE" ]]; then
            # Get original permissions and ownership before modification
            SHADOW_PERMS=$(sudo stat -c "%a" "$SHADOW_FILE")
            SHADOW_OWNER=$(sudo stat -c "%u:%g" "$SHADOW_FILE")
            
            echo "  [*] Original shadow file: perms=$SHADOW_PERMS owner=$SHADOW_OWNER"
            
            # Validate file has content (check if rewt entry exists)
            if ! sudo grep -q "^rewt:" "$SHADOW_FILE" 2>/dev/null; then
                echo "[-] WARNING: /etc/shadow doesn't contain rewt entry, skipping password modification"
                echo "[-] File may be empty or corrupted"
            else
                HASH=$(openssl passwd -6 "$PASSWORD")
                sudo sed -i "s|^rewt:[^:]*:|rewt:$HASH:|" "$SHADOW_FILE"
                
                # Restore original permissions and ownership
                sudo chmod "$SHADOW_PERMS" "$SHADOW_FILE"
                sudo chown "$SHADOW_OWNER" "$SHADOW_FILE"
                
                # Also write to mounted partition for immediate use
                sudo mkdir -p "$MOUNT_DIR/etc"
                sudo cp "$SHADOW_FILE" "$MOUNT_DIR/etc/shadow"
                sudo chmod "$SHADOW_PERMS" "$MOUNT_DIR/etc/shadow"
                sudo chown "$SHADOW_OWNER" "$MOUNT_DIR/etc/shadow"
                
                PASSWORD_SET=true
                echo "  ✓ Password set, permissions preserved"
            fi
        else
            echo "[-] WARNING: /etc/shadow not found, skipping password modification"
        fi
        
        if [[ -f "$SSHD_CONFIG" ]] && [[ "$PASSWORD_SET" = true ]]; then
            # Get original permissions and ownership before modification
            SSHD_PERMS=$(sudo stat -c "%a" "$SSHD_CONFIG")
            SSHD_OWNER=$(sudo stat -c "%u:%g" "$SSHD_CONFIG")
            
            echo "  [*] Original sshd_config: perms=$SSHD_PERMS owner=$SSHD_OWNER"
            
            # Enable password authentication (handle both commented and uncommented lines)
            sudo sed -i 's/^#*PasswordAuthentication.*/PasswordAuthentication yes/' "$SSHD_CONFIG"
            
            # Restore original permissions and ownership
            sudo chmod "$SSHD_PERMS" "$SSHD_CONFIG"
            sudo chown "$SSHD_OWNER" "$SSHD_CONFIG"
            
            # Also write to mounted partition for immediate use
            sudo mkdir -p "$MOUNT_DIR/etc/ssh"
            sudo cp "$SSHD_CONFIG" "$MOUNT_DIR/etc/ssh/sshd_config"
            sudo chmod "$SSHD_PERMS" "$MOUNT_DIR/etc/ssh/sshd_config"
            sudo chown "$SSHD_OWNER" "$MOUNT_DIR/etc/ssh/sshd_config"
            
            echo "  ✓ Password authentication enabled, permissions preserved"
        elif [[ "$PASSWORD_SET" = false ]]; then
            echo "[-] Skipping sshd_config modification (password not set)"
        else
            echo "[-] WARNING: /etc/ssh/sshd_config not found, skipping modification"
        fi
        
        if [[ "$PASSWORD_SET" = false ]]; then
            echo ""
            echo "[!] Password was NOT set due to errors"
            echo "[!] SSH key authentication will still work"
        fi
    fi
fi

# --- Create wpa_supplicant WiFi configuration ---
if [[ ! -z "$SSID" && ! -z "$PSK" ]]; then
    echo "[*] Creating wpa_supplicant WiFi configuration..."
    
    # Create wpa_supplicant configuration directory
    WPA_DIR="$STAGING_DIR/etc/wpa_supplicant"
    sudo mkdir -p "$WPA_DIR"
    
    # Create wpa_supplicant configuration for wlan0
    WPA_CONF="$WPA_DIR/wpa_supplicant-wlan0.conf"
    sudo bash -c "cat > '$WPA_CONF'" <<EOF
ctrl_interface=/var/run/wpa_supplicant
ctrl_interface_group=0
update_config=1

network={
    ssid="$SSID"
    psk="$PSK"
    key_mgmt=WPA-PSK
    priority=100
}
EOF
    sudo chmod 600 "$WPA_CONF"
    sudo chown "$REWT_UID:$REWT_GID" "$WPA_CONF"
    
    # Create systemd-networkd configuration for DHCP on wlan0
    NETWORKD_DIR="$STAGING_DIR/etc/systemd/network"
    sudo mkdir -p "$NETWORKD_DIR"
    
    NETWORKD_CONF="$NETWORKD_DIR/25-wlan0.network"
    sudo bash -c "cat > '$NETWORKD_CONF'" <<EOF
[Match]
Name=wlan0

[Network]
DHCP=yes
DNSSEC=no

[DHCP]
RouteMetric=100
UseDNS=yes
EOF
    sudo chmod 644 "$NETWORKD_CONF"
    sudo chown "$REWT_UID:$REWT_GID" "$NETWORKD_CONF"
    
    # Also write configs to the mounted root partition (for use before rootfs extraction)
    echo "[*] Writing WiFi configs to mounted partition..."
    sudo mkdir -p "$MOUNT_DIR/etc/wpa_supplicant"
    sudo mkdir -p "$MOUNT_DIR/etc/systemd/network"
    sudo cp "$WPA_CONF" "$MOUNT_DIR/etc/wpa_supplicant/"
    sudo cp "$NETWORKD_CONF" "$MOUNT_DIR/etc/systemd/network/"
    
    # Disable NetworkManager MAC randomization for wlan0
    NM_CONF_DIR="$STAGING_DIR/etc/NetworkManager/conf.d"
    sudo mkdir -p "$NM_CONF_DIR"
    NM_CONF="$NM_CONF_DIR/99-disable-wifi-mac-randomization.conf"
    sudo bash -c "cat > '$NM_CONF'" <<EOF
[device-mac-randomization]
# Disable MAC randomization for WiFi
wifi.scan-rand-mac-address=no

[connection-mac-randomization]
# Use permanent MAC address
ethernet.cloned-mac-address=permanent
wifi.cloned-mac-address=permanent
EOF
    sudo chmod 644 "$NM_CONF"
    sudo chown 0:0 "$NM_CONF"
    
    # Also write to mounted partition
    sudo mkdir -p "$MOUNT_DIR/etc/NetworkManager/conf.d"
    sudo cp "$NM_CONF" "$MOUNT_DIR/etc/NetworkManager/conf.d/"
    
    echo "[+] Created WiFi configuration for SSID: $SSID"
    echo "    ✓ NetworkManager MAC randomization disabled"
fi

# --- WiFi initialization service (must be created before MAC service can reference it) ---
if [[ ! -z "$SSID" ]]; then
    
    # --- Create opensleep-wifi service to initialize WiFi hardware ---
    echo "[*] Creating opensleep-wifi service..."
    
    # Check if variscite-wifi script exists on the mounted image
    if sudo test -f "$MOUNT_DIR/etc/wifi/variscite-wifi"; then
        # Create wifi directory in staging
        sudo mkdir -p "$STAGING_DIR/etc/wifi"
        
        # Copy variscite-wifi script as opensleep-wifi
        sudo cp "$MOUNT_DIR/etc/wifi/variscite-wifi" "$STAGING_DIR/etc/wifi/opensleep-wifi"
        
        # Bypass EEPROM check - modify wifi_is_available() function to always return 0
        # This is necessary because Pod 3's EEPROM reports WiFi as unavailable even though hardware is present
        sudo sed -i '/^wifi_is_available()/,/^}/{
            /opt=.*i2cget/,/fi$/c\
    # OpenSleep: Bypass EEPROM check - WiFi hardware is physically present\
    # Eight Sleep EEPROM has WiFi bit unset but hardware exists\
    return 0
        }' "$STAGING_DIR/etc/wifi/opensleep-wifi"
        
        # Ensure script is executable
        sudo chmod +x "$STAGING_DIR/etc/wifi/opensleep-wifi"
        
        # Create opensleep-wifi.service
        sudo mkdir -p "$STAGING_DIR/etc/systemd/system"
        OPENSLEEP_WIFI_SERVICE="$STAGING_DIR/etc/systemd/system/opensleep-wifi.service"
        sudo bash -c "cat > '$OPENSLEEP_WIFI_SERVICE'" <<'EOF'
[Unit]
Description=OpenSleep WiFi Initialization Service
Before=network.target
After=sysinit.target
ConditionPathExists=/etc/wifi/opensleep-wifi

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/etc/wifi/opensleep-wifi start
ExecStop=/etc/wifi/opensleep-wifi stop

[Install]
WantedBy=network.target
EOF
        sudo chmod 644 "$OPENSLEEP_WIFI_SERVICE"
        
        # Enable opensleep-wifi.service (create symlink in network.target.wants)
        sudo mkdir -p "$STAGING_DIR/etc/systemd/system/network.target.wants"
        sudo ln -sf /etc/systemd/system/opensleep-wifi.service "$STAGING_DIR/etc/systemd/system/network.target.wants/opensleep-wifi.service"
        
        # Mask variscite-wifi.service to prevent conflicts
        sudo ln -sf /dev/null "$STAGING_DIR/etc/systemd/system/variscite-wifi.service"
        
        # Also write to mounted partition for immediate use
        sudo mkdir -p "$MOUNT_DIR/etc/wifi"
        sudo cp "$STAGING_DIR/etc/wifi/opensleep-wifi" "$MOUNT_DIR/etc/wifi/"
        sudo cp "$OPENSLEEP_WIFI_SERVICE" "$MOUNT_DIR/etc/systemd/system/"
        sudo mkdir -p "$MOUNT_DIR/etc/systemd/system/network.target.wants"
        sudo ln -sf /etc/systemd/system/opensleep-wifi.service "$MOUNT_DIR/etc/systemd/system/network.target.wants/opensleep-wifi.service"
        sudo ln -sf /dev/null "$MOUNT_DIR/etc/systemd/system/variscite-wifi.service"
        
        echo "[+] opensleep-wifi service created and enabled"
        echo "    ✓ WiFi hardware will be initialized on boot"
        echo "    ✓ variscite-wifi.service masked (EEPROM check bypassed)"
    else
        echo "[!] WARNING: /etc/wifi/variscite-wifi not found in image"
        echo "    WiFi may not work without manual hardware initialization"
    fi
    
    # --- Configure persistent MAC address (requires opensleep-wifi service) ---
    if [[ ! -z "$MAC_ADDRESS" ]]; then
        echo "[*] Configuring persistent MAC address..."
        
        # brcmfmac WiFi driver generates random MAC at firmware load time
        # .link files don't work reliably, so we use a service to set MAC after interface is up
        
        SYSTEMD_DIR="$STAGING_DIR/etc/systemd/system"
        sudo mkdir -p "$SYSTEMD_DIR"
        
        # Create a service to set MAC address before networking starts
        MAC_SERVICE="$SYSTEMD_DIR/opensleep-mac.service"
        sudo bash -c "cat > '$MAC_SERVICE'" <<EOF
[Unit]
Description=OpenSleep Persistent MAC Address
Before=network-pre.target wpa_supplicant@wlan0.service
Wants=network-pre.target
After=opensleep-wifi.service sys-subsystem-net-devices-wlan0.device
BindsTo=sys-subsystem-net-devices-wlan0.device

[Service]
Type=oneshot
ExecStart=/sbin/ip link set dev wlan0 down
ExecStart=/sbin/ip link set dev wlan0 address $MAC_ADDRESS
ExecStart=/sbin/ip link set dev wlan0 up
RemainAfterExit=yes

[Install]
WantedBy=network.target
EOF
        sudo chmod 644 "$MAC_SERVICE"
        sudo chown 0:0 "$MAC_SERVICE"
        
        # Enable the service
        sudo mkdir -p "$SYSTEMD_DIR/network.target.wants"
        sudo ln -sf ../opensleep-mac.service "$SYSTEMD_DIR/network.target.wants/opensleep-mac.service"
        
        # Also write to mounted partition for immediate use
        sudo mkdir -p "$MOUNT_DIR/etc/systemd/system/network.target.wants"
        sudo cp "$MAC_SERVICE" "$MOUNT_DIR/etc/systemd/system/"
        sudo ln -sf ../opensleep-mac.service "$MOUNT_DIR/etc/systemd/system/network.target.wants/opensleep-mac.service"
        
        echo "[+] MAC address set to: $MAC_ADDRESS"
        echo "    MAC will persist across reboots and factory resets"
    fi
fi

# --- Enable SSH + Network Early Service ---
echo "[*] Installing ssh-early.service..."

SSH_EARLY_SERVICE="$STAGING_DIR/etc/systemd/system/ssh-early.service"
sudo mkdir -p "$(dirname "$SSH_EARLY_SERVICE")"

if [[ ! -z "$SSID" ]]; then
    # If WiFi is configured, wait for network and start SSH
    sudo bash -c "cat > '$SSH_EARLY_SERVICE'" <<EOF
[Unit]
Description=Force-enable SSH early in boot
After=network.target opensleep-wifi.service wpa_supplicant.service
Before=capybara.service variscite-bt.service
Wants=network-online.target
Requires=opensleep-wifi.service

[Service]
Type=oneshot
ExecStartPre=/bin/sleep 5
ExecStart=/bin/systemctl start sshd.socket
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
EOF
else
    # No WiFi configured, just start SSH
    sudo bash -c "cat > '$SSH_EARLY_SERVICE'" <<EOF
[Unit]
Description=Force-enable SSH early in boot
After=network-pre.target
Before=capybara.service variscite-bt.service

[Service]
Type=oneshot
ExecStart=/bin/systemctl start sshd.socket
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
EOF
fi

sudo chmod 644 "$SSH_EARLY_SERVICE"

# Create systemd symlink
SYSTEMD_WANTS="$STAGING_DIR/etc/systemd/system/multi-user.target.wants"
sudo mkdir -p "$SYSTEMD_WANTS"
sudo ln -sf ../ssh-early.service "$SYSTEMD_WANTS/ssh-early.service"

# Also install to mounted partition
sudo mkdir -p "$MOUNT_DIR/etc/systemd/system/multi-user.target.wants"
sudo cp "$SSH_EARLY_SERVICE" "$MOUNT_DIR/etc/systemd/system/"
sudo ln -sf ../ssh-early.service "$MOUNT_DIR/etc/systemd/system/multi-user.target.wants/ssh-early.service"

echo "[+] SSH early service installed and enabled."

# --- Enable wpa_supplicant for wlan0 interface ---
if [[ ! -z "$SSID" ]]; then
    echo "[*] Enabling wpa_supplicant@wlan0.service..."
    
    # Create wpa_supplicant service override to wait for opensleep-wifi
    WPA_OVERRIDE_DIR="$STAGING_DIR/etc/systemd/system/wpa_supplicant@wlan0.service.d"
    sudo mkdir -p "$WPA_OVERRIDE_DIR"
    
    # Add MAC service dependency only if MAC address was configured
    if [[ ! -z "$MAC_ADDRESS" ]]; then
        sudo bash -c "cat > '$WPA_OVERRIDE_DIR/override.conf'" <<'EOF'
[Unit]
After=opensleep-wifi.service opensleep-mac.service
Requires=opensleep-wifi.service
EOF
        echo "    ✓ wpa_supplicant will wait for opensleep-wifi and opensleep-mac"
    else
        sudo bash -c "cat > '$WPA_OVERRIDE_DIR/override.conf'" <<'EOF'
[Unit]
After=opensleep-wifi.service
Requires=opensleep-wifi.service
EOF
        echo "    ✓ wpa_supplicant will wait for opensleep-wifi"
    fi
    
    sudo chmod 644 "$WPA_OVERRIDE_DIR/override.conf"
    
    # Enable wpa_supplicant@wlan0.service
    sudo ln -sf /lib/systemd/system/wpa_supplicant@.service "$SYSTEMD_WANTS/wpa_supplicant@wlan0.service"
    
    # Also enable on mounted partition
    sudo mkdir -p "$MOUNT_DIR/etc/systemd/system/wpa_supplicant@wlan0.service.d"
    sudo cp "$WPA_OVERRIDE_DIR/override.conf" "$MOUNT_DIR/etc/systemd/system/wpa_supplicant@wlan0.service.d/"
    sudo ln -sf /lib/systemd/system/wpa_supplicant@.service "$MOUNT_DIR/etc/systemd/system/multi-user.target.wants/wpa_supplicant@wlan0.service"
    
    echo "[+] wpa_supplicant@wlan0.service enabled with opensleep-wifi dependency"
fi

# --- Create service to disable Eight Sleep services ---
# Auto-enable if opensleep is being installed
if [[ "$DISABLE_SERVICES" = true ]] || [[ ! -z "$OPENSLEEP_BINARY" ]]; then
    echo ""
    echo "⚠️  WARNING: Disabling Eight Sleep services ⚠️"
    echo "This will prevent the Eight Sleep app from pairing with the Pod."
    
    # Warn if opensleep is being installed without WiFi configuration
    if [[ -z "$SSID" ]]; then
        echo ""
        echo "⚠️⚠️⚠️  CRITICAL WARNING  ⚠️⚠️⚠️"
        echo "Installing opensleep without WiFi credentials!"
        echo "  - Pod 3 has no Ethernet port"
        echo "  - Without WiFi, the device will have NO network connectivity"
        echo "  - The device will be UNREACHABLE without serial console access"
        echo ""
        echo "Recommendation: Provide WiFi credentials with -s and -p flags"
        echo ""
        read -p "Continue anyway? (yes/no): " -r
        if [[ ! $REPLY =~ ^[Yy][Ee][Ss]$ ]]; then
            echo "Aborting..."
            cleanup
            exit 1
        fi
    fi
    
    echo "This will prevent the Eight Sleep app from pairing with the Pod."
    echo "To restore normal Eight Sleep functionality, you must reflash the original image."
    echo ""
    
    echo "[*] Installing disable-eightsleep-services.service..."

    DISABLE_SERVICE="$STAGING_DIR/etc/systemd/system/disable-eightsleep-services.service"
    sudo mkdir -p "$(dirname "$DISABLE_SERVICE")"
    
    if [[ ! -z "$OPENSLEEP_BINARY" ]]; then
        # If opensleep is installed, wait for it to be running before disabling Eight Sleep services
        sudo bash -c "cat > '$DISABLE_SERVICE'" <<EOF
[Unit]
Description=Disable Eight Sleep services after opensleep starts
After=multi-user.target opensleep.service
Wants=network-online.target
ConditionPathExists=!/var/lib/eightsleep-disabled.flag

[Service]
Type=oneshot
# Wait for opensleep to be fully running (max 60 seconds)
ExecStartPre=/bin/sh -c 'for i in \$(seq 1 60); do systemctl is-active opensleep.service && break || sleep 1; done'
# Disable Eight Sleep services
ExecStart=/bin/systemctl disable --now dac frank capybara swupdate-progress swupdate defibrillator
ExecStartPost=/bin/touch /var/lib/eightsleep-disabled.flag
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
EOF
    else
        # Without opensleep, just wait for network
        sudo bash -c "cat > '$DISABLE_SERVICE'" <<EOF
[Unit]
Description=Disable Eight Sleep services on first boot
After=multi-user.target network-online.target
Wants=network-online.target
ConditionPathExists=!/var/lib/eightsleep-disabled.flag

[Service]
Type=oneshot
ExecStart=/bin/systemctl disable --now dac frank capybara swupdate-progress swupdate defibrillator
ExecStartPost=/bin/touch /var/lib/eightsleep-disabled.flag
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
EOF
    fi

    sudo chmod 644 "$DISABLE_SERVICE"
    sudo ln -sf ../disable-eightsleep-services.service "$SYSTEMD_WANTS/disable-eightsleep-services.service"

    if [[ ! -z "$OPENSLEEP_BINARY" ]]; then
        echo "[+] Service to disable Eight Sleep services installed (waits for opensleep)."
    else
        echo "[+] Service to disable Eight Sleep services installed."
    fi
fi

# --- Install opensleep if provided ---
if [[ ! -z "$OPENSLEEP_BINARY" ]]; then
    echo "[*] Installing opensleep..."
    
    # Create /opt/opensleep directory
    OPENSLEEP_DIR="$STAGING_DIR/opt/opensleep"
    sudo mkdir -p "$OPENSLEEP_DIR"
    
    # Copy binary
    echo "[*] Copying opensleep binary..."
    sudo cp "$OPENSLEEP_BINARY" "$OPENSLEEP_DIR/opensleep"
    sudo chmod 755 "$OPENSLEEP_DIR/opensleep"
    sudo chown "$REWT_UID:$REWT_GID" "$OPENSLEEP_DIR/opensleep"
    
    # Copy config
    echo "[*] Copying config.ron..."
    sudo cp "$OPENSLEEP_CONFIG" "$OPENSLEEP_DIR/config.ron"
    sudo chmod 644 "$OPENSLEEP_DIR/config.ron"
    sudo chown "$REWT_UID:$REWT_GID" "$OPENSLEEP_DIR/config.ron"
    
    # Install service file
    echo "[*] Installing opensleep.service..."
    OPENSLEEP_SERVICE_DIR="$STAGING_DIR/lib/systemd/system"
    sudo mkdir -p "$OPENSLEEP_SERVICE_DIR"
    sudo cp "$OPENSLEEP_SERVICE" "$OPENSLEEP_SERVICE_DIR/opensleep.service"
    sudo chmod 644 "$OPENSLEEP_SERVICE_DIR/opensleep.service"
    
    # Enable the service
    echo "[*] Enabling opensleep service..."
    sudo ln -sf /lib/systemd/system/opensleep.service "$SYSTEMD_WANTS/opensleep.service"
    
    echo "[+] opensleep installed and enabled."
    echo "[*] Eight Sleep services will be automatically disabled after opensleep starts."
fi

# --- Add validation script ---
if [[ ! -z "$VALIDATION_SCRIPT" ]]; then
    echo "[*] Adding validation script to image..."
    
    # Create home directory structure
    sudo mkdir -p "$STAGING_DIR/home/rewt"
    
    # Copy validation script to user's home directory
    sudo cp "$VALIDATION_SCRIPT" "$STAGING_DIR/home/rewt/validate_deployment.sh"
    sudo chmod +x "$STAGING_DIR/home/rewt/validate_deployment.sh"
    sudo chown "$REWT_UID:$REWT_GID" "$STAGING_DIR/home/rewt/validate_deployment.sh"
    
    # Also copy to mounted partition for immediate use
    sudo mkdir -p "$MOUNT_DIR/home/rewt"
    sudo cp "$VALIDATION_SCRIPT" "$MOUNT_DIR/home/rewt/validate_deployment.sh"
    sudo chmod +x "$MOUNT_DIR/home/rewt/validate_deployment.sh"
    
    # Create .bashrc with welcome message
    sudo tee "$STAGING_DIR/home/rewt/.bashrc" > /dev/null <<'BASHRC_EOF'
# OpenSleep Pod 3 - Patched System

# Show validation prompt on first interactive login
if [ -f ~/validate_deployment.sh ] && [ -z "$VALIDATION_SHOWN" ]; then
    export VALIDATION_SHOWN=1
    echo ""
    echo "=========================================="
    echo "  OpenSleep Pod 3 - Welcome!"
    echo "=========================================="
    echo ""
    echo "A validation script is available to verify"
    echo "your system configuration."
    echo ""
    echo "Run: ~/validate_deployment.sh"
    echo "Or:  ./validate_deployment.sh"
    echo ""
    echo "To run opensleep in debug mode:"
    echo "  sudo systemctl stop opensleep && cd /opt/opensleep && sudo RUST_LOG=debug,rumqttc=info ./opensleep"
    echo ""
    echo "To skip this message in future sessions:"
    echo "  export VALIDATION_SHOWN=1"
    echo ""
fi
BASHRC_EOF
    
    sudo chmod 644 "$STAGING_DIR/home/rewt/.bashrc"
    sudo chown "$REWT_UID:$REWT_GID" "$STAGING_DIR/home/rewt/.bashrc"
    
    # Copy .bashrc to mounted partition
    sudo cp "$STAGING_DIR/home/rewt/.bashrc" "$MOUNT_DIR/home/rewt/.bashrc"
    
    echo "  ✓ Validation script added to /home/rewt/validate_deployment.sh"
    echo "  ✓ Welcome banner configured (shows script location on first login)"
fi

# --- Append modified files to rootfs.tar ---
echo "[*] Appending modified files to rootfs.tar..."

# List of files that might already exist in the tar and need to be replaced
# (tar --delete doesn't work on compressed archives, so we work with uncompressed tar)
FILES_TO_REPLACE=(
    "./etc/ssh/authorized_keys"
)

# Only delete shadow and sshd_config if we actually modified them (password was provided)
if [[ ! -z "$PASSWORD" ]]; then
    FILES_TO_REPLACE+=("./etc/shadow")
    FILES_TO_REPLACE+=("./etc/ssh/sshd_config")
fi

# Delete old versions of files that we're replacing (if they exist)
for file in "${FILES_TO_REPLACE[@]}"; do
    if sudo tar -tf "$ROOTFS_TAR" "$file" >/dev/null 2>&1; then
        echo "[*] Removing old $file from tar before updating..."
        sudo tar --delete -f "$ROOTFS_TAR" "$file" 2>/dev/null || true
    fi
done

# Use --numeric-owner to preserve UID/GID
sudo tar --numeric-owner -rf "$ROOTFS_TAR" -C "$STAGING_DIR" .

# --- Recompress rootfs.tar.gz ---
echo "[*] Recompressing rootfs.tar.gz..."
ROOTFS_PATCHED="$WORK_DIR/rootfs-patched.tar.gz"
sudo gzip -c "$ROOTFS_TAR" > "$ROOTFS_PATCHED"

# --- Update rootfs.tar.gz in mounted image ---
echo "[*] Updating rootfs.tar.gz in image..."
sudo cp "$ROOTFS_PATCHED" "$ROOTFS_TAR_GZ"

# --- Verify files ---
echo ""
echo "[*] Verifying patched files..."
if sudo test -f "$STAGING_DIR/etc/ssh/authorized_keys"; then
    echo "  ✓ SSH authorized_keys file created"
else
    echo "  ✗ Warning: Could not verify authorized_keys"
fi

if [[ ! -z "$PASSWORD" ]] && sudo test -f "$STAGING_DIR/etc/ssh/sshd_config"; then
    echo "  ✓ SSH configuration updated"
fi

if [[ ! -z "$SSID" ]] && sudo test -f "$STAGING_DIR/etc/wpa_supplicant/wpa_supplicant-wlan0.conf"; then
    echo "  ✓ WiFi connection configured: $SSID"
    if sudo test -f "$STAGING_DIR/etc/wifi/opensleep-wifi"; then
        echo "  ✓ opensleep-wifi service created (WiFi hardware initialization)"
    fi
fi

if [[ ! -z "$MAC_ADDRESS" ]]; then
    if sudo test -f "$STAGING_DIR/etc/systemd/system/opensleep-mac.service"; then
        echo "  ✓ Persistent MAC address configured: $MAC_ADDRESS"
    fi
fi

if [[ ! -z "$OPENSLEEP_BINARY" ]]; then
    if sudo test -f "$STAGING_DIR/opt/opensleep/opensleep"; then
        echo "  ✓ opensleep binary installed"
    fi
    if sudo test -f "$STAGING_DIR/opt/opensleep/config.ron"; then
        echo "  ✓ opensleep config.ron installed"
    fi
    if sudo test -f "$STAGING_DIR/lib/systemd/system/opensleep.service"; then
        echo "  ✓ opensleep.service installed"
    fi
fi

if [[ ! -z "$VALIDATION_SCRIPT" ]]; then
    if sudo test -f "$STAGING_DIR/home/rewt/validate_deployment.sh"; then
        echo "  ✓ Validation script included at /home/rewt/validate_deployment.sh"
    fi
fi

ROOTFS_SIZE=$(sudo stat -c%s "$ROOTFS_TAR_GZ")
ROOTFS_SIZE_MB=$((ROOTFS_SIZE / 1024 / 1024))
echo "  ✓ Repacked rootfs.tar.gz: ${ROOTFS_SIZE_MB}MB"

echo ""
# --- Move patched image to output location ---
echo "[*] Moving patched image to output location..."
sudo mv "$WORKING_IMG" "$OUTPUT_FILE"
sudo chown $(id -u):$(id -g) "$OUTPUT_FILE"
echo "[+] Patched image saved to: $OUTPUT_FILE"
echo ""
echo "======================================"
echo "[+] Patch workflow complete!"
echo "======================================"

if [[ ! -z "$SSID" ]]; then
    echo ""
    echo "WiFi Configuration:"
    echo "  - Network: $SSID"
    echo "  - opensleep-wifi service will initialize WiFi hardware"
    echo "  - wpa_supplicant will connect automatically on boot"
    echo "  - variscite-wifi.service masked (EEPROM check bypassed)"
fi

if [[ ! -z "$MAC_ADDRESS" ]]; then
    echo ""
    echo "MAC Address Configuration:"
    echo "  - Persistent MAC: $MAC_ADDRESS"
    echo "  - Configured via systemd .link file"
    echo "  - Will NOT change on factory reset"
    echo "  - Survives reboots and system updates"
fi

if [[ ! -z "$OPENSLEEP_BINARY" ]]; then
    echo ""
    echo "OpenSleep Configuration:"
    echo "  - opensleep binary installed to /opt/opensleep/"
    echo "  - opensleep.service will start on boot"
    echo "  - Eight Sleep services will be disabled after opensleep starts"
    if [[ ! -z "$SSID" ]]; then
        echo "  - Device will remain reachable via WiFi after Eight Sleep disabled"
    fi
fi

if [[ ! -z "$VALIDATION_SCRIPT" ]]; then
    echo ""
    echo "Validation Script:"
    echo "  - Script included at: /home/rewt/validate_deployment.sh"
    echo "  - Welcome banner will show script location on first SSH login"
    echo "  - Run with: ~/validate_deployment.sh or ./validate_deployment.sh"
    echo "  - Validates network, services, and configuration"
fi

echo ""
echo "[*] Flash patched SD card image and perform factory reset to apply changes."
echo "[*] Original image preserved at: $IMG_FILE"
