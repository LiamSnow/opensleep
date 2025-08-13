# Open Sleep

Free open-source firmware for Eight Sleep Pod 3.

Completely replaces all Eight Sleep services (Frank, DAC/PizzaRat, Capybara).

The use of open source will prevent the mobile app from working.

## TODO
 - [ ] Profile
    - [ ] Rework
    - [ ] Alarm
    - [ ] Prime
    - [ ] LED
 - [ ] Rework Presence Detector
 - [ ] Leverage Sensor temperature readings to improve Frozen
 - [ ] Rework MQTT
 - [ ] Sleep Tracking: Heartrate, HRV, Breathing Rate

## Features

1.  Custom temperature profile
2.  Set a heat and/or vibration wakeup alarm
3.  Presence detection
4.  Control config and monitor remotely via MQTT
5.  `Solo` or `Couples` modes
6.  LED control & effects

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

### Making your Config

Make a new `config.ron` file. See the examples `example_couples.ron` and `example_solo.ron`.

### Adding Open Sleep

Download the `opensleep` binary from release page.

1.  Stop services `systemctl disable --now dac frank capybara swupdate-progress swupdate defibrillator eight-kernel telegraf vector`
2.  Place the binary `opensleep` and `config.ron` at `/opt/opensleep`
3.  Place the service `opensleep.service` at `/lib/systemd/system`
4.  Enable Open Sleep `systemctl enable --now opensleep`

## MQTT Interface




## Credits

This project was inspired by [ninesleep](https://github.com/bobobo1618/ninesleep).

## Footnotes

This project is not affiliated with or endorsed by Eight Sleep.

If you encounter issues with this project please make an issue on this repository.

For anything else please contact me at [mail@liamsnow.com](mailto:mail@liamsnow.com).

See more at [liamsnow.com](https://liamsnow.com/projects/opensleep)
