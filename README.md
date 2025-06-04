# Open Sleep

Control the Eight Sleep Pod 3 locally and automatically!

Open Sleep communicates with the bed's firmware (`frakenfirmware`) by pretending
to be the DAC. This means that, once setup, you **CANNOT** use the Eight Sleep
mobile app to control the bed.

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
- **Disable Updates**: `systemctl disable --now swupdate-progress swupdate defibrillator eight-kernel telegraf vector`

Eventually I will add thorough tutorial for this, but for now I would recommend
[Bo Lopker's Tutorial](https://blopker.com/writing/04-zerosleep-1/#disassembly-overview)
and the [ninesleep instructions](https://github.com/bobobo1618/ninesleep?tab=readme-ov-file#instructions).

### Adding Open Sleep

Build with:

```bash
cargo build --target aarch64-unknown-linux-musl --release
```

1.  Create a `settings.json` (see examples `example_couples.json` and `example_solo.json`)
2.  `scp` the binary, `opensleep.service`, and `settings.json` to the Pod
3.  `ssh` in, sign in as root
4.  Move the binary and JSON to `/opt/opensleep`
5.  Move the service file to `/etc/systemd/system`
6.  Stop the DAC `systemctl disable --now dac`
7.  Enable the Open Sleep `systemctl enable --now opensleep`

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
