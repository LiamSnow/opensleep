# Home Assistant opensleep Setup

## configuration.yaml

```yaml
...
mqtt:
  - text:
      name: "opensleep sleep time"
      command_topic: "opensleep/actions/set_profile"
      command_template: >
        {% if value %}
            both.sleep={{ value }}
        {% endif %}
      retain: false
      pattern: ^([01][0-9]|2[0-3]):[0-5][0-9](?::[0-5][0-9])?$
      state_topic: "opensleep/state/config/profile/left/sleep"
  - text:
      name: "opensleep wake time"
      command_topic: "opensleep/actions/set_profile"
      command_template: >
        {% if value %}
            both.wake={{ value }}
        {% endif %}
      retain: false
      pattern: ^([01][0-9]|2[0-3]):[0-5][0-9](?::[0-5][0-9])?$
      state_topic: "opensleep/state/config/profile/left/wake"
  - text:
      name: "opensleep alarm"
      command_topic: "opensleep/actions/set_profile"
      command_template: >
        {% if value %}
            both.alarm={{ value }}
        {% endif %}
      retain: false
      state_topic: "opensleep/state/config/profile/left/alarm"
  - text:
      name: "opensleep target temp"
      command_topic: "opensleep/state/frozen/left_target_temp" # RO
      retain: false
      state_topic: "opensleep/state/frozen/left_target_temp"
  - text:
      name: "opensleep current temp"
      command_topic: "opensleep/state/frozen/left_temp" # RO
      retain: false
      state_topic: "opensleep/state/frozen/left_temp"
  - switch:
      name: "opensleep away mode"
      command_topic: "opensleep/set_away_mode"
      state_topic: "opensleep/state/config/away_mode"
      retain: false
  - text:
      name: "opensleep temperatures"
      command_topic: "opensleep/actions/set_profile"
      command_template: >
        {% if value %}
            both.temperatures={{ value }}
        {% endif %}
      retain: false
      state_topic: "opensleep/state/config/profile/left/temperatures"
```

## Dashboard

```yaml
views:
  - title: # ...
    # ...
    cards:
      - type: entities
         entities:
           - entity: text.opensleep_sleep_time
             name: sleep time
             icon: mdi:bed-clock
           - entity: text.opensleep_wake_time
             icon: mdi:sun-clock
             name: wake time
           - entity: text.opensleep_temperatures
             name: temperatures
             icon: mdi:thermometer
           - entity: text.opensleep_alarm
             icon: mdi:alarm
             name: alarm
```

## Automations

```yaml
alias: "opensleep leave bed"
description: ""
triggers:
  - trigger: mqtt
    topic: opensleep/state/presence/any
    payload: "false"
conditions: []
actions:
  - action: light.turn_on
    metadata: {}
    data:
      brightness_pct: 100
    target:
      area_id: 107d
mode: single
```
