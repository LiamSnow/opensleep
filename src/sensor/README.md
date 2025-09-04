# Sensor

`manager.rs`: top level manager for Sensor
 - runs discovery to try and connect to Sensor in firmware (high baud) or bootloader (low baud) mode
 - maintains connection with Sensor, continously reading sensor data
 - schedules commands to be sends (enabling vibration motors, setting piezo gain, ..)

`state.rs`: state management for manager
 - takes in packets and updates it's `SensorState` + publishes changes to MQTT

`command.rs`: all Sensor cmds (`SensorCommand`) & serialization

`packet.rs`: all Sensor packets (`SensorPacket`) & deserialization

`presence.rs`: presense detection & calibration
 - takes in `CapacitanceData` from `manager.rs` and outputs state to MQTT
