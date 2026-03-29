# Pod 3 SD Card Patching Guide - Windows

This guide provides Windows-specific instructions for patching Pod 3 SD card images to enable SSH access and install OpenSleep.

## ⚠️ CRITICAL WARNING ⚠️

**DO NOT FORMAT THE SD CARD WHEN WINDOWS PROMPTS YOU!**

When you insert the SD card into your Windows computer, Windows may show a popup saying:
- "You need to format the disk in drive X: before you can use it"
- "The disk is not formatted"
- "Format disk now?"

**CLICK "CANCEL" OR CLOSE THE DIALOG!**

🛑 **DO NOT CLICK FORMAT!** 🛑

The SD card contains a Linux filesystem (ext4) that Windows cannot read. **The data IS there**, Windows just doesn't understand the Linux file system. If you format the disk, **all your data will be permanently lost** and you'll need to start over from scratch.

---

## Overview

At a high level, here is the process:
1. Create an image backup of the SD card
2. Copy the image to Ubuntu running in WSL
3. Run the patching script to modify the image
4. Write the modified image back to the SD card 


## Prerequisites

### Required Software

- **Windows Subsystem for Linux (WSL2)** with Ubuntu distribution
- **PowerShell** - Built into Windows
- **Win32 Disk Imager** - [Download here](https://win32diskimager.org/) for reading/writing SD card images

### Required Hardware

- **Micro SD Card Reader** - The author used [TS-RDF5K model](https://www.amazon.com/dp/B009D79VH4)
- **Your Pod 3 SD card**

### Optional Tools

- **ImageUSB by PassMark** - [Download here](https://www.osforensics.com/tools/write-usb-images.html) - Can create backups but not recommended for patching

### Installing WSL2

1. Open PowerShell as Administrator
2. Run the following command:
   ```powershell
   wsl --install
   ```
3. Restart your computer when prompted
4. Complete the Ubuntu setup when it launches
5. Create a username and password for your Ubuntu instance

---

## Step-by-Step Instructions

### Step 1: Create SD Card Image

1. Insert your Pod 3 SD card into the card reader
2. Open **Win32 Disk Imager**
3. Select the SD card device (verify it's the correct drive!)
4. Choose a location and filename for the image (e.g., `pod3_original.img`)
5. Click **Read** to create the image
6. Wait for the process to complete

**Note:** Win32 Disk Imager will read the entire SD card even if you select the second partition.

### Step 2: Gather Required Files

Before patching, collect these files:
- `pod3_original.img` - Image file from Step 1
- Your SSH public key (e.g., `~/.ssh/id_rsa.pub` or `~/.ssh/id_ed25519.pub`)
- `opensleep` - The opensleep binary
- `opensleep.service` - The systemd service file
- `config.ron` - Your opensleep configuration file
- `full_patch_workflow.sh` - The patching script from `scripts/`

### Step 3: Run the Patching Script

1. Open WSL (search for "Ubuntu" in Windows Start menu)
2. Navigate to where your files are located (Windows drives are mounted at `/mnt/`, e.g., `/mnt/c/Users/YourName/`)
3. Make the script executable:
   ```bash
   chmod +x ./full_patch_workflow.sh
   ```
4. Run the patching script with your arguments:

**Basic example:**
```bash
./full_patch_workflow.sh \
  -i pod3_original.img \
  -k ~/.ssh/id_rsa.pub \
  -s "YourWiFiSSID" \
  -p "YourWiFiPassword" \
  -b ./opensleep \
  -S ./opensleep.service \
  -c ./config.ron \
  -d
```

**Real-world example (with file paths):**
```bash
/mnt/f/workspaces/opensleep/scripts/full_patch_workflow.sh \
  -i "/mnt/f/drive images/pod3_original.img" \
  -k /mnt/c/Users/YourName/.ssh/id_ed25519.pub \
  -s "MyWiFiNetwork" \
  -p "MyWiFiPassword" \
  -b "/mnt/f/drive images/opensleep" \
  -S "/mnt/f/drive images/opensleep.service" \
  -c "/mnt/f/drive images/config.ron" \
  -d
```

**Script arguments:**
- `-i` - Input image file (required)
- `-k` - SSH public key file (required)
- `-s` - WiFi SSID (optional, but recommended)
- `-p` - WiFi password (optional, but recommended)
- `-m` - MAC address for wlan0 (optional, format: XX:XX:XX:XX:XX:XX)
- `-P` - Password for rewt user (optional, enables password authentication)
- `-b` - opensleep binary (optional)
- `-S` - opensleep.service file (optional)
- `-c` - config.ron file (optional)
- `-d` - Disable Eight Sleep services (optional, use with opensleep)

**⚠️ Note about `-d` flag:**
Using `-d` disables Eight Sleep services and prevents normal Eight Sleep app pairing. To restore Eight Sleep functionality, you must reflash the original image.

**📝 Note about `-P` flag:**
Using `-P` enables password authentication for the `rewt` user. The password will persist across factory resets. This is useful if you want both SSH key and password authentication available.

**📝 Note about `-m` flag:**
Setting a MAC address helps with router/firewall MAC filtering. The MAC address will persist across factory resets, but **WILL CHANGE** after each factory reset if not set (due to the Broadcom WiFi driver generating a random MAC).

The script will create a patched image named `pod3_original-patched.img` in the same directory as the input image.

### Step 4: Write Patched Image to SD Card

1. Insert your SD card into the card reader
2. Open **Win32 Disk Imager**
3. Select your patched image file (e.g., `pod3_original-patched.img`)
4. Select the SD card device
5. **⚠️ VERIFY IT'S THE CORRECT DRIVE!** 
   - Double-check by looking at "Safely Remove USB" in the taskbar
   - It should show 3 partitions for your SD card
6. Click **Write** to write the image
7. Wait for the process to complete
8. Safely eject the SD card

### Step 5: Factory Reset the Pod

1. Insert the SD card back into your Pod 3
2. Hold down the small button on the Pod Hub while powering it on
3. Keep holding until the factory reset process begins
4. The Pod will boot with your patched image

---

## Verification

After the Pod boots up:

1. **SSH Access (with key):**
   ```bash
   ssh rewt@<pod-ip-address> -p 8822
   ```

2. **SSH Access (with password, if -P was used):**
   ```bash
   ssh rewt@<pod-ip-address> -p 8822
   # Enter password when prompted
   ```
   
3. **Check opensleep status** (if installed):
   ```bash
   systemctl status opensleep
   ```

4. **Check WiFi connection:**
   ```bash
   nmcli connection show
   ```

5. **Check MAC address** (if -m was used):
   ```bash
   ip addr show wlan0 | grep ether
   ```

---

## Tested Features

The following features have been tested and confirmed working:

✅ **SSH Key Authentication** - Tested and working  
✅ **WiFi Auto-Connect** - Tested and working  
✅ **MAC Address Persistence** - Tested and working (survives factory reset)  
✅ **Password Authentication** - Tested and working (survives factory reset)  
✅ **Eight Sleep Service Disabling** - Tested and working  
✅ **opensleep Service** - Tested and working  
✅ **Factory Reset Survival** - All configurations persist after factory reset + power cycle  

**Note:** Password authentication (`-P` flag) was tested on November 2, 2025 and confirmed to work correctly. The validation bug that was rejecting legitimate root-owned shadow files has been fixed.

---

## Troubleshooting

### SD card not detected
- Try a different USB port
- Ensure the card reader is compatible with your SD card size

### Image too large
- The patched image should be approximately the same size as the original
- Ensure you have enough free space (requires ~16GB during patching)

### SSH connection fails
- Verify the Pod is on your network: `ping <pod-ip>`
- Check SSH port: `8822` (not the standard `22`)
- Ensure your public key was added correctly

### WiFi not connecting
- Verify SSID and password are correct
- Check router compatibility (2.4GHz network recommended)
- Look at Pod logs after SSH access: `journalctl -u ssh-early.service`

---

## Lessons Learned: MAC Address Configuration

### Background

Pod 3 uses the `brcmfmac` WiFi driver, which generates a **random MAC address** every time the firmware is loaded. This causes several issues:
- MAC address changes on every boot
- MAC address changes after factory reset
- DHCP reservations don't work reliably
- Network monitoring becomes difficult

### What Doesn't Work

❌ **systemd .link files** - The standard approach for setting persistent MAC addresses
- The `brcmfmac` driver ignores `.link` file MAC settings
- The driver loads at firmware initialization, before systemd-networkd processes `.link` files
- MAC is already set by the time `.link` files are evaluated

❌ **Running MAC service too early** - Before the driver is loaded
- Services in `sysinit.target` run before the WiFi driver loads
- Setting MAC before driver load results in "device not found" errors
- Service would need to run as ExecStart in opensleep-wifi, but that's messy

❌ **Hardcoded user/group IDs** - Using `0:0` for all files
- Some files (like `/etc/shadow`) require specific ownership
- Wrong ownership can corrupt authentication and break SSH completely
- Must preserve original permissions and ownership from tar extraction

### What Does Work

✅ **systemd service running AFTER WiFi driver loads + NetworkManager configuration**

The solution is a four-stage boot process:

1. **opensleep-wifi.service** - Loads the brcmfmac driver
   - Runs in `network.target` 
   - Bypasses EEPROM check (Pod 3 reports WiFi unavailable)
   - Initializes WiFi hardware with random MAC

2. **opensleep-mac.service** - Sets persistent MAC address
   - Runs `After=opensleep-wifi.service` (waits for driver)
   - Uses `/sbin/ip link set` commands (not `/usr/sbin/ip`)
   - Brings interface down, sets MAC, brings it back up
   - Runs `Before=wpa_supplicant@wlan0.service`

3. **NetworkManager** - Network management daemon
   - Configured to NOT randomize MAC addresses
   - Config: `/etc/NetworkManager/conf.d/99-disable-wifi-mac-randomization.conf`
   - Uses `wifi.cloned-mac-address=permanent` (respects our set MAC)
   - This was the missing piece - NetworkManager was overriding our MAC!

4. **wpa_supplicant@wlan0.service** - Connects to WiFi
   - Runs after both opensleep-wifi AND opensleep-mac complete
   - Override config: `After=opensleep-wifi.service opensleep-mac.service`
   - Uses the configured persistent MAC address

### Critical Implementation Details

**Service Dependencies:**
```ini
[Unit]
After=opensleep-wifi.service sys-subsystem-net-devices-wlan0.device
Before=network-pre.target wpa_supplicant@wlan0.service
BindsTo=sys-subsystem-net-devices-wlan0.device
```

**Correct IP Command Path:**
- Use `/sbin/ip` (NOT `/usr/sbin/ip`)
- Embedded systems have different PATH structures
- Wrong path causes exit code 203/EXEC failure

**Three-Step MAC Setting:**
```bash
ExecStart=/sbin/ip link set dev wlan0 down
ExecStart=/sbin/ip link set dev wlan0 address $MAC_ADDRESS
ExecStart=/sbin/ip link set dev wlan0 up
```

**NetworkManager Configuration (CRITICAL):**
- NetworkManager can override manually set MAC addresses
- Config file: `/etc/NetworkManager/conf.d/99-disable-wifi-mac-randomization.conf`
- Settings:
  ```ini
  [device-mac-randomization]
  wifi.scan-rand-mac-address=no
  
  [connection-mac-randomization]
  wifi.cloned-mac-address=permanent
  ```
- Without this, NetworkManager will randomize the MAC after opensleep-mac sets it!

**wpa_supplicant Override:**
- Must wait for BOTH opensleep-wifi AND opensleep-mac
- Override file: `/etc/systemd/system/wpa_supplicant@wlan0.service.d/override.conf`
- Only add opensleep-mac dependency if MAC address was configured (conditional)

**File Permission Preservation:**
- Always check original permissions: `stat -c "%a" file`
- Always check original ownership: `stat -c "%u:%g" file`
- Validate ownership before modification (error if `0:0` for shadow file)
- Restore exact permissions after modification
- Dual-write: staging directory for tar + mounted partition for immediate use

### Debugging Tips

**Check service status:**
```bash
systemctl status opensleep-wifi.service
systemctl status opensleep-mac.service
systemctl status wpa_supplicant@wlan0.service
```

**Verify MAC address:**
```bash
ip link show wlan0
cat /sys/class/net/wlan0/address
```

**Check service timing:**
```bash
systemd-analyze critical-chain opensleep-mac.service
```

**View boot logs:**
```bash
journalctl -u opensleep-wifi.service
journalctl -u opensleep-mac.service
```

### Factory Reset Behavior

The persistent MAC configuration survives factory reset because:
1. Service file is in `/etc/systemd/system/` (user space)
2. Factory reset extracts `rootfs.tar.gz` which includes our service
3. Tar duplicate prevention ensures latest version is used
4. Service runs on every boot, reapplying MAC

**Important:** After factory reset on patched images, you must **power cycle** (not just reboot):
- Factory reset LED: Green (in progress) → Yellow (complete)
- Power off the device when yellow LED shows
- Power back on to boot with patched configuration

---

## Additional Resources

- [Main SETUP.md](../../../../SETUP.md) - General setup instructions
- [OpenSleep README](../../../../README.md) - Project overview
- See the `scripts/full_patch_workflow.sh` for detailed script options (`-h` flag) 