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

### Diagrams

![Diagram](diagrams/main.svg)

## Setup
Before being able to setup and programs to run on Pod's SOM, you are going to need SSH access.
This is NOT an easy mod to do and requires quite a bit of technical know how.
For some Pods this requires specialized tooling.

See [SETUP.md](SETUP.md)

## MQTT Spec
Please see [MQTT.md](MQTT.md)

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
