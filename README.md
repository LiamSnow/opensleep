# opensleep

Open-source firmware for the Eight Sleep Pod 3.

Completely replaces all Eight Sleep programs running on the SOM (Frank/frakenfirmware, DAC/PizzaRat, Capybara).

## Background
For some quick background, projects like [ninesleep](https://github.com/bobobo1618/ninesleep),
[freesleep](https://github.com/throwaway31265/free-sleep), and my original version of opensleep
all work by pretending to be the DAC (the interface to the Eight Sleep servers) by communicating
with Frank over a Unix socket.

Frank then manages communication to two "subsystems" called "Sensor" (the STM32 on the sensor unit communicated by over USART via the USB cable) and "Frozen" (an onboard STM32 for managing water pumps, TECs, solenoids, etc.). Capybara's purpose isn't entirely clear to me but its seems to handle initial Bluetooth setup along with I2C control of the LED controller and an IO expander for resetting and enabling Frozen.

While communicating with Frank works totally fine, it limits your ability to get real-time sensor data. I really really wanted to create Home Assistant automations for when I get in and out of bed so I decided to go down this really annoying but also interesting journey.

I will expand it more eventually, but I have some more information at [BACKGROUND.md](BACKGROUND.md).

## Disclaimer

This project is purely intended educational and research purposes. It is for personal, non-commercial use only. It is not affiliated with, endorsed by, or sponsored by Eight Sleep. The Eight Sleep name and Pod are trademarks of Eight Sleep, Inc.

The use of opensleep will prevent the mobile app from working and _may_, but most likely will not, permanently alter or damage your device. Use at your own risk.

## Features

1.  **MQTT** interface for remotely updating config and monitoring state
2.  Confugration via **Ron** file
3.  Presence detection
4.  Custom temperature profile with as many points as you want. It will spready out this profile between `sleep` and `wake` time.
5.  Vibration alarms relative to `wake` time (offsets and vibration settings can be configured)
6.  `Solo` or `Couples` modes
7.  LED control & cool effects
8.  Daily priming

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

See more at [liamsnow.com](https://liamsnow.com/projects/opensleep)
