#!/bin/bash

# Pod 3 (SD Card) Post-Patch Validation Script
# Run this on the Pod 3 after patching to validate the setup
# Usage: ./full_patch_workflow_post_ssh_validation.sh
#
# ⚠️  For Pod 3 with SD card only!

set -e

echo "========================================"
echo "Pod 3 (SD Card) Patch Validation"
echo "========================================"
echo ""

# === System Information ===
echo "=== System Information ==="
echo "Hostname: $(hostname 2>/dev/null || echo 'unknown')"
echo "Boot time: $(uptime -s 2>/dev/null || cat /proc/uptime | awk '{print int($1)}' | xargs -I {} echo '{} seconds ago')"
echo ""

# === MAC Address (Important: Changes on factory reset!) ===
echo "=== MAC Address ==="
WLAN_MAC=$(cat /sys/class/net/wlan0/address 2>/dev/null || echo "unknown")
echo "wlan0 MAC: $WLAN_MAC"
echo ""
echo "⚠️  IMPORTANT: MAC address changes after factory reset!"
echo "   If you have MAC filtering on your router/firewall,"
echo "   you'll need to update the whitelist after each reset."
echo ""

# === WiFi and Network Status ===
echo "=== WiFi Status ==="
# Try multiple commands to get WiFi status
if command -v iwconfig >/dev/null 2>&1; then
    iwconfig wlan0 2>/dev/null
elif command -v iw >/dev/null 2>&1; then
    iw dev wlan0 info 2>/dev/null
else
    echo "Interface state: $(cat /sys/class/net/wlan0/operstate 2>/dev/null || echo 'unknown')"
fi
echo ""

echo "=== IP Address ==="
# Try ip command first, fall back to ifconfig or cat /proc/net
if command -v ip >/dev/null 2>&1; then
    ip addr show wlan0
elif command -v ifconfig >/dev/null 2>&1; then
    ifconfig wlan0
else
    echo "Checking via /sys filesystem..."
    if [ -d /sys/class/net/wlan0 ]; then
        echo "wlan0 exists"
        echo "MAC Address: $(cat /sys/class/net/wlan0/address 2>/dev/null)"
        echo "State: $(cat /sys/class/net/wlan0/operstate 2>/dev/null)"
        # Try to get IP from busybox or other tools
        if command -v busybox >/dev/null 2>&1; then
            busybox ifconfig wlan0
        fi
    else
        echo "wlan0 interface not found"
    fi
fi
echo ""

echo "=== Routing Table ==="
if command -v ip >/dev/null 2>&1; then
    ip route
elif command -v route >/dev/null 2>&1; then
    route -n
elif command -v netstat >/dev/null 2>&1; then
    netstat -rn
else
    echo "No routing tools available"
fi
echo ""

echo "=== Internet Connectivity ==="
ping -c 3 8.8.8.8 2>/dev/null || echo "No internet connectivity or ping not available"
echo ""

# === Service Status ===
echo "=========================================="
echo "Services That SHOULD Be Running"
echo "=========================================="
echo ""

echo "=== opensleep-wifi Service ==="
echo "Purpose: Creates wlan0 interface by bypassing EEPROM WiFi check"
echo "Status:"
systemctl status opensleep-wifi --no-pager || echo "opensleep-wifi service not found"
echo ""

echo "=== wpa_supplicant Service ==="
echo "Purpose: Manages WiFi authentication and connection"
echo "Status:"
systemctl status wpa_supplicant@wlan0 --no-pager || echo "wpa_supplicant service not running"
echo ""

echo "=== ssh-early Service ==="
echo "Purpose: Enables SSH access early in boot process"
echo "Status:"
systemctl status ssh-early --no-pager || echo "ssh-early service not found"
echo ""

echo "=== opensleep Service ==="
echo "Purpose: Main opensleep daemon for Pod control and MQTT communication"
echo "Status:"
systemctl status opensleep --no-pager || echo "opensleep service not found"
echo ""

echo "=========================================="
echo "Services That Should NOT Be Running"
echo "=========================================="
echo ""

echo "=== variscite-wifi Service ==="
echo "Purpose: Original Eight Sleep WiFi service (replaced by opensleep-wifi)"
echo "Expected: masked (symlinked to /dev/null)"
echo "Status:"
systemctl status variscite-wifi --no-pager || echo "variscite-wifi masked or not found"
echo ""

echo "=== capybara Service ==="
echo "Purpose: Eight Sleep's main control daemon (conflicts with opensleep)"
echo "Expected: disabled/inactive"
echo "Status:"
systemctl status capybara --no-pager || echo "capybara service status"
echo ""

# === Verify Files ===
echo "=== Check opensleep-wifi script exists ==="
if [ -f /etc/wifi/opensleep-wifi ]; then
    ls -la /etc/wifi/opensleep-wifi
    echo "✓ opensleep-wifi script found"
else
    echo "✗ opensleep-wifi script NOT found"
fi
echo ""

echo "=== Check SSH key ==="
CURRENT_USER=$(whoami)
echo "Current user: $CURRENT_USER"
SSH_KEY_FOUND=false
for key_path in /home/$CURRENT_USER/.ssh/authorized_keys /home/root/.ssh/authorized_keys /root/.ssh/authorized_keys ~/.ssh/authorized_keys; do
    if [ -f "$key_path" ]; then
        ls -la "$key_path"
        echo "✓ SSH authorized_keys found at $key_path"
        echo "Key fingerprint:"
        ssh-keygen -lf "$key_path" 2>/dev/null || echo "Could not read key fingerprint"
        SSH_KEY_FOUND=true
        break
    fi
done
if [ "$SSH_KEY_FOUND" = false ]; then
    echo "ℹ SSH authorized_keys NOT found (using password authentication)"
