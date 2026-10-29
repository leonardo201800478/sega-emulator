# Qt SMS input configuration and visual guide

The controller configuration dialog now includes a **Master System / Light Phaser**
page. It reuses the existing SG-1000 keyboard/gamepad mapping tabs rather than
introducing a second, conflicting controller mapping system.

## Visual reference

- The standard SMS controller has a directional pad and two action buttons,
  labelled **1** and **2**. The same layout is shown for players 1 and 2.
- The SG-1000-specific **SW/Hold (NMI)** action is not a third SMS controller
  button. Existing mappings remain in the player tabs.
- The Light Phaser panel illustrates the gun, target/crosshair and trigger. It
  offers preferences for mouse aiming, left-click trigger, optional right-stick
  aiming, vertical inversion, crosshair display and sensitivity (25–300%).

## Persisted settings

The page stores `sms_light_phaser_enabled`, `sms_light_phaser_mouse_aim`,
`sms_light_phaser_mouse_trigger`, `sms_light_phaser_right_stick_aim`,
`sms_light_phaser_invert_y`, `sms_light_phaser_crosshair` and
`sms_light_phaser_sensitivity` in the existing Qt settings store.

## Integration boundary

This is configuration and user guidance, not a claim of working SMS gameplay.
The current SMS core accepts host-independent Light Phaser aim/trigger state, but
the Qt-to-Rust SMS input adapter is not integrated yet. The right analog axis is
also not yet forwarded into the SMS core. Accurate hit detection requires future
VDP/raster light sensing; mouse coordinates alone must not synthesize a light
pulse. The settings page explicitly communicates this limitation.
