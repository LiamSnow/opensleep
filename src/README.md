# Structure

`main.rs`: program entry

`reset.rs`: handles resetting and enabling subsystems via the PCAL6416A I2C GPIO Expander

`mqtt.rs`:
 - MQTT event loop
 - actions & top level publishing (`device/`, `result/`, `availability`)
 - initialization of MQTT `AsyncClient`

`common/`: common serial and protocol handling for both Sensor and Frozen (checksum, codec, shared packets, ..)

`config/`: config model & MQTT publishing

`led/`: controller & model for the IS31FL3194 I2C LED controller

`sensor/`: communication with Sensor subsystem & presence detection

`frozen/`: communication with Frozen subsystem & temperature profile
