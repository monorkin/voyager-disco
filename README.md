# voyager-disco

A small stand-alone CLI tool to control the RGB LEDs on [ZSA Voyager](https://www.zsa.io/voyager) keyboards on-the-fly, without Keymapp or reflashing firmware.

It talks directly to the keyboard over USB HID using ZSA's Oryx protocol.

## Installation

```sh
curl -fsSL https://raw.githubusercontent.com/monorkin/voyager-disco/main/install.sh | sh
```

## Usage

### Set a color

```sh
voyager-disco set-color ff00aa
voyager-disco set-color '#ff00aa'
```

Sets all LEDs to the given hex color. The color persists until reset or keyboard power cycle.

### Match the current OS theme

```sh
voyager-disco match-theme
```

Reads the accent color from `~/.config/omarchy/current/theme/colors.toml` and applies it to all LEDs.

### Reset to normal lighting

```sh
voyager-disco reset
```

Restores the keyboard's own lighting (whatever you configured in Oryx).

### Target specific keyboards

All commands that change LEDs accept `-d` / `--device` to target specific keyboards by serial number (comma-separated). Without it, all connected Voyagers are targeted.

```sh
voyager-disco set-color ff0000 -d ABC123
voyager-disco set-color ff0000 -d ABC123,DEF456
voyager-disco reset -d ABC123
```

Use `voyager-disco list` to find serial numbers.

### List connected keyboards

```sh
voyager-disco list
```

Prints serial number, product name, and HID path for each connected Voyager.

## Build

Requires Rust and a C compiler (for the `hidapi` native dependency).

```sh
make build
```

The binary is at `target/release/voyager-disco`.

### Linux: udev rules

You need a udev rule to access the keyboard without root. If you've used Keymapp before, you likely already have one. Otherwise, create `/etc/udev/rules.d/50-zsa.rules`:

```
ATTRS{idVendor}=="3297", TAG+="uaccess"
```

Then reload:

```sh
sudo udevadm control --reload-rules && sudo udevadm trigger
```

### Cross-platform releases

You'll need to install Cross, if you don't have it installed, and then run a `build-all`

```bash
cargo install cross
make build-all
```

### Cutting a release

To publish a release all you have to do is run `make release TAG=vX.Y.Z`.
That will do a cross-platform build and then create a GitHub release with the generated binaries attached.

## Notes

- **Keymapp must not be running** (or at least not connected to the keyboard), as it holds the raw HID interface open.
- **Brightness scaling**: The firmware scales colors by the keyboard's brightness setting. At 50% brightness, `ff0000` appears as `7f0000`.
- **No custom firmware needed**: This works with stock ZSA firmware. The Oryx protocol is built into every Voyager.
- Colors are stored in keyboard RAM. They survive the tool exiting but not a power cycle.

## How it works

The Voyager's firmware includes a QMK module called "Oryx" ([source](https://github.com/zsa/qmk_modules/blob/main/oryx/oryx.c)) that accepts commands over USB Raw HID. This is the same protocol that Keymapp uses under the hood. No custom firmware is needed — stock Voyager firmware already supports this.

### USB device identification

The Voyager exposes multiple HID interfaces (keyboard, consumer control, raw HID). The tool finds the correct one by filtering on:

| Parameter      | Value    |
|----------------|----------|
| Vendor ID      | `0x3297` |
| Product ID     | `0x1977` |
| HID Usage Page | `0xFF60` |
| HID Usage ID   | `0x61`   |
| Packet Size    | 32 bytes |

### Oryx protocol (v0x04)

All packets are exactly 32 bytes, zero-padded.

#### Command codes (host -> keyboard)

| Code   | Name                    | Description                        |
|--------|-------------------------|------------------------------------|
| `0x00` | GET_FW_VERSION          | Get firmware version               |
| `0x01` | PAIRING_INIT            | Initiate pairing (always succeeds) |
| `0x02` | PAIRING_VALIDATE        | Legacy, no-op                      |
| `0x03` | DISCONNECT              | Disconnect                         |
| `0x04` | SET_LAYER               | Switch active layer                |
| `0x05` | RGB_CONTROL             | Enable/disable RGB control mode    |
| `0x06` | SET_RGB_LED             | Set a single LED color             |
| `0x07` | SET_STATUS_LED          | Set a status LED                   |
| `0x08` | UPDATE_BRIGHTNESS       | Increase/decrease brightness       |
| `0x09` | SET_RGB_LED_ALL         | Set all LEDs to one color          |
| `0x0A` | STATUS_LED_CONTROL      | Status LED control mode            |
| `0xFE` | GET_PROTOCOL_VERSION    | Get protocol version               |

#### Event codes (keyboard -> host)

| Code   | Name                    | Description                    |
|--------|-------------------------|--------------------------------|
| `0x00` | EVT_GET_FW_VERSION      | Firmware version response      |
| `0x01` | EVT_PAIRING_INPUT       | Pairing input event            |
| `0x02` | EVT_PAIRING_KEY_INPUT   | Pairing key input              |
| `0x03` | EVT_PAIRING_FAILED      | Pairing failed                 |
| `0x04` | EVT_PAIRING_SUCCESS     | Pairing succeeded              |
| `0x05` | EVT_LAYER               | Layer change notification      |
| `0x06` | EVT_KEYDOWN             | Key pressed                    |
| `0x07` | EVT_KEYUP               | Key released                   |
| `0x08` | EVT_RGB_CONTROL         | RGB control state change       |
| `0x09` | EVT_TOGGLE_SMART_LAYER  | Smart layer toggled            |
| `0x0A` | EVT_TRIGGER_SMART_LAYER | Smart layer triggered          |
| `0x0B` | EVT_STATUS_LED_CONTROL  | Status LED control state       |
| `0xFE` | EVT_GET_PROTOCOL_VERSION| Protocol version response      |
| `0xFF` | EVT_ERROR               | Error                          |

The stop bit marker is `0xFE` in event payloads.

### Message sequence for RGB control

**Step 1: Pair** (required, but there's no real challenge — always succeeds)

```
Send:  [0x01, 0x00, ...]           # PAIRING_INIT
Recv:  [0x04, 0xFE, ...]           # EVT_PAIRING_SUCCESS
Recv:  [0x05, <layer>, 0xFE, ...]  # EVT_LAYER (current layer)
```

**Step 2: Enable RGB control mode**

```
Send:  [0x05, 0x01, 0x00, ...]     # RGB_CONTROL, enable=1
Recv:  [0x08, 0x01, ...]           # EVT_RGB_CONTROL, active=1
```

This switches the keyboard to a special `oryx_webhid_effect` RGB matrix effect that reads colors from a `webhid_leds[]` array in firmware RAM.

**Step 3a: Set a single LED** (no response)

```
Send:  [0x06, <index>, <R>, <G>, <B>, 0x00, ...]  # SET_RGB_LED
```

LED indices: 0-25 = left half, 26-51 = right half (52 total).

**Step 3b: Set ALL LEDs to one color** (no response)

```
Send:  [0x09, <R>, <G>, <B>, 0x00, ...]  # SET_RGB_LED_ALL
```

**Step 4: Restore normal lighting**

```
Send:  [0x05, 0x00, 0x00, ...]     # RGB_CONTROL, enable=0
Recv:  [0x08, 0x00, ...]           # EVT_RGB_CONTROL, active=0
```

This restores the user's configured lighting from EEPROM.

### Other commands

**Set layer:**
```
Send:  [0x04, <on/off>, <layer_num>, ...]
```
`on/off`: 0 = layer_off, 1 = layer_move.

**Set status LED:**
```
Send:  [0x07, <index (0-3)>, <on/off>, ...]
```

**Brightness:**
```
Send:  [0x08, <direction>, ...]
```
`direction`: 1 = increase, 0 = decrease.

### LED layout

52 RGB LEDs total:
- Indices 0-25: Left half (26 keys)
- Indices 26-51: Right half (26 keys)

The index mapping matches the `rgb_matrix.layout` array in the keyboard's `keyboard.json` in the ZSA QMK firmware repo.

### Firmware source references

- [oryx/oryx.c](https://github.com/zsa/qmk_modules/blob/main/oryx/oryx.c) — command dispatcher
- [oryx/oryx.h](https://github.com/zsa/qmk_modules/blob/main/oryx/oryx.h) — command/event enums
- [oryx/config.h](https://github.com/zsa/qmk_modules/blob/main/oryx/config.h) — USB usage page/ID
- [oryx/rgb_matrix_kb.inc](https://github.com/zsa/qmk_modules/blob/main/oryx/rgb_matrix_kb.inc) — webhid RGB effect
- [keyboards/zsa/voyager/keyboard.json](https://github.com/zsa/qmk_firmware/blob/firmware24/keyboards/zsa/voyager/keyboard.json) — USB IDs, LED layout

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
