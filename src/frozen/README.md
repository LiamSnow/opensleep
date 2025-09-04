# Frozen

`manager.rs`: top level manager for Frozen
 - maintains connection with Frozen
 - schedules commands (priming, setting temps, ..)
   - sometimes Frozen goes to sleep when its not doing anything, so this also wakes it up before sending commands
 - changes LED based on profile state

`state.rs`: state management for manager
 - takes in packets and updates it's `FrozenState` + publishes changes to MQTT

`command.rs`: all Frozen cmds (`FrozenCommand`) & serialization

`packet.rs`: all Frozen packets (`FrozenPacket`) & deserialization

`profile.rs`: calculates temperature profile
 - takes current Time and returns target temperatures
