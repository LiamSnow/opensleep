# Open Sleep

FOSS firmware for Eight Sleep Pod 3.
Completely replaces all Eight Sleep services (Frank, DAC/PizzaRat, Capybara).
As this replaces the DAC, you cannot use the mobile app to control the
bed once setup.

TODO
 - [ ] OpenSensor
     - [x] Connect
     - [x] Parse messages
     - [x] Set gain, sampling rate, enable vibration, start sampling
     - [ ] Read bed temp sensors
     - [ ] Read capacitance sensors
     - [ ] Read piezo sensors
     - [ ] Set alarms
 - [ ] OpenFrozen
     - [x] Connect
     - [x] Parse messages
     - [x] Set l/r temps
     - [x] Prime
     - [ ] Turn off
     - [ ] Parse unknown values


## Eight Sleep Background

Linux SOM (`VAR-SOM-MX8M-MINI_V1.x`) running pretty minimal Yocto build.
 - Systems runs off 8GB eMMC normally
 - Micro SD card (16GB industrial SanDisk) contains 3 partitions (p1 to boot from, p3 for persistent storage)
    - If the small button is held in during boot, the SOM will boot from the SD card p1
    - It will run a script that will copy `/opt/images/Yocto/rootfs.tar.gz` onto the eMMC, then reboots from eMMC