fi
echo ""

echo "=== Check opensleep binary ==="
# Check multiple possible locations
OPENSLEEP_BIN=""
for bin_path in /usr/local/bin/opensleep /opt/opensleep/opensleep /usr/bin/opensleep; do
    if [ -f "$bin_path" ]; then
        OPENSLEEP_BIN="$bin_path"
        break
    fi
done

if [ -n "$OPENSLEEP_BIN" ]; then
    ls -la "$OPENSLEEP_BIN"
    echo "✓ opensleep binary found at $OPENSLEEP_BIN"
    "$OPENSLEEP_BIN" --version 2>/dev/null || echo "(version info not available)"
else
    echo "✗ opensleep binary NOT found (may not have been installed with -b option)"
fi
echo ""

echo "=== Check opensleep config ==="
# Check multiple possible locations
OPENSLEEP_CONFIG=""
for config_path in /etc/opensleep/config.ron /opt/opensleep/config.ron; do
    if [ -f "$config_path" ]; then
        OPENSLEEP_CONFIG="$config_path"
        break
    fi
done

if [ -n "$OPENSLEEP_CONFIG" ]; then
    ls -la "$OPENSLEEP_CONFIG"
    echo "✓ opensleep config found at $OPENSLEEP_CONFIG"
else
    echo "✗ opensleep config NOT found"
fi
echo ""

echo "=== opensleep service logs (last 30 lines) ==="
if systemctl is-active opensleep >/dev/null 2>&1; then
    # Use journalctl if available, fall back to dmesg
    if command -v journalctl >/dev/null 2>&1; then
        # Suppress the ACL/permission warnings
        journalctl -u opensleep --no-pager -n 30 2>&1 | grep -v "Warning: some journal files were not opened" | grep -v "Failed to search journal ACL" || echo "(no recent logs or insufficient permissions)"
    else
        echo "(journalctl not available, checking dmesg...)"
        dmesg | grep -i opensleep | tail -30 || echo "(no opensleep entries in dmesg)"
    fi
else
    echo "opensleep service is not active, no logs to display"
fi
echo ""

# === Boot Order Verification ===
echo "=== Service Dependencies (opensleep-wifi) ==="
systemctl list-dependencies opensleep-wifi --no-pager | head -20
echo ""

echo "=== Service Dependencies (ssh-early) ==="
systemctl list-dependencies ssh-early --no-pager | head -20
echo ""

# === Masked Services ===
echo "=== Verify variscite-wifi is masked ==="
systemctl is-enabled variscite-wifi 2>/dev/null || echo "variscite-wifi status check"
ls -la /etc/systemd/system/variscite-wifi.service 2>/dev/null || echo "variscite-wifi service file not found in /etc/systemd/system"
echo ""

# === Summary ===
echo "========================================"
echo "Validation Summary"
echo "========================================"

# Check critical items
ERRORS=0

# Check for IP address using available tools
HAS_IP=false
if command -v ip >/dev/null 2>&1; then
    if ip addr show wlan0 2>/dev/null | grep -q "inet "; then
        HAS_IP=true
    fi
elif command -v ifconfig >/dev/null 2>&1; then
    if ifconfig wlan0 2>/dev/null | grep -q "inet "; then
        HAS_IP=true
    fi
fi

if [ "$HAS_IP" = false ]; then
    echo "✗ wlan0 does not have an IP address"
    ((ERRORS++))
else
    echo "✓ wlan0 has an IP address"
fi

if ! systemctl is-active opensleep-wifi >/dev/null 2>&1; then
    echo "✗ opensleep-wifi service is not active"
    ((ERRORS++))
else
    echo "✓ opensleep-wifi service is active"
fi

if ! systemctl is-active wpa_supplicant@wlan0 >/dev/null 2>&1; then
    echo "✗ wpa_supplicant@wlan0 service is not active"
    ((ERRORS++))
else
    echo "✓ wpa_supplicant@wlan0 service is active"
fi

if ! systemctl is-active ssh-early >/dev/null 2>&1; then
    echo "✗ ssh-early service is not active"
    ((ERRORS++))
else
    echo "✓ ssh-early service is active"
fi

# SSH key check is informational only (password auth is valid too)
SSH_KEY_EXISTS=false
for key_path in /home/root/.ssh/authorized_keys /root/.ssh/authorized_keys ~/.ssh/authorized_keys; do
    if [ -f "$key_path" ]; then
        SSH_KEY_EXISTS=true
        break
    fi
done
if [ "$SSH_KEY_EXISTS" = true ]; then
    echo "✓ SSH key-based authentication configured"
else
    echo "ℹ Using SSH password authentication"
fi

if systemctl is-active variscite-wifi >/dev/null 2>&1; then
    echo "⚠ variscite-wifi service is still active (should be masked)"
    ((ERRORS++))
else
    echo "✓ variscite-wifi service is masked/inactive"
fi

echo ""
echo "=== Eight Sleep Services Status ==="
EIGHT_SLEEP_SERVICES="dac frank capybara swupdate-progress swupdate defibrillator"
for service in $EIGHT_SLEEP_SERVICES; do
    if systemctl is-active "$service" >/dev/null 2>&1; then
        echo "⚠ $service is still active (should be disabled)"
    elif systemctl is-enabled "$service" >/dev/null 2>&1; then
        echo "⚠ $service is enabled but not running"
    else
        echo "✓ $service is disabled/inactive"
    fi
done

echo ""
if [ $ERRORS -eq 0 ]; then
    echo "✓✓✓ All critical checks passed! ✓✓✓"
    echo "Pod 3 patch deployment is successful."
else
    echo "✗✗✗ Found $ERRORS issue(s) ✗✗✗"
    echo "Review the output above for details."
fi

echo ""
echo "========================================"
