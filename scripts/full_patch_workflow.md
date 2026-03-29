# Pod 3 (SD Card) Image Patching Script - Documentation

## ⚠️ Important: Pod 3 SD Card Version ONLY

**This script is specifically designed for:**
- Eight Sleep Pod 3 with **removable SD card**

**This script does NOT work with:**
- ❌ Pod 1 (no SD card, no SSH access)
- ❌ Pod 2 (no SD card, no SSH access)
- ❌ Pod 3 eMMC version (non-removable storage, requires different method)
- ❌ Pod 4 (no SD card, see [free-sleep tutorial](https://github.com/throwaway31265/free-sleep))
- ❌ Pod 5 (no SD card, see [free-sleep tutorial](https://github.com/throwaway31265/free-sleep))

For Pod 3 without SD card or other Pod versions, see the [SETUP.md](../SETUP.md) file.

---

## Overview

The `full_patch_workflow.sh` script modifies Pod 3 SD card images to enable:
- WiFi auto-connect on boot
- SSH access via WiFi
- Optional OpenSleep installation (MQTT bridge)
- Optional disabling of Eight Sleep cloud services

This document explains what the script does and how the patched Pod 3 boots.

---

## Table of Contents

1. [What the Script Does](#what-the-script-does)
2. [Boot Sequence - Unpatched Pod 3](#boot-sequence---unpatched-pod-3)
3. [Boot Sequence - Patched Pod 3](#boot-sequence---patched-pod-3)
4. [Service Dependency Chain](#service-dependency-chain)
5. [The WiFi Problem and Solution](#the-wifi-problem-and-solution)
6. [Configuration Options](#configuration-options)
7. [Files Created/Modified](#files-createdmodified)
8. [Factory Reset Behavior](#factory-reset-behavior)
9. [Post-Patch Validation](#post-patch-validation)
10. [Troubleshooting](#troubleshooting)

---

## What the Script Does

### High-Level Process

1. **Mounts SD card image** - Uses loop device to access partitions
2. **Extracts rootfs.tar.gz** - Decompresses root filesystem archive
3. **Stages modifications** - Prepares all files to be added/modified
4. **Appends to tar archive** - Efficiently adds modifications using `tar -rf`
5. **Recompresses rootfs.tar.gz** - Creates patched archive
6. **Writes back to image** - Updates both archive and mounted partition
7. **Unmounts and cleans up** - Safely releases resources

### Key Features

**Efficient Tar Append Method:**
- Does NOT fully extract rootfs (would be ~2GB)
- Uses `tar -rf` to append new files
- Preserves file ownership with `--numeric-owner`
- Much faster than full extract/repack

**Dual-Write Strategy:**
- Writes configs to `rootfs.tar.gz` (survives factory reset)
- ALSO writes to mounted partition (works without factory reset)
- Ensures WiFi works in both scenarios

**Safety Features:**
- Validates disk space before starting
- Checks for required files (SSH keys, binaries)
- Warns if creating unreachable system (opensleep without WiFi)
- Confirmation prompts for destructive operations

---

## Boot Sequence - Unpatched Pod 3

### Standard Eight Sleep Boot (BROKEN WiFi)

```
1. Kernel loads
   └─> imx8mm-var-dart-eight (Variscite SOM)

2. systemd starts
   └─> sysinit.target

3. variscite-wifi.service attempts to start
   ├─> Runs /etc/wifi/variscite-wifi start
   ├─> Checks EEPROM (i2cget -f -y 0x0 0x52 0x20)
   ├─> EEPROM bit 0 = 0 (WiFi reported as "not available")
   └─> ❌ EXITS EARLY - No WiFi initialization!
       └─> wlan0 interface NEVER CREATED

4. variscite-bt.service starts
   ├─> Bluetooth hardware initialization
   └─> ✅ Bluetooth comes up successfully

5. network.target reached
   └─> No network interfaces available (eth0 doesn't exist, wlan0 not created)

6. Eight Sleep services start
   ├─> capybara.service (main control service)
   ├─> variscite-bt.service (Bluetooth - already running)
   └─> Other Eight Sleep services

7. Result:
   ❌ No WiFi connectivity (wlan0 never created)
   ✅ Bluetooth working (for phone app pairing)
   ❌ No network access initially
   ❌ Eight Sleep app cannot connect yet
   ⏳ Waiting for user to configure WiFi via Bluetooth...
```

### Normal User Flow (Without Patching)

**How Eight Sleep configures WiFi:**

```
1. User performs factory reset
   └─> Holds button, system resets to defaults

2. Pod 3 boots
   ├─> Bluetooth comes up (variscite-bt.service works)
   └─> WiFi hardware NOT initialized (variscite-wifi exits early)

3. User opens Eight Sleep app on phone
   └─> App searches for Pod via Bluetooth

4. Phone connects to Pod via Bluetooth
   ├─> Bluetooth pairing successful
   └─> Communication channel established

5. App prompts for WiFi credentials
   └─> User enters SSID and password in app

6. Phone sends WiFi config to Pod via Bluetooth
   ├─> capybara.service or another Eight Sleep service receives config
   └─> Stores WiFi credentials (not via wpa_supplicant initially)

7. Eight Sleep service configures WiFi
   ├─> Likely bypasses variscite-wifi completely
   ├─> May manually initialize WiFi hardware (GPIO, MMC, modprobe)
   ├─> OR uses a different WiFi management service
   └─> WiFi connects to network

8. Pod connects to Eight Sleep cloud services
   ├─> Checks for firmware updates
   └─> Registers device with cloud

9. If update available:
   ├─> Update downloaded from Eight Sleep servers
   ├─> Update applied (firmware, rootfs, etc.)
   └─> System reboots

10. Result:
    ✅ WiFi configured and connected
    ✅ Pod registered with Eight Sleep cloud
    ✅ App can control Pod
    ✅ Cloud services active
```

**Why variscite-wifi fails but Eight Sleep WiFi works:**

Eight Sleep's setup process **bypasses the variscite-wifi EEPROM check** by:
- Using Bluetooth for initial configuration (not relying on WiFi at first)
- Manually initializing WiFi hardware through capybara or custom service
- OR downloading updated variscite-wifi script during firmware update
- OR using entirely different WiFi management (ConnMan, custom daemon, etc.)

The EEPROM check failure is **intentional** - it prevents WiFi from auto-starting, forcing users to configure through the app (which ensures cloud registration).

**Why WiFi Fails:**
The Eight Sleep Pod 3 hardware HAS WiFi (Broadcom BCM4339/2 chip), but the EEPROM configuration reports it as unavailable. This appears to be intentional - Eight Sleep likely configures WiFi through a different mechanism during manufacturing or through their cloud services.

This is **intentional by design**:
- Prevents Pod from connecting to random WiFi networks
- Forces user to set up through Eight Sleep app
- Ensures device registers with Eight Sleep cloud during setup
- Allows Eight Sleep to control WiFi configuration process
- May download updated WiFi scripts during initial firmware update

---

## Boot Sequence - Patched Pod 3

### With WiFi Configuration Only (`-w -s`)

```
1. Kernel loads
   └─> imx8mm-var-dart-eight

2. systemd starts
   └─> sysinit.target

3. opensleep-wifi.service starts ✅
   ├─> Runs /etc/wifi/opensleep-wifi start
   ├─> Bypasses EEPROM check (always returns "available")
   ├─> Configures GPIO pins:
   │   ├─> GPIO 39 (WIFI_3V3) - 3.3V power rail
   │   ├─> GPIO 52 (WIFI_1V8) - 1.8V power rail
   │   ├─> GPIO 42 (WIFI_EN) - WiFi chip enable
   │   └─> GPIO 38 (BT_EN) - Bluetooth enable
   ├─> Power sequencing:
   │   ├─> Enable 3.3V → Wait 10ms
   │   ├─> Enable 1.8V → Wait 10ms
   │   ├─> Assert WIFI_EN
   │   ├─> Toggle BT_BUF (timing)
   │   └─> Deassert BT_EN
   ├─> MMC controller operations:
   │   ├─> Unbind WiFi from MMC: echo 30b40000.mmc > .../unbind
   │   ├─> Power up WiFi chip
   │   └─> Rebind to MMC: echo 30b40000.mmc > .../bind
   ├─> Load driver: modprobe brcmfmac
   ├─> Wait for wlan0 interface (up to 20 seconds, 3 retries)
   └─> ✅ SUCCESS - wlan0 interface created!

4. Kernel initializes WiFi driver
   ├─> brcmfmac: Firmware BCM4339/2 version 6.37.39.114 loaded
   ├─> cfg80211: Using regulatory domain US
   └─> wlan0 interface appears in /sys/class/net/

5. network.target reached
   └─> Network infrastructure ready

6. wpa_supplicant@wlan0.service starts ✅
   ├─> Waits for opensleep-wifi.service (dependency)
   ├─> Reads /etc/wpa_supplicant/wpa_supplicant-wlan0.conf
   ├─> Scans for configured SSID
   ├─> Authenticates with PSK (WPA2)
   ├─> Association successful
   └─> ✅ CTRL-EVENT-CONNECTED

7. systemd-networkd handles network configuration
   ├─> Reads /etc/systemd/network/25-wlan0.network
   ├─> Sends DHCP request
   ├─> Receives IP address from router
   └─> ✅ Network connectivity established

8. network-online.target reached
   └─> Network fully operational

9. ssh-early.service starts ✅
   ├─> Waits for opensleep-wifi.service (dependency)
   ├─> Waits 5 seconds (network stabilization)
   ├─> Starts sshd.socket
   └─> ✅ SSH now accessible over WiFi

10. Multi-user.target reached
    └─> System fully booted

11. Result:
    ✅ WiFi connected to configured network
    ✅ IP address obtained via DHCP
    ✅ SSH accessible: ssh rewt@<pod3-ip>
    ✅ Eight Sleep services still running (app works)
    ✅ Device accessible for development/debugging
```

### With WiFi + OpenSleep (`-w -s -b opensleep`)

```
1-8. [Same as WiFi-only boot above]

9. opensleep.service starts ✅
   ├─> Waits for network-online.target
   ├─> Runs /opt/opensleep/opensleep
   ├─> Reads /opt/opensleep/config.ron
   ├─> Connects to MQTT broker
   └─> ✅ Begins publishing Pod 3 sensor data

10. disable-eightsleep-services.service starts ✅
    ├─> Waits for opensleep.service to be active
    ├─> Timeout: 60 seconds
    ├─> Checks: systemctl is-active opensleep.service
    ├─> Once opensleep active:
    │   ├─> systemctl disable capybara.service
    │   ├─> systemctl disable variscite-bt.service
    │   ├─> systemctl disable eightsleep-*.service
    │   ├─> systemctl stop capybara.service
    │   └─> systemctl stop [other Eight Sleep services]
    └─> ✅ Eight Sleep cloud services disabled

11. ssh-early.service starts ✅
    └─> SSH accessible

12. Multi-user.target reached
    └─> System fully booted

13. Result:
    ✅ WiFi connected (via opensleep-wifi, NOT variscite-wifi)
    ✅ opensleep running and publishing to MQTT
    ✅ Eight Sleep cloud services disabled
    ✅ Eight Sleep app CANNOT connect (local control only)
    ✅ SSH accessible over WiFi
    ✅ Pod 3 under full local control
```

**Critical Point:** WiFi continues to work after Eight Sleep services are disabled because:
- opensleep-wifi.service is independent of Eight Sleep
- variscite-wifi.service is masked (never runs)
- wpa_supplicant is a system service, not Eight Sleep specific
- Network infrastructure doesn't depend on Eight Sleep services

---

## Service Dependency Chain

### Visual Dependency Graph

```
Boot
 │
 ├──> sysinit.target
 │     │
 │     └──> opensleep-wifi.service
 │          ├─ Description: Initialize WiFi hardware (GPIO, MMC, driver)
 │          ├─ Type: oneshot
 │          ├─ RemainAfterExit: yes (stays "active")
 │          ├─ Before: network.target
 │          └─ Creates: wlan0 interface
 │               │
 ├──> network.target <─────┘
 │     │
 │     ├──> systemd-networkd.service
 │     │    └─ Manages: Network interface configuration
 │     │
 │     └──> wpa_supplicant@wlan0.service
 │          ├─ After: opensleep-wifi.service
 │          ├─ Requires: opensleep-wifi.service ← Hard dependency
 │          └─ Connects to WiFi network
 │               │
 ├──> network-online.target <─┘
 │     │
 │     └──> opensleep.service (if installed)
 │          ├─ After: network-online.target
 │          ├─ Runs: /opt/opensleep/opensleep
 │          └─ Provides: MQTT bridge for local control
 │               │
 │               └──> disable-eightsleep-services.service (if -b used)
 │                    ├─ After: opensleep.service
 │                    ├─ Waits: Until opensleep is active (60s timeout)
 │                    └─ Disables: Eight Sleep cloud services
 │
 └──> multi-user.target
       │
       ├──> ssh-early.service
       │    ├─ After: opensleep-wifi.service, wpa_supplicant.service
       │    ├─ Requires: opensleep-wifi.service
       │    ├─ Waits: 5 seconds for network stabilization
       │    └─ Starts: sshd.socket (SSH daemon)
       │
       └──> [Other services...]
```

### Service Relationships Explained

**opensleep-wifi.service:**
- **Purpose:** Initialize WiFi hardware (what variscite-wifi should do but doesn't)
- **Why RemainAfterExit=yes:** Service stays "active" so dependents know WiFi is ready
- **Why Before=network.target:** Must create wlan0 before network initialization
- **What it does:** GPIO setup, MMC binding, load brcmfmac driver, create wlan0

**wpa_supplicant@wlan0.service:**
- **Purpose:** Connect to WiFi network
- **Requires opensleep-wifi:** Won't start if WiFi hardware init fails
- **After opensleep-wifi:** Waits until wlan0 exists
- **Template service:** @wlan0 specifies which interface to use

**ssh-early.service:**
- **Purpose:** Enable SSH access over WiFi
- **Requires opensleep-wifi:** Ensures WiFi hardware is initialized
- **5-second wait:** Allows network to stabilize before starting SSH
- **Why "early":** Starts before other services, enables debugging

**opensleep.service:**
- **Purpose:** MQTT bridge for local control
- **After network-online:** Waits for full network connectivity
- **Optional:** Only created if `-b` flag used

**disable-eightsleep-services.service:**
- **Purpose:** Disable Eight Sleep cloud connectivity
- **Conditional:** Only created if `-b` flag used or `-d` flag set
- **Waits for opensleep:** Ensures local control is working before disabling cloud
- **One-shot:** Runs once on first boot, then disables itself

---

## The WiFi Problem and Solution

### The Problem

**Eight Sleep's variscite-wifi.service fails on Pod 3:**

```bash
#!/bin/bash
# variscite-wifi script (simplified)

wifi_is_available() {
    # Read SOM options from EEPROM
    opt=$(i2cget -f -y 0x0 0x52 0x20)
    
    # Check bit 0 (WiFi present flag)
    if [ $((opt & 0x1)) -eq 1 ]; then
        return 0  # WiFi available
    else
        return 1  # WiFi NOT available
    fi
}

wifi_start() {
    # Exit early if EEPROM says no WiFi
    if ! wifi_is_available; then
        exit 0  # ← PROBLEM: Exits without initializing WiFi!
    fi
    
    # This code never runs because EEPROM check fails:
    # - GPIO setup
    # - MMC binding
    # - modprobe brcmfmac
    # - Wait for wlan0
}
```

**Why the EEPROM check fails:**
- Pod 3 hardware DOES have WiFi (Broadcom BCM4339/2 chip)
- EEPROM at I2C address 0x52, offset 0x20, bit 0 = 0 (WiFi not available)
- **Intentional by Eight Sleep** - not a hardware defect
- Forces WiFi setup through app (ensures cloud registration)
- Prevents Pod from connecting without going through Eight Sleep pairing flow
- variscite-wifi exits early, no WiFi initialization happens
- wlan0 interface never created
- wpa_supplicant can't start (no interface)
- No network connectivity until user configures via Bluetooth + app

### Eight Sleep's WiFi Configuration Method

**During normal setup (via Eight Sleep app):**

1. User factory resets Pod 3
2. Bluetooth comes up (variscite-bt.service works fine)
3. User opens Eight Sleep app on phone
4. Phone connects to Pod via Bluetooth
5. App sends WiFi credentials to Pod over Bluetooth
6. Eight Sleep service (likely capybara or custom daemon):
   - Receives WiFi config via Bluetooth
   - **Manually initializes WiFi hardware** (bypassing variscite-wifi)
   - OR triggers a different WiFi management service
   - OR downloads updated scripts during firmware update
7. Pod connects to Eight Sleep cloud
8. Firmware/config updates applied if available
9. WiFi now works for cloud communication

**Why Eight Sleep does this:**
- ✅ Ensures device registration with cloud
- ✅ Controls first-time setup experience
- ✅ Prevents "rogue" pods from connecting without app
- ✅ Allows firmware updates during initial setup
- ✅ Can track device activation

**Why variscite-wifi fails but Eight Sleep WiFi works:**
- Eight Sleep bypasses variscite-wifi completely during setup
- Uses Bluetooth as initial communication channel
- Has alternative WiFi initialization in capybara or other service
- May download working WiFi scripts during firmware update

### The Solution

**Create opensleep-wifi service that bypasses EEPROM check:**

This replicates what Eight Sleep does internally (manual WiFi init) but makes it automatic on boot:

```bash
#!/bin/bash
# opensleep-wifi script (modified)

wifi_is_available() {
    # OpenSleep: Bypass EEPROM check
    # WiFi hardware IS physically present
    # Eight Sleep EEPROM has bit unset to force app-based setup
    # We bypass this to enable direct WiFi configuration
    return 0  # Always return "available"
}

wifi_start() {
    # Now this code DOES run:
    if ! wifi_is_available; then  # Always passes now
        exit 0
    fi
    
    # GPIO initialization (runs)
    # MMC binding (runs)
    # modprobe brcmfmac (runs)
    # Wait for wlan0 (runs)
    # ✅ WiFi works!
}
```

**What we're doing:**
- Replicating Eight Sleep's internal WiFi initialization
- Bypassing the app-based setup requirement
- Allowing direct WiFi configuration via wpa_supplicant
- Skipping Eight Sleep cloud registration

**Trade-offs:**
- ✅ WiFi auto-connects on boot (no app needed)
- ✅ SSH accessible immediately
- ✅ Can use local control (opensleep/MQTT)
- ⚠️ Bypasses Eight Sleep's intended setup flow
- ⚠️ Doesn't register with Eight Sleep cloud (unless you want it to)
- ⚠️ Eight Sleep app won't work (if you disable Eight Sleep services)

**How we implement this:**

1. **Copy variscite-wifi script** to `/etc/wifi/opensleep-wifi`
2. **Modify with sed** to replace EEPROM check with `return 0`
3. **Create opensleep-wifi.service** to run the modified script
4. **Mask variscite-wifi.service** to prevent conflicts
5. **Update dependencies** so services wait for opensleep-wifi, not variscite-wifi

**Result:**
- WiFi hardware gets initialized on boot
- wlan0 interface appears
- wpa_supplicant can connect
- SSH becomes accessible
- Network connectivity works

---

## Configuration Options

### Required Arguments

```bash
-i IMAGE_FILE    # Path to SD card image file
-k PUBKEY_FILE   # SSH public key for 'rewt' user authentication
```

### Optional Arguments

```bash
# WiFi Configuration
-w WIFI_SSID          # WiFi network name
-s WIFI_PASSWORD      # WiFi network password/passphrase

# MAC Address Configuration
-m MAC_ADDRESS        # Set persistent MAC address (format: AA:BB:CC:DD:EE:FF)
                      # Prevents MAC from changing on factory reset

# SSH Configuration  
-P ROOT_PASSWORD      # Set password for 'rewt' user (enables password auth)

# OpenSleep Installation
-b OPENSLEEP_BINARY   # Path to opensleep binary to install
-S OPENSLEEP_SERVICE  # Path to custom opensleep.service file
-c OPENSLEEP_CONFIG   # Path to custom config.ron file

# Validation Script
-v VALIDATION_SCRIPT  # Path to validation script (default: auto-detect in same directory)

# Eight Sleep Services
-d                    # Disable Eight Sleep services (even without -b)

# Output Control
-o OUTPUT_FILE        # Custom output path (default: <input>-patched.img)
-w WORK_DIR          # Custom working directory (needs 16GB free)

# Help
-h                   # Show usage information
```

### Validation Script Option

The script can automatically include a validation script in the patched image:

**Auto-detection (default):**
- Looks for `full_patch_workflow_post_ssh_validation.sh` in the same directory
- If found, automatically includes it in the image
- Placed at `/home/rewt/validate_deployment.sh` on the device
- Welcome banner shows script location on first SSH login

**Manual specification:**
```bash
-v /path/to/custom_validation.sh
```

**Skip validation script:**
- Simply don't have the validation script in the same directory
- Or use `-v ""` to explicitly skip

**After patching with validation script:**
```bash
# SSH into Pod 3
ssh rewt@<pod-ip>

# You'll see a welcome banner:
==========================================
  OpenSleep Pod 3 - Welcome!
==========================================

A validation script is available to verify
your system configuration.

Run: ~/validate_deployment.sh
Or:  ./validate_deployment.sh

To skip this message in future sessions:
  export VALIDATION_SHOWN=1

# Run validation
./validate_deployment.sh
```

**Note:** This validation is specific to Pod 3 with SD card. Other Pod versions have different configurations.

See [Post-Patch Validation](#post-patch-validation) for details on what the script checks.

### Common Usage Patterns

**WiFi + SSH only:**
```bash
./full_patch_workflow.sh \
    -i pod3.img \
    -k ~/.ssh/id_rsa.pub \
    -w "MyNetwork" \
    -s "MyPassword"
```
→ WiFi auto-connects, SSH accessible, Eight Sleep services still work

**WiFi + SSH + Persistent MAC:**
```bash
./full_patch_workflow.sh \
    -i pod3.img \
    -k ~/.ssh/id_rsa.pub \
    -w "MyNetwork" \
    -s "MyPassword" \
    -m "02:11:22:33:44:55"
```
→ WiFi auto-connects, MAC address stays the same after factory resets

**WiFi + SSH + OpenSleep + Validation:**
```bash
./full_patch_workflow.sh \
    -i pod3.img \
    -k ~/.ssh/id_rsa.pub \
    -w "MyNetwork" \
    -s "MyPassword" \
    -b ./opensleep
```
→ Full local control, Eight Sleep services disabled, WiFi works, validation script included (auto-detected)

**With custom validation script:**
```bash
./full_patch_workflow.sh \
    -i pod3.img \
    -k ~/.ssh/id_rsa.pub \
    -w "MyNetwork" \
    -s "MyPassword" \
    -v ./custom_validation.sh
```
→ Uses custom validation script instead of default

**OpenSleep without WiFi (NOT RECOMMENDED):**
```bash
./full_patch_workflow.sh \
    -i pod3.img \
    -k ~/.ssh/id_rsa.pub \
    -b ./opensleep
```
→ ⚠️ WARNING: Device will be unreachable, you will need to configure wifi via the app (no Ethernet on Pod 3)

---

## Files Created/Modified

### SSH Configuration

**Created:**
- `/etc/ssh/authorized_keys` - SSH public key for 'rewt' user
- `/etc/systemd/system/ssh-early.service` - Enable SSH early in boot

**Modified:**
- `/etc/ssh/sshd_config` - Enable key authentication, optionally password auth

### WiFi Configuration (if `-w -s` provided)

**Created:**
- `/etc/wifi/opensleep-wifi` - Modified variscite-wifi script (EEPROM bypass)
- `/etc/systemd/system/opensleep-wifi.service` - WiFi hardware initialization
- `/etc/systemd/system/network.target.wants/opensleep-wifi.service` - Enable service
- `/etc/wpa_supplicant/wpa_supplicant-wlan0.conf` - WiFi credentials (SSID/PSK)
- `/etc/systemd/network/25-wlan0.network` - DHCP configuration for wlan0
- `/etc/systemd/system/wpa_supplicant@wlan0.service.d/override.conf` - Service dependencies
- `/etc/systemd/system/multi-user.target.wants/wpa_supplicant@wlan0.service` - Enable service

**Masked:**
- `/etc/systemd/system/variscite-wifi.service` → `/dev/null` - Prevent Eight Sleep WiFi from running

### MAC Address Configuration (if `-m` provided)

**Created:**
- `/etc/systemd/system/opensleep-mac.service` - Service to set persistent MAC address
- `/etc/systemd/system/network.target.wants/opensleep-mac.service` - Enable service

**How it works:**
- brcmfmac WiFi driver generates random MAC at firmware load time
- systemd `.link` files don't work reliably with this driver
- `opensleep-wifi.service` loads the brcmfmac driver (modprobe brcmfmac)
- `opensleep-mac.service` runs **after** opensleep-wifi loads the driver
- Service sets MAC using `ip link set dev wlan0 address <MAC>`
- Runs **before** wpa_supplicant starts, so WiFi connects with correct MAC
- Persists across reboots, updates, and **factory resets**

**Service timing:**
```
opensleep-wifi.service
  └─> modprobe brcmfmac (loads driver, random MAC generated)
      └─> opensleep-mac.service
          └─> ip link set wlan0 down
          └─> ip link set wlan0 address <MAC>
          └─> ip link set wlan0 up
              └─> wpa_supplicant@wlan0.service (connects to WiFi with correct MAC)
```

**Why this approach:**
- `.link` files are processed by udev, but brcmfmac sets MAC during firmware init
- By the time udev processes the .link file, MAC is already set and can't be changed
- opensleep-wifi reloads the driver (modprobe), which generates a new random MAC
- We must set MAC **after** driver load but **before** wpa_supplicant starts
- Using a service with proper ordering dependencies is the only reliable way

**Why this matters:**
- By default, Pod 3 generates a random MAC on each factory reset
- This breaks MAC filtering/whitelisting on routers
- With `-m` option, MAC stays the same forever
- Useful for network security policies that require MAC whitelisting

**Example use cases:**
- Corporate/enterprise WiFi with MAC filtering
- Home network with MAC whitelist for security
- DHCP reservations based on MAC address
- Network access control lists (ACLs)

### OpenSleep Installation (if `-b` provided)

**Created:**
- `/opt/opensleep/opensleep` - OpenSleep binary
- `/opt/opensleep/config.ron` - Configuration file (MQTT broker, topics, etc.)
- `/lib/systemd/system/opensleep.service` - systemd service definition
- `/etc/systemd/system/multi-user.target.wants/opensleep.service` - Enable service

### Validation Script (if included)

**Created:**
- `/home/rewt/validate_deployment.sh` - Post-deployment validation script
- `/home/rewt/.bashrc` - Welcome banner showing script location

**Purpose:**
- Validates network connectivity (WiFi, IP, internet)
- Checks service status (opensleep-wifi, wpa_supplicant, ssh-early, opensleep)
- Verifies Eight Sleep services are disabled
- Confirms files are in correct locations
- Shows service logs if available

**Auto-included when:**
- `full_patch_workflow_post_ssh_validation.sh` exists in same directory as patching script
- Or explicitly specified with `-v` option

**User Experience:**
- Welcome banner on first SSH login shows script location
- Easy to run: `./validate_deployment.sh` or `~/validate_deployment.sh`
- Banner can be suppressed: `export VALIDATION_SHOWN=1`

### Eight Sleep Service Disabling (if `-b` or `-d` provided)

**Created:**
- `/etc/systemd/system/disable-eightsleep-services.service` - One-time service disabler
- `/etc/systemd/system/multi-user.target.wants/disable-eightsleep-services.service` - Enable

**Services Disabled (on first boot):**
- `capybara.service` - Eight Sleep main control service
- `variscite-bt.service` - Bluetooth service
- Other Eight Sleep cloud communication services

---

## Factory Reset Behavior

### What is Factory Reset?

**Physical Button:**
- Pod 3 has a reset button (power button) accessible on the device
- Connected to `bd718xx-pwrkey` GPIO (power management IC)
- Detected by kernel as `/dev/input/event1`

**How Factory Reset is Triggered:**

```
1. User holds reset button during power-on
   └─> Button state detected by bd718xx power management IC

2. Kernel boots, detects button press
   └─> GPIO state readable at /dev/input/event1

3. factory-reset.service starts (early in boot)
   ├─> Runs /usr/bin/factory-reset.sh or similar
   ├─> Checks button state (held = factory reset)
   └─> If button held:
       ├─> Output: "loading globals..."
       ├─> Output: "blinking leds..." (visual feedback)
       └─> Calls: install_yocto.sh -u

4. install_yocto.sh -u runs:
   ├─> "*** Variscite MX8 Yocto eMMC Recovery ***"
   ├─> Detects board: imx8mm-var-dart-eight
   ├─> Deletes current eMMC partitions
   ├─> Creates new partitions (A/B + cage style)
   ├─> Formats partitions
   └─> Extracts rootfs.tar.gz over root filesystem:
       └─> cd / && gunzip -c /dev/mmcblk2p1/rootfs.tar.gz | tar -xp

5. System reboots with fresh filesystem
```

**What Gets Reset:**
- ✅ All system files (from rootfs.tar.gz)
- ✅ Configuration in /etc
- ✅ Installed services
- ❌ User data in /var (may be preserved)
- ❌ SD card partition table (already set up)

**What Persists:**
- SD card remains formatted (partitions exist)
- rootfs.tar.gz remains on partition 1
- Hardware configuration (EEPROM)

Pod 3's "factory reset" button does:
```bash
#!/bin/bash
# Simplified factory reset process

# Extract rootfs over root filesystem
cd /
gunzip -c /dev/mmcblk2p1/rootfs.tar.gz | tar -xp

# Reboot
reboot
```

**Effect:** Restores system files from `rootfs.tar.gz`, but preserves:
- User data in `/var`
- Some config files
- Partition table

### Why We Write to Both Locations

The patching script uses a **dual-write strategy** because factory reset behavior depends on when it's triggered:

**Location 1: rootfs.tar.gz (SD card partition 1)**
- Modified files appended to tar archive
- Gzipped and written back to partition
- **Survives factory reset** ✅
- User does factory reset → rootfs.tar.gz extracted → our changes applied

**Location 2: Mounted root partition**
- Files written directly to `/etc`, `/opt`, etc.
- **Works immediately without factory reset** ✅
- User reimages SD card and boots → WiFi works right away
- **Gets overwritten if factory reset triggered** ⚠️

### How factory-reset.service Works

**Service behavior:**
```
[Unit]
Description=Factory Reset Service
# Runs early in boot, before most services

[Service]
Type=oneshot
ExecStart=/usr/bin/factory-reset.sh
# Checks if reset button (bd718xx-pwrkey) is held
# If held: extract rootfs.tar.gz and reboot
# If not held: exit quietly
```

**Important: Factory reset ALWAYS works, even if Eight Sleep services disabled!**
- factory-reset.service is a **system service**, not an Eight Sleep service
- Runs very early in boot (before network, before opensleep)
- Our patching script does NOT disable or modify it
- Provides a "safety net" - you can always factory reset to recover

**Why this is important:**
- If something goes wrong with opensleep or WiFi config
- If device becomes unreachable
- If you want to revert changes
- Factory reset button ALWAYS works → extracts original rootfs.tar.gz → system restored

**However, after factory reset:**
- If you patched rootfs.tar.gz: Your changes survive (WiFi, SSH, opensleep)
- If you only wrote to mounted partition: Changes lost, back to original state
- This is why we use **dual-write strategy**

**⚠️ Important: MAC Address Changes on Factory Reset**
- **Without `-m` option:** The wlan0 MAC address is randomly generated on each factory reset
- **With `-m` option:** MAC address stays the same (configured via systemd .link file)
- If you have MAC filtering on your router, either:
  - Use `-m` option to set a persistent MAC, OR
  - Update your router's whitelist after each factory reset
- The validation script displays the current MAC address
- Random MAC is normal Eight Sleep behavior (for privacy)

**Button detection:**
- Reads GPIO state from `/dev/input/event1` (bd718xx-pwrkey)
- Button held = factory reset triggered
- Button not held = normal boot continues
- Visual feedback: LEDs blink during reset process

**⚠️ IMPORTANT: Power Cycle Required After Factory Reset (Patched Images Only)**

When using our **patched Pod 3 image**, after factory reset you MUST power cycle the device:

1. **Hold reset button during power-on** (factory reset starts)
2. **Wait for LED sequence:**
   - Green blinking → Factory reset in progress
   - **Yellow blinking → Factory reset complete**
3. **⚠️ POWER OFF the device** (unplug power)
4. **Wait 5-10 seconds**
5. **Power ON the device** (plug power back in)
6. **Device will now boot normally** with patched configuration

**Why is this required?**
- Factory reset extracts files but some services may be in inconsistent state
- Power cycle ensures clean boot with all services starting correctly
- This is specific to our patched image (opensleep-wifi, ssh-early, opensleep services)
- **Not required for factory Eight Sleep images** (their reset process handles this)

**What if I don't power cycle?**
- Some services may not start correctly (WiFi, SSH, opensleep)
- Device may appear to hang or not connect to WiFi
- Simply power cycle to fix

**Reset process (when button held):**
```bash
#!/bin/bash
# Simplified factory-reset.sh logic

# Check if button held
if button_is_held; then
    echo "Factory reset triggered..."
    
    # Blink LEDs for visual feedback (green)
    blink_leds
    
    # Run Yocto eMMC recovery
    install_yocto.sh -u
    # This script:
    # - Repartitions eMMC (if needed)
    # - Formats partitions
    # - Extracts rootfs.tar.gz over /
    
    # LED changes to yellow when complete
    # ⚠️ USER MUST POWER CYCLE HERE (patched images only)
    
    # Reboot
    reboot
else
    echo "Normal boot, no factory reset"
    exit 0
fi
```

### Factory Reset Scenarios

**Scenario A: User reimages SD card, boots directly (no factory reset)**
```
1. SD card written with patched image
2. Pod 3 boots from SD card
3. Reads files from mounted partition
4. ✅ opensleep-wifi script present → WiFi works
5. ✅ SSH accessible immediately
```

**Scenario B: User reimages SD card, performs factory reset**
```
1. SD card written with patched image
2. User holds reset button
3. Factory reset extracts rootfs.tar.gz over /
4. opensleep-wifi script extracted from tar ✅
5. Pod 3 reboots
6. ✅ WiFi works after reset
7. ✅ SSH accessible
```

**Scenario C: User patches SD card in live Pod 3**
```
1. User removes SD card from Pod 3
2. Patches SD card on computer
3. Reinserts SD card
4. Factory reset (to load new rootfs.tar.gz)
5. ✅ All changes applied
```

---

## Troubleshooting

### WiFi Not Connecting

**Step 1: Check if opensleep-wifi was created**

Extract logs from SD card:
```bash
./extract_pod3_logs.sh -i patched_pod3.img -o ./logs
grep "opensleep-wifi" ./logs/system_info.txt
```

Should show:
```
Custom services found:
  ✓ opensleep-wifi.service
```

**Step 2: Check if opensleep-wifi.service started**

```bash
cat ./logs/journal/decoded_full.log | grep opensleep-wifi
```

Should show:
```
systemd[1]: Starting OpenSleep WiFi Initialization Service...
systemd[1]: Finished OpenSleep WiFi Initialization Service.
```

**Step 3: Check if wlan0 was created**

```bash
cat ./logs/journal/decoded_kernel.log | grep wlan0
```

Should show interface creation messages.

**Step 4: Check wpa_supplicant connection**

```bash
cat ./logs/journal/decoded_full.log | grep CTRL-EVENT-CONNECTED
```

Should show successful WiFi connection.

### Device Unreachable After Disabling Eight Sleep Services

**Symptom:** Installed opensleep without WiFi credentials, device now unreachable.

**Cause:** Pod 3 has no Ethernet port, only WiFi. Without WiFi config:
- variscite-wifi fails (EEPROM check)
- opensleep-wifi not created (no `-w -s` flags)
- No network connectivity
- No way to access device

**Solutions:**

**Option 1: Factory Reset (ALWAYS WORKS) ✅**
```bash
# Hold reset button during power-on
# Factory reset is a SYSTEM service, not Eight Sleep service
# It ALWAYS works, even if you disabled all Eight Sleep services
# 
# After reset:
# - If you patched rootfs.tar.gz: Your changes survive!
#   (WiFi, SSH, opensleep all come back)
# - Device will boot with your patched configuration
# - You can SSH in again to fix issues
```

**Why factory reset is your safety net:**
- factory-reset.service is NOT disabled by our patching script
- Runs before opensleep, before network, before everything
- Physical button → GPIO → systemd → extract rootfs.tar.gz
- Cannot be disabled by software changes
- Even if system is completely broken, factory reset works

**Option 2: Serial Console (if available)**
```bash
# Connect to serial console
screen /dev/ttyUSB0 115200

# Re-enable Eight Sleep services
systemctl enable capybara.service
systemctl start capybara.service

# Or configure WiFi manually
wpa_passphrase "SSID" "password" > /etc/wpa_supplicant/wpa_supplicant.conf
systemctl start wpa_supplicant@wlan0
```

**Option 3: Reimage SD Card**
```bash
# Write original Eight Sleep image
# Or write patched image WITH WiFi credentials this time
./full_patch_workflow.sh -i original.img -k key.pub -w "WiFi" -s "pass" -b opensleep
```

**Recommendation: Always use Option 1 (Factory Reset)**
- Fastest and easiest
- No need to open Pod or connect cables
- Works even if software is completely broken
- If you patched rootfs.tar.gz (which you did), all your changes come back
- Just hold button → wait 5 minutes → device back online with your patches

### SSH Connection Refused

**Symptom:** WiFi connected but SSH refuses connection.

**Check 1: Is sshd running?**
```bash
cat ./logs/journal/decoded_full.log | grep ssh-early
```

**Check 2: Is SSH key correct?**
```bash
cat ./logs/ssh/rewt_authorized_keys
# Compare with your public key
```

**Check 3: Try password authentication (if `-P` was used)**
```bash
ssh rewt@<pod3-ip>
# Enter password set with -P flag
```

### OpenSleep Not Starting

**Check 1: Is binary executable?**
```bash
# On Pod 3:
ls -la /opt/opensleep/opensleep
# Should show: -rwxr-xr-x
```

**Check 2: Check service status**
```bash
systemctl status opensleep.service
journalctl -u opensleep.service
```

**Check 3: Verify network connectivity**
```bash
ping 8.8.8.8  # Test internet
ping <mqtt-broker>  # Test MQTT broker
```

### Factory Reset Doesn't Apply Changes

**Symptom:** Factory reset but changes don't appear.

**Cause:** Changes only written to mounted partition, not rootfs.tar.gz.

**Verification:**
```bash
# Check if changes are in rootfs.tar.gz
mkdir /tmp/test
cd /tmp/test
tar -tzf /dev/mmcblk2p1/rootfs.tar.gz | grep opensleep-wifi
# Should show: ./etc/wifi/opensleep-wifi
```

**Solution:**
- Ensure patching script completed successfully
- Check that tar append step didn't fail
- Re-patch image

---

## Technical Details

### Efficient Tar Append Method

**Traditional approach (slow):**
```bash
# Extract entire rootfs (2+ GB)
gunzip -c rootfs.tar.gz | tar -x
# Modify files
# Repack entire rootfs
tar -czf rootfs.tar.gz .
```

**Our approach (fast):**
```bash
# Decompress once
gunzip -c rootfs.tar.gz > rootfs.tar

# Stage modifications in separate directory
mkdir staging
# ... create files in staging/ ...

# Append modifications only
tar --numeric-owner -rf rootfs.tar -C staging .

# Recompress
gzip -c rootfs.tar > rootfs-patched.tar.gz
```

**Benefits:**
- 10x faster (only append, don't extract)
- Uses less disk space
- Preserves original tar structure
- `--numeric-owner` preserves UID/GID (important for rewt user)

### User ID Management

**rewt user:**
- UID: 1001
- GID: 1001
- Home: /home/rewt
- Shell: /bin/bash

**Why important:**
Files must be owned by rewt (1001:1001) to work correctly:
```bash
# Wrong (breaks permissions):
chown root:root /home/rewt/.ssh/authorized_keys

# Correct:
chown 1001:1001 /home/rewt/.ssh/authorized_keys
```

Script uses `--numeric-owner` in tar to preserve these IDs.

### systemd Service Types

**Type=oneshot:**
- Used for: opensleep-wifi.service, ssh-early.service
- Runs once, then exits
- Can have `RemainAfterExit=yes` to stay "active"

**Type=simple:**
- Used for: opensleep.service
- Long-running daemon
- Process must stay in foreground

**Type=oneshot + ConditionPathExists:**
- Used for: disable-eightsleep-services.service
- Runs once on first boot only
- Disables itself after running

---

## Summary

### What Gets Configured

**With `-w -s` (WiFi):**
- ✅ opensleep-wifi.service initializes WiFi hardware
- ✅ wlan0 interface created on boot
- ✅ wpa_supplicant connects to configured network
- ✅ DHCP obtains IP address
- ✅ SSH accessible over WiFi

**With `-b` (OpenSleep):**
- ✅ opensleep binary installed
- ✅ opensleep.service starts on boot
- ✅ Eight Sleep services disabled after opensleep starts
- ✅ Local MQTT control available

**With `-w -s -b` (WiFi + OpenSleep):**
- ✅ Full local control
- ✅ WiFi works independently (survives Eight Sleep disable)
- ✅ SSH accessible
- ✅ Eight Sleep app disconnected
- ✅ Pod 3 under complete local control

**With validation script (auto or `-v`):**
- ✅ Validation script included at `/home/rewt/validate_deployment.sh`
- ✅ Welcome banner on first SSH login shows script location
- ✅ Can verify deployment after SSH'ing in
- ✅ Checks network, services, files, and configuration
- ✅ Survives factory reset (included in rootfs.tar.gz)

**With persistent MAC (`-m`):**
- ✅ MAC address configured via systemd .link file
- ✅ MAC stays the same across reboots and factory resets
- ✅ Useful for MAC filtering/whitelisting on routers
- ✅ Prevents network access issues after factory reset

### Boot Time Expectations

**First boot after patching:**
- WiFi initialization: ~5 seconds
- Network connection: ~10 seconds
- SSH available: ~20-30 seconds total

**After factory reset:**
- rootfs extraction: ~2-3 minutes
- Then same as above
- Total: ~3-4 minutes

### Next Steps After Patching

1. **Write image to SD card**
2. **Insert into Pod 3**
3. **Wait 1-2 minutes for boot**
4. **Check router for Pod 3's IP address**
5. **SSH to device:** `ssh rewt@<ip-address>`
6. **Run validation script:** See [Post-Patch Validation](#post-patch-validation)
7. **Verify opensleep (if installed):** `systemctl status opensleep`

---

## Post-Patch Validation

After successfully patching and booting the Pod 3 (SD card version), you can run a comprehensive validation script to verify everything is working correctly.

### Running the Validation Script

**If validation script was included during patching:**
```bash
# SSH into Pod 3
ssh rewt@<pod-ip>

# Welcome banner appears automatically:
==========================================
  OpenSleep Pod 3 - Welcome!
==========================================

A validation script is available to verify
your system configuration.

Run: ~/validate_deployment.sh
Or:  ./validate_deployment.sh

# Run validation (in home directory)
./validate_deployment.sh
```

**If you need to copy it manually:**
```bash
# Copy to Pod 3
scp scripts/full_patch_workflow_post_ssh_validation.sh rewt@<pod-ip>:/tmp/

# SSH in and run
ssh rewt@<pod-ip>
chmod +x /tmp/full_patch_workflow_post_ssh_validation.sh
/tmp/full_patch_workflow_post_ssh_validation.sh
```

**Note:** The patching script automatically includes the validation script if it's in the same directory. You can also specify a custom validation script with the `-v` option.

**Suppressing the welcome banner:**
```bash
# Add to .bashrc permanently
echo "export VALIDATION_SHOWN=1" >> ~/.bashrc

# Or set for current session only
export VALIDATION_SHOWN=1
```

### What the Validation Script Checks

**System Information:**
- ✅ Hostname and boot time
- ✅ **MAC address** (⚠️ changes on every factory reset!)

**Network Connectivity:**
- ✅ WiFi interface state (up/down)
- ✅ IP address assigned to wlan0
- ✅ Routing table configuration
- ✅ Internet connectivity (ping test to 8.8.8.8)

**Services That SHOULD Be Running:**
- ✅ `opensleep-wifi` - Creates wlan0 interface by bypassing EEPROM WiFi check
- ✅ `wpa_supplicant@wlan0` - Manages WiFi authentication and connection
- ✅ `ssh-early` - Enables SSH access early in boot process
- ✅ `opensleep` - Main opensleep daemon (if installed with `-b`)

**Services That Should NOT Be Running:**
- ✅ `variscite-wifi` - Should be masked (original Eight Sleep WiFi service)
- ✅ `capybara` - Should be disabled (Eight Sleep's main control daemon)

**Eight Sleep Services Status:**
- Checks: `dac`, `frank`, `capybara`, `swupdate-progress`, `swupdate`, `defibrillator`
- All should be disabled/inactive when opensleep is installed

**File Verification:**
- ✅ opensleep-wifi script exists at `/etc/wifi/opensleep-wifi`
- ✅ opensleep binary (if installed with `-b`)
- ✅ opensleep config.ron (if installed with `-b`)
- ✅ SSH keys (informational - password auth is also valid)

**Service Dependencies:**
- ✅ Verifies correct boot order chain
- ✅ Confirms ssh-early depends on opensleep-wifi
- ✅ Checks variscite-wifi is properly masked to `/dev/null`

### Expected Output

**Successful validation will show:**
```
========================================
Validation Summary
========================================
✓ wlan0 has an IP address
✓ opensleep-wifi service is active
✓ wpa_supplicant@wlan0 service is active
✓ ssh-early service is active
ℹ Using SSH password authentication
✓ variscite-wifi service is masked/inactive

=== Eight Sleep Services Status ===
✓ dac is disabled/inactive
✓ frank is disabled/inactive
✓ capybara is disabled/inactive
✓ swupdate-progress is disabled/inactive
✓ swupdate is disabled/inactive
✓ defibrillator is disabled/inactive

✓✓✓ All critical checks passed! ✓✓✓
Pod 3 patch deployment is successful.
========================================
```

### Interpreting Results

**Information Messages (ℹ):**
- Not errors, just informational
- Example: "Using SSH password authentication" - means you're using password auth instead of SSH keys (both are valid)

**Success Messages (✓):**
- Green checkmarks indicate everything working correctly
- Critical services are running
- Eight Sleep services are properly disabled

**Warning/Error Messages (⚠ or ✗):**
- Indicates something may need attention
- Review the specific service or file mentioned
- Check troubleshooting section below

**MAC Address Warning:**
- The validation script shows a warning that MAC address changes on factory reset
- This is normal Eight Sleep behavior, not a bug
- Important if you use MAC filtering on your router/firewall
- You'll need to update your whitelist after each factory reset

---

## Troubleshooting

### Common Issues

**Pod 3 not showing up on network after factory reset:**
1. Check if MAC address changed (run validation script)
2. Update MAC whitelist on router if you have MAC filtering enabled
3. Verify WiFi credentials are still correct
4. Check router logs for DHCP denials

**Pod 3 not showing up on network:**
1. Verify WiFi credentials are correct
2. Check router/AP supports 2.4GHz (Pod 3 doesn't support 5GHz)
3. Run validation script after SSH'ing via serial console
4. Check `journalctl -u opensleep-wifi` for errors

**SSH connection refused:**
1. Wait 30 seconds after boot completes
2. Verify you can ping the Pod 3's IP address
3. Check port 22 (default SSH port, not 8822)
4. Try serial console if available

**Factory reset doesn't restore patches:**
1. Verify you pressed button during boot (not before power on)
2. Check LED flashes yellow during reset (indicates extraction)
3. Wait full 5 minutes for extraction to complete
4. If patches not restored, rootfs.tar.gz may not have been written correctly

**opensleep service not running:**
1. Run validation script to check status
2. Check logs: `journalctl -u opensleep`
3. Verify binary exists: `ls -la /opt/opensleep/opensleep`
4. Check config exists: `ls -la /opt/opensleep/config.ron`

**WiFi connects but no internet:**
1. Check routing table: `route -n`
2. Verify gateway is reachable: `ping <gateway-ip>`
3. Test DNS: `ping 8.8.8.8`
4. Check wpa_supplicant: `systemctl status wpa_supplicant@wlan0`

### Getting Help

For issues or questions:
1. Run validation script and capture output
2. Extract logs: `./extract_pod3_logs.sh -i image.img`
3. Review `.memory-bank/` documentation
4. Check GitHub issues
5. Include validation script output when reporting issues