### Services
Frank (`/opt/eight/bin/frakenfirmware`) C++ binary with simple UNIX socket commands. Controls:
 - LEDs over I2C (IS31FL3194)
    - Also controlled by other processes
 - Sensor Unit (STM32F030CCT6) over UART (`/dev/ttymxc0`), flashes `firmware-sensor.bbin`
    - 6 capacitance sensors, 1x/second
    - 2 Piezo sensors, 500x/second
    - Bed temp (microcontroller's temp, ambient temp, humidity, 6 on bed)
    - Freezer temp (ambient, hs, left/right)
    - Vibration alarms
    - Takes in a left and right ADC gain parameter (default `400`)
 - "Frozen" over UART (`/dev/ttymxc2`), flashes `firmware-frozen.bbin`
    - Takes l/r temperatures and durations
 - TODO water level? solenoid? ...?
 - Uploading Raw sensor data + logs to `raw-api-upload.8slp.net:1337`

Capybara (`/opt/eight/bin/Eight.Capybara`) .NET code. Didn't look into this much but it seems to handle initial setup via bluetooth
 - Writes `/deviceinfo`
 - Has a loopback with the sensor UART (for debugging?)
 - Restarts Frozen (seemingly a critical function??)

Device-API-Client (DAC)/PizzaRat (`/home/dac/app`) Node TypeScript code
 - CoAP for device API `device-api.8slp.net:5684`
 - Basically just a wrapper for Frank

SWUpdate gets software updates from `update-api.8slp.net:443`

### Hardware




## Features

1.  Automatically set bed temperature every night
2.  Create a custom temperature profile
3.  Set a heat and/or vibration wakeup alarm
4.  Control settings and monitor remotely via API
5.  Use in `Solo` or `Couples` mode

## Setup

To use Open Sleep you must disassemble the Eight Sleep Pod 3, modify the SD card's `rootfs.tar.gz`
to add your SSH key + root password, and reset the Pod. Then power the Pod while holding the small
button on the back, which performs a factory reset from `rootfs.tar.gz`. Now you can disable
Eight Sleep's update service and [Add Open Sleep](#adding-open-sleep-).

- **Note**: the default SSH port for Pod 3 is `8822`.
- **Disable Updates**: `systemctl disable --now swupdate-progress swupdate defibrillator`

Eventually I will add thorough tutorial for this, but for now I would recommend
[Bo Lopker's Tutorial](https://blopker.com/writing/04-zerosleep-1/#disassembly-overview)
and the [ninesleep instructions](https://github.com/bobobo1618/ninesleep?tab=readme-ov-file#instructions).

### Making your Settings

Make a new `settings.json` file. See the examples `example_couples.json` and `example_solo.json`.

```json
{
  "timezone": "America/New_York", // IANA timezone
  "away_mode": false, // disables everything temporarily
  "prime": "15:00", // daily time to prime the bed
  "led_brightness": 0, // 0-100% brightness
  // either "both" (Solo mode) or "left" and "right" (Couples mode)
  "both": {
    "temp_profile": [-10, 10, 20], // spread from "sleep" to "wake"
    "sleep": "22:00", // must be 24-hour time
    "wake": "07:30",
    "vibration": {
      "pattern": "double",
      "intensity": 50, // 0-100%
      "duration": 600, // seconds
      "offset": 300 // seconds before "wake"
    },
    "heat": {
      "temp": 100,
      "offset": 1800 // lasts from wake-offset until wake
    }
  }
}
```

### Adding Open Sleep

Build with:

```bash
cargo build --target aarch64-unknown-linux-musl --release
```

1.  (Recommended) block all internet access (except NTP) on your router
2.  Stop services `systemctl disable --now dac frank`
    - (Optional) Stop other services `swupdate-progress swupdate defibrillator eight-kernel telegraf vector`
3.  Place the binary `opensleep` and `settings.json` at `/opt/opensleep`
4.  Place the services `opensleep.service` and `frank.service` at `/lib/systemd/system`
5.  `echo "127.0.0.1 raw-api-upload.8slp.net" | sudo tee -a /etc/hosts` (to intercept raw upload data)
6.  Enable Open Sleep and Frank `systemctl enable --now opensleep frank`

## API

### Health

`GET /health` → 500 `BAD` | 200 `OK`

### State

`GET /state` → 200

```ron
{
    /// Before Frank connects this will be false
    /// and all values will be default
    valid: bool,
    /// The current temperature for each side of the bed
    cur_temp: {
        left: i16,
        right: i16,
    },
    /// The target/setpoint temperature for each side of the bed
    tar_temp: {
        left: i16,
        right: i16,
    },
    /// How long the target temperture will last
    /// for in seconds for each side of the bed
    tar_temp_time: {
        left: u16,
        right: u16,
    },
    /// Example "20600-0001-F00-0001089C"
    sensor_label: String,
    water_level: bool,
    /// Whether the bed is priming or not
    priming: bool,
    settings: {
        version: 1,
        gain_left: u16,
        gain_right: u16,
        led_brightness_perc: u8,
    },
}
```

### All Settings R/W

`GET /settings` → 500 (Error Message) | 200 (Settings)

`POST /settings` → 500 (Error Message) | 200 `OK`

### Partial Settings R/W

#### General

`GET /{setting}` -> 500 (Error Message) | 200 (Value)

`POST /{setting}` (body: Value) -> 500 (Error Message) | 200 `OK`

| `{setting}`      | Value Type | Example            |
| ---------------- | ---------- | ------------------ |
| `timezone`       | `String`   | `America/New_York` |
| `away_mode`      | `bool`     | -                  |
| `prime`          | `Time`     | `14:00`            |
| `led_brightness` | `u8`       | `100` (%)          |

#### Bed Side

If you wish to change the mode from `Solo` to and from `Couples`,
you must POST the entire settings, otherwise these commands will fail.

For `Solo` mode, use the `both` prefix. For `Couples` use `left` and `right`.

`GET /{prefix}/{setting}` -> 500 (Error Message) | 200 (Value)

`POST /{prefix}/{setting}` (body: Value) -> 500 (Error Message) | 200 `OK`

| `{setting}`    | Value Type               | Example                                                 |
| -------------- | ------------------------ | ------------------------------------------------------- |
| `temp_profile` | `Vec<i16>`               | `[-10, -5, 20]`                                         |
| `sleep`        | `Time`                   | `22:00`                                                 |
| `wake`         | `Time`                   | `9:00`                                                  |
| `vibration`    | `Option<VibrationAlarm>` | `{pattern:"rise",intensity:20,duration:360,offset:300}` |
| `heat`         | `Option<HeatAlarm>`      | `{temp:50,offset:1200}`                                 |

## Credits

This project was inspired by [ninesleep](https://github.com/bobobo1618/ninesleep).

## Footnotes

This project is not affiliated with or endorsed by Eight Sleep.

If you encounter issues with this project please make an
issue on this repository.

For anything else please contact me at [mail@liamsnow.com](mailto:mail@liamsnow.com).
