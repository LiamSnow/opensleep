# SSH Setup

| Pod | Setup |
| --- | ----- |
| Pod 1 | ❌ not possible |
| Pod 2 | ❌ not possible |
| Pod 3 (with sd card) | see [below](#pod-3-with-sd-card) |
| Pod 3 (no sd card) | see [free-sleep tutorial](https://github.com/throwaway31265/free-sleep/blob/main/INSTALLATION.md) |
| Pod 4 | see [free-sleep tutorial](https://github.com/throwaway31265/free-sleep/blob/main/INSTALLATION.md) |
| Pod 5 | see [free-sleep tutorial](https://github.com/throwaway31265/free-sleep/blob/main/INSTALLATION.md) |

WARNING: opensleep has only been tested with Pod 3. Pod 4 and 5 specific features are NOT implemented.
If you would like to help add Pod 4 & 5 support please contact me!

## Pod 3 with SD Card

Eventually I will add thorough tutorial for this, but for now I would recommend cross-referencing:
 - [Bo Lopker's Tutorial](https://blopker.com/writing/04-zerosleep-1/#disassembly-overview)
 - [ninesleep instructions](https://github.com/bobobo1618/ninesleep?tab=readme-ov-file#instructions)

Basically this involve:
 1. Partially disassembling the Pod
 2. Removing the SD card
 3. Modifying the `rootfs.tar.gz` file on the SD card, adding your SSH keys, WiFi network, and own password
 4. Reinserting the SD card
 5. Powering the Pod up with the small button pressed in (factory resetting the Pod to your new `rootfs.tar.gz` file)

Notes:
- Default SSH port is `8822`
- Updates will reset your system, disable the updater with: `systemctl disable --now swupdate-progress swupdate defibrillator`
