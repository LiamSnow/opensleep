## Eight Sleep Background

### Hardware
Linux SOM (`VAR-SOM-MX8M-MINI_V1.3`) running pretty minimal Yocto build.
 - Systems runs off 8GB eMMC normally
 - Micro SD card (16GB industrial SanDisk) contains 3 partitions (p1 to boot from, p3 for persistent storage)
    - If the small button is held in during boot, the SOM will boot from the SD card p1
    - It will run a script that will copy `/opt/images/Yocto/rootfs.tar.gz` onto the eMMC, then reboots from eMMC

#### Subsystems

"Frozen" (STM32F030CCT6) on the main PCB
 - Manages water temperature control and priming (2 TECs, 1 solenoid, 2 pumps)
 - USART control over `/dev/ttymxc0` at 38400 baud
 - Firmware: `/opt/eight/lib/subsystem_updates/firmware-frozen.bbin`

"Sensor-board" on the bed control unit (connected over USB)
 - Manages vibration alarm motors, 6 capacitance sensors (2Hz), 8 bed temperature sensors, ambient sensor (temp + humidity), ADC connected to 2x piezo sensors (1000kHz), heater?
 - USART control over `/dev/ttymxc2` at 38400 baud in bootloader mode and 115200 in firmware mode
 - Firmware: `/opt/eight/lib/subsystem_updates/firmware-sensor.bbin`


### Services
#### Frank (`/opt/eight/bin/frakenfirmware`)
C++ with simple UNIX socket commands. Controls:
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

#### Device-API-Client (DAC)/PizzaRat (`/home/dac/app`)
Node TypeScript
 - CoAP for device API `device-api.8slp.net:5684`
 - Basically just a wrapper for Frank

#### SWUpdate
Gets software updates from `update-api.8slp.net:443`

#### Capybara (`/opt/eight/bin/Eight.Capybara`)
.NET
 - Handles initial setup via Bluetooth
 - Writes `/deviceinfo`
 - Has a loopback with the sensor UART (for debugging?)
 - Enables Subsystems (Frozen + Sensor) over `/dev/i2c-1` `0x20` which is a PCAL6416
    - Restarts Frozen when `/persistent/frozen.heartbeat` is old



[.dts changes](https://github.com/varigit/linux-imx/commit/593a62b5dcd311f4e469fa2dad91cf1b8865c6fb?diff=unified)



