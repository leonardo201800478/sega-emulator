# Native egui application shell

`sega-app` is a native Rust/egui application shell. At the current migration checkpoint it renders only the initial shell and does not yet create an SMS machine, load a ROM or connect the SMS worker.

The next integration increment must connect this shell to `rust/runtime/sms-runtime` through coarse-grained commands and native host services. The UI must not advance hardware time or own machine state.

The shell is not yet an executable emulator frontend. Do not claim gameplay, audio, input or video integration until the corresponding Windows and headless tests pass.
