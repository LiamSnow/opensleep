# MQTT Spec

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
  
## Types
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

`AlarmConfig` may be "disabled" or a comma-separated list of config, where `PATTERN,INTENSITY,DURATION,OFFSET`. For example:
 - `Double,80,600,0`
 - `Single,20,600,0`

`centidegrees_celcius` a u16 representing a temperature in centidegrees celcius IE `deg C * 100`

`celcius` an f32 representing a temperature in degrees celcius

`DeviceMode` one of `Unknown`, `Bootloader`, `Firmware`. `Firmware` means the device is initialized and working properly.


`HardwareInfo`: ex. `SN 000157e2 PN 20500 SKU 2 HWREV 0502 FACTORYFLAG 1 DATECODE 16070c`
