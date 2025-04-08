# belabot

A chat bot alternative to control [belaUI](https://github.com/BELABOX/belaUI) in combination with [BELABOX Cloud](https://cloud.belabox.net)

## How do i run this?

Just download the latest binary from [releases](https://github.com/715209/belabot/releases) and execute it.

## Config

Example of the config that will be automatically generated upon running the binary and saved as `config.json`.

```JSON
{
    "belabox": {
        "remote_key": "your BELABOX Cloud key",
        "custom_interface_name": {
            "eth0": "ETH",
            "usb0": "USB"
        },
        "monitor": {
            "modems": true,
            "notifications": true,
            "ups": true,
            "network": false,
            "ups_plugged_in": 5.1,
            "notification_timeout": 30,
            "network_timeout": 30
        }
    },
    "twitch": {
        "bot_username": "715209",
        "bot_oauth": "oauth:YOUR_OAUTH",
        "channel": "715209",
        "admins": ["b3ck"]
    },
    "commands": {
        "Sensor": {
            "command": "!bbsensor",
            "permission": "Public"
        },
        "Stats": {
            "command": "!bbs",
            "permission": "Public"
        },
        "Poweroff": {
            "command": "!bbpo",
            "permission": "Broadcaster"
        },
        "Restart": {
            "command": "!bbrs",
            "permission": "Broadcaster"
        },
        "Bitrate": {
            "command": "!bbb",
            "permission": "Broadcaster"
        },
        "Start": {
            "command": "!bbstart",
            "permission": "Broadcaster"
        },
        "Stop": {
            "command": "!bbstop",
            "permission": "Broadcaster"
        },
        "Network": {
            "command": "!bbt",
            "permission": "Broadcaster"
        }
    }
}
```

### BELABOX

```JSON
"belabox": {
    "remote_key": "key",
    "custom_interface_name": {
        "eth0": "Something",
        "usb0": "Else"
    },
    "monitor": {
        "modems": true,
        "notifications": true,
        "ups": true,
        "ups_plugged_in": 5.1,
        "notification_timeout": 30
    }
}
```

- `remote_key`: Your [BELABOX Cloud](https://cloud.belabox.net) key
- `custom_interface_name`: Change the name of the interface
- `monitor`: Enable monitoring for automatic chat messages

### Twitch

```JSON
"twitch": {
    "bot_username": "715209",
    "bot_oauth": "oauth:YOUR_OAUTH",
    "channel": "715209",
    "admins": ["b3ck", "another"]
},
```

- `bot_username`: The username of your bot account
- `bot_oauth`: The oauth of your bot ([generate an oauth](https://twitchapps.com/tmi)).
- `channel`: The channel the bot should join
- `admins`: Comma sepperated list of twitch usernames, these will have permissions to run all commands

### Commands

```JSON
"commands": {
    "Sensor": {
        "command": "!bbsensor",
        "permission": "Public"
    },
    ...
}
```

- `command`: The chat command
- `permission`: The permission for this command, valid options are: `Public`, `Vip`, `Moderator`, `Broadcaster`.

## Chat Commands

After running the executable successfully you can use the following commands in your chat:

| Name       | Default command | Description                                           |
| ---------- | --------------- | ----------------------------------------------------- |
| Bitrate    | !bbb (bitrate)  | Sets the max bitrate                                  |
| Network    | !bbt (name)     | Toggles an interface to disable or enable             |
| Poweroff   | !bbpo           | Poweroff the jetson nano                              |
| Restart    | !bbrestart      | Restarts the jetson nano                              |
| Sensor     | !bbsensor       | Shows the current sensor information                  |
| Stats      | !bbs            | Shows the current connected modems status and bitrate |
| Modems     | !bbm            | Shows the current connected modems status and bitrate |
| Start      | !bbstart        | Starts the stream                                     |
| Stop       | !bbstop         | Stops the stream                                      |
| Latency    | !bbl (latency)  | Changes the SRT latency in ms                         |
| AudioDelay | !bbd (delay)    | Changes the audio delay in ms                         |
| AudioSrc   | !bba (source)   | Changes the audio source                              |
| Pipeline   | !bbp (pipeline) | Changes the pipeline                                  |

## Disclaimer

This is a third party tool, please do not ask for help on the BELABOX discord server. Instead, join the [NOALBS Community Server](https://discord.gg/efWu5HWM2u) for all your questions.
