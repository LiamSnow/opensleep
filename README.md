# opensleep

Open-source Rust firmware for the Eight Sleep Pod 3 that completely replaces all of Eight Sleep's programs.

With opensleep you can use your Pod 3 with complete privacy and make cool Home Assistant automations for
when you get in and out of bed. Personally I have it set up to read my daily calendar when I get out of
bed in the morning and remind to go to bed when its late. 

**TL;DR** Other projects like [ninesleep](https://github.com/bobobo1618/ninesleep) and
[freesleep](https://github.com/throwaway31265/free-sleep) replace 1/3 of Eight Sleep's
programs, opensleep replaces them all.

## Disclaimer
This project is purely intended educational and research purposes. It is for personal, non-commercial use only.
It is not affiliated with, endorsed by, or sponsored by Eight Sleep.
The Eight Sleep name and Pod are trademarks of Eight Sleep, Inc.

The use of opensleep will prevent the mobile app from working and _may_, but most likely will not,
permanently alter or damage your device. Use at your own risk.

## Features

1.  **MQTT** interface for remotely updating config and monitoring state
2.  Confugration via **[Ron](https://github.com/ron-rs/ron)** file
3.  Presence detection
4.  Custom temperature profile with as many points as you want. It will spready out this profile between `sleep` and `wake` time.
5.  Vibration alarms relative to `wake` time (offsets and vibration settings can be configured)
6.  `Solo` or `Couples` modes
7.  LED control & cool effects
8.  Daily priming

## Background
Explaining this projects requires quite a bit of background, so I would highly recommend
reading this section.

### Nomenclature
 - **Eight Sleep**: a temperature controlled mattress cover system with sleep tracking
   - "system" refers to the fact that its both the matress cover (+ sensors in it) and a physical unit which controls temperature and has a computer onboard
   - you can control settings of the bed and view sleep tracking data in the mobile app
   - all sleep tracking is streamed to the cloud (Eight Sleep knows when your in bed and a lot about your sleep.. eek)
 - **SOM**: refers to the small computer inside the Eight Sleep ([Varisite System-On-Module](https://www.variscite.com/system-on-module-som/i-mx-8/i-mx-8m-mini/var-som-mx8m-mini/))
   - this is the master controller of the whole system
 - **Sensor Subsystem**: an [STM32 microcontroller](https://en.wikipedia.org/wiki/STM32) on the sensor unit (control box inside the matress cover)
   - collects data from all sensors (8 temperature, 6 capacitance, 2 piezoelectric - all lying around check height)
   - controls vibration motors/alarm
 - **Frozen Subsystem**: an [STM32 microcontroller](https://en.wikipedia.org/wiki/STM32) on the main control board (where the SOM is located)
   - manages 2x thermoelectric coolers for heating and cooling water
   - manages 2x pumps to move water through the system
   - manages priming components (solenoid attached to water tank, water level sensor, reed switch)
 - **DAC** (Device API Client/PizzaRat): takes input from Eight Sleep servers to control the bed, sends commands to Frank
 - **Frank** (frankenfirmware):
   - controls Sensor and Frozen subsystems
   - takes commands from DAC to control the bed (IE DAC says set an alarm for tomorrow morning, it schedule a command to be send to Sensor at that time)
   - collects raw sensor data from the Sensor subsystem, batches it into a file, and uploads it to the Eight Sleep servers
 - **Capybara**: manages LEDs, initial Bluetooth setup..

See more about the Pod 3's technical details in [BACKGROUND.md](BACKGROUND.md).

### Existing Work
 - [ninesleep](https://github.com/bobobo1618/ninesleep),
 - [freesleep](https://github.com/throwaway31265/free-sleep)
 - opensleep v1 

All work by pretending to be the DAC and sending commands to Frank.
This achieves full functionality of the bed BUT by keeping Frank
you cannot get real-time sensor data (you can only see the batch files).

### This Project
Completely replaces Frank, DAC, and Capybara - communicating directly with Sensor and Frozen. 

## Setup
### SSH
Before being able to setup and programs to run on Pod's SOM, you are going to need SSH access.
This is NOT an easy mod to do and requires quite a bit of technical know how.

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

### Making your Config

Make a new `config.ron` file. See the examples `example_couples.ron` and `example_solo.ron`.

### Adding opensleep

Download the `opensleep` binary from release page.

1.  Disable Eight Sleep services `systemctl disable --now dac frank capybara`
2.  SCP over the `opensleep` binary, your `config.ron`, and systemd service `opensleep.service`
3.  Move the binary and config to `/opt/opensleep`
4.  Move the service to `opensleep.service` at `/lib/systemd/system`
5.  Enable opensleep `systemctl enable --now opensleep`

Now you're all setup!! You can check logs to make sure everythings working correctly using `journalctl -u opensleep`.

## MQTT Interface

- `opensleep/`

  - `availability`: `string` ("online")

  - `device/`
    - `name`: `string` ("opensleep")
    - `version`: `string`
    - `label`: `string` (ex. "20500-0000-F00-00001234")

  - `state/`
    - `presence/`: Person Presense Detection
      - `any`: `bool`
      - `left`: `bool`
      - `right`: `bool`

    - `sensor/` Sensor Subsystem Info
      - `mode`: `DeviceMode`
      - `hwinfo`: `HardwareInfo`
      - `piezo_ok`: `bool`
      - `vibration_enabled`: `bool`
      - `bed_temp`: `[centidegrees_celcius; 6]`
      - `ambient_temp`: `centidegrees_celcius`
      - `humidity`: `u16`
      - `mcu_temp`: `centidegrees_celcius`

    - `frozen/`: Frozen Subsystem Info
      - `mode`: `DeviceMode`
      - `hwinfo`: `HardwareInfo`
      - `left_temp`: `centidegrees_celcius` (left side water temperature)
      - `right_temp`: `centidegrees_celcius`
      - `heatsink_temp`: `centidegrees_celcius`
      - `left_target_temp`: `centidegrees_celcius`|`disabled` (target left side water temperature)
      - `right_target_temp`: `centidegrees_celcius`|`disabled`

    - `config/`: Published config from `config.ron`. Modifications will be saved back to `config.ron`. 
      - `timezone`: `string`
      - `away_mode`: `bool`
      - `prime`: `time`
      - `led/`
        - `idle`: `LedPattern`
        - `active`: `LedPattern`
        - `band`: `CurrentBand`
      - `profile/`
        - `type`: `string` ("couples" or "solo")
        - `left/`, `right/` (solo mode only publishes to `left/`)
          - `sleep`: `time`
          - `wake`: `time`
          - `temperatures`: `Vec<celcius>`
          - `alarm/`: `AlarmConfig`
      - `presence/`
        - `baselines`: `[u16; 6]`
        - `threshold`: `u16`
        - `debounce_count`: `u8`

  - `actions/` NOTE any changes to config here will be saved back to the `config.ron` file.
    - `calibrate`: triggers presence calibration, do not sit on the bed during this time
    - `set_away_mode` (`bool`): sets away mode config
    - `set_prime` (`time`): sets time to prime
    - `set_profile` (`TARGET.FIELD=VALUE`)
      - `TARGET` must be `left` or `right` for couples mode or `both` for solo
      - `FIELD` is one of `sleep`, `wake`, `temperatures`, `alarm`
      - Ex: `left.sleep=20:30`
    - `set_presence_config` (`FIELD=VALUE`)
      - `FIELD` must be one of `baselines`, `threshold`, `debounce_count`
      - Ex: `threshold=50`

  - `result/`
    - `action`: `string` (ex "set_away_mode")
    - `status`: `string` ("success" or "error")
    - `message`: `string`
  
### Types
`time` is a zero-padding 24-hour time string. For example:
 - `12:00`, `06:00` valid
 - `6:00`, `5:00 PM`, `5:00pm` invalid

`Vec<T>` is a comma separated list. For example: `111,146,160,185,192,209`

`[T; N]` is a fixed-size comma separated list.

`LedPattern` is a string representation of the Rust enum:
 - `Fixed( 0, 255, 0, )`
 - `SlowBreath( 255, 0, 0, )`
 - `FastBreath( 255, 0, 0, )`
 - See code for more patterns

`CurrentBand`:
 - `One`: 0mA\~10mA, Imax=10mA
 - `Two`: 0mA\~20mA, Imax=20mA
 - `Three`: 0mA\~30mA, Imax=30mA
 - `Four`: 0mA\~40mA, Imax=40mA

`AlarmConfig` may be `None` or a comma-separated list of config, where `PATTERN,INTENSITY,DURATION,OFFSET`. For example:
 - `Double,80,600,0`
 - `Single,20,600,0`

`centidegrees_celcius` a u16 representing a temperature in centidegrees celcius IE `deg C * 100`

`celcius` an f32 representing a temperature in degrees celcius

`DeviceMode` one of `Unknown`, `Bootloader`, `Firmware`. `Firmware` means the device is initialized and working properly.


`HardwareInfo`: ex. `SN 000157e2 PN 20500 SKU 2 HWREV 0502 FACTORYFLAG 1 DATECODE 16070c`


## Roadmap
 - [ ] Use Sensor's bed temperature readings to improve Frozen
 - [ ] Sleep Tracking: Heartrate, HRV, Breathing Rate
 - [ ] More advanced LED patterns using direct current level control

## Footnotes
If you encounter issues with this project please make an issue on this repository. For anything else please contact me at [mail@liamsnow.com](mailto:mail@liamsnow.com).

Normally opensleep should run around 0.3-0.6% CPU usage and 0.1% RAM usage.
If you see numbers above this range, please reach out to me! 

If you have a Pod other than Pod 3 and would be interesting in getting opensleep working on it, please reach out to me!!

See more at [liamsnow.com](https://liamsnow.com/projects/opensleep)
