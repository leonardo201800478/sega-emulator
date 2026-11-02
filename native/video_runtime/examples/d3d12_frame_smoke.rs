#[cfg(not(windows))]
fn main() {
    eprintln!("This example requires Windows x64 and Direct3D 12.");
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::time::Duration;

    use sega_video_runtime::d3d12::D3d12Host;
    use sega_video_runtime::{FrameMailbox, PixelFormat, VideoFrame};

    const FRAME_WIDTH: u32 = 256;
    const FRAME_HEIGHT: u32 = 192;

    let palette: [[u8; 3]; 8] = [
        [235, 48, 48],
        [235, 128, 35],
        [220, 220, 45],
        [45, 190, 70],
        [40, 185, 190],
        [45, 90, 225],
        [150, 65, 210],
        [220, 220, 220],
    ];

    let mut pixels = Vec::with_capacity(
        FRAME_WIDTH as usize * FRAME_HEIGHT as usize * PixelFormat::Rgb565Le.bytes_per_pixel(),
    );
    for y in 0..FRAME_HEIGHT {
        for x in 0..FRAME_WIDTH {
            let bar = (x * palette.len() as u32 / FRAME_WIDTH) as usize;
            let [red, green, blue] = palette[bar];
            let shade = if (y / 16) % 2 == 0 { 255 } else { 220 };
            let red = u16::from(red) * shade / 255;
            let green = u16::from(green) * shade / 255;
            let blue = u16::from(blue) * shade / 255;
            let rgb565 = ((red >> 3) << 11) | ((green >> 2) << 5) | (blue >> 3);
            pixels.extend_from_slice(&rgb565.to_le_bytes());
        }
    }

    let frame = VideoFrame::new(
        1,
        FRAME_WIDTH,
        FRAME_HEIGHT,
        PixelFormat::Rgb565Le,
        FRAME_WIDTH as usize * 2,
        pixels,
    )?;
    let mailbox = FrameMailbox::new();
    mailbox.publish(frame)?;

    let mut host = D3d12Host::initialize_with_window_size(
        FRAME_WIDTH,
        FRAME_HEIGHT,
        FRAME_WIDTH * 3,
        FRAME_HEIGHT * 3,
    )?;

    let presented = host.present_latest_from_mailbox(&mailbox)?;
    println!("Presented synthetic VideoFrame sequence: {presented:?}");

    while host.pump_messages() {
        std::thread::sleep(Duration::from_millis(10));
    }

    Ok(())
}
