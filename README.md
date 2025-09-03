# opensleep

Open-source firmware for the Eight Sleep Pod 3.

Completely replaces all Eight Sleep services (Frank, DAC/PizzaRat, Capybara).

WARNING: The use of opensleep will prevent the mobile app from working.

## TODO
 - [ ] Fix Alarm
 - [ ] Sleep Tracking: Heartrate, HRV, Breathing Rate
 - [ ] Use Sensor's bed temperature readings to improve Frozen

## Features

1.  Custom temperature profile
2.  Vibration alarms
3.  Presence detection
4.  Control config and monitor remotely via MQTT
5.  `Solo` or `Couples` modes
6.  LED control & effects
7.  Daily priming

## Setup

To use opensleep you must disassemble the Eight Sleep Pod 3, modify the SD card's `rootfs.tar.gz`
to add your SSH key + root password, and reset the Pod. Then power the Pod while holding the small
button on the back, which performs a factory reset from `rootfs.tar.gz`. Now you can disable
Eight Sleep's update service and [Add opensleep](#adding-open-sleep-).

- **Note**: the default SSH port for Pod 3 is `8822`.
- **Disable Updates**: `systemctl disable --now swupdate-progress swupdate defibrillator`

Eventually I will add thorough tutorial for this, but for now I would recommend split-screening
[Bo Lopker's Tutorial](https://blopker.com/writing/04-zerosleep-1/#disassembly-overview)
and the [ninesleep instructions](https://github.com/bobobo1618/ninesleep?tab=readme-ov-file#instructions).

### Making your Config

Make a new `config.ron` file. See the examples `example_couples.ron` and `example_solo.ron`.

### Adding opensleep

Download the `opensleep` binary from release page.

1.  Stop services `systemctl disable --now dac frank capybara swupdate-progress swupdate defibrillator eight-kernel telegraf vector`
2.  Place the binary `opensleep` and `config.ron` at `/opt/opensleep`
3.  Place the service `opensleep.service` at `/lib/systemd/system`
4.  Enable opensleep `systemctl enable --now opensleep`

## MQTT Interface

- `opensleep/`
  - `presence/`: Person Presense Detection, All RO
    - `in_bed`: `bool`
    - `on_left`: `bool`
    - `on_right`: `bool`
  - `config/`: Published config from `config.ron`. Modifications will be saved back to `config.ron`. 
    - `timezone`: RO `string`
    - `away_mode`: RW `bool`
    - `prime`: RW `time`
    - `led/`
      - `idle`: RO `LedPattern`
      - `active`: RO `LedPattern`
      - `band`: RO `CurrentBand`
    - `mqtt/`
      - `server`: RO `string`
      - `port`: RO `u16`
      - `user`: RO `string`
    - `profile/` Profile type (`couples` or `solo`) may not be changed in runtime.
      - `type`: RO `string` (`couples` or `solo`)
      - `left/` and `right/` for couples, or `solo/` for solo
        - `sleep`: RW `time`
        - `wake`: RW `time`
        - `temperatures`: RW `Vec<celcius>`
        - `alarm/`: RW `AlarmConfig`
    - `presence/`
      - `baselines`: RW `[u16; 6]`
      - `threshold`: RW `u16`
      - `debounce_count`: RW `u8`
  - `calibrate`: WO (triggers presense calibration, do not sit on the bed during this time)
  - `sensor/` Sensor Subsystem Info, All RO
    - `mode`: `DeviceMode`
    - `hwinfo`: `HardwareInfo`
    - `piezo_ok`: `bool`
    - `vibration_enabled`: `bool`
    - `bed_temp`: `[centcel; 6]`
    - `ambient_temp`: `centcel`
    - `humidity`: `u16`
    - `mcu_temp`: `centcel`
  - `frozen/`: Frozen Subsystem Info, All RO
    - `mode`: `DeviceMode`
    - `hwinfo`: `HardwareInfo`
    - `left_temp`: `centcel` (left side water temperature)
    - `right_temp`: `centcel`
    - `heatsink_temp`: `centcel`
    - `left_target_temp`: `centcel`|`disabled` (target left side water temperature)
    - `right_target_temp`: `centcel`|`disabled`

  
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

`centcel` a u16 representing a temperature in centidegrees celcius IE `deg C * 100`

`celcius` an f32 representing a temperature in degrees celcius

`DeviceMode` one of `Unknown`, `Bootloader`, `Firmware`. `Firmware` means the device is initialized and working properly.


`HardwareInfo`: ex. `SN 000157e2 PN 20500 SKU 2 HWREV 0502 FACTORYFLAG 1 DATECODE 16070c`



## Credits

This project was inspired by [ninesleep](https://github.com/bobobo1618/ninesleep).

## Footnotes

This project is not affiliated with or endorsed by Eight Sleep.

If you encounter issues with this project please make an issue on this repository.

For anything else please contact me at [mail@liamsnow.com](mailto:mail@liamsnow.com).

See more at [liamsnow.com](https://liamsnow.com/projects/opensleep)
