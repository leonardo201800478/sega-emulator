#[cfg(not(windows))]
fn main() {
    eprintln!("This example requires Windows x64 and Direct3D 12.");
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::thread;
    use std::time::Duration;

    use sega_video_runtime::d3d12::D3d12Session;
    use sega_video_runtime::{PixelFormat, VideoFrame};

    const FRAME_WIDTH: u32 = 256;
    const FRAME_HEIGHT: u32 = 192;

    let mut pixels = Vec::with_capacity(
        FRAME_WIDTH as usize * FRAME_HEIGHT as usize * PixelFormat::Rgb565Le.bytes_per_pixel(),
    );
    for y in 0..FRAME_HEIGHT {
        for x in 0..FRAME_WIDTH {
            let red = (x * 31 / FRAME_WIDTH) as u16;
            let green = (y * 63 / FRAME_HEIGHT) as u16;
            let blue = ((x + y) * 31 / (FRAME_WIDTH + FRAME_HEIGHT)) as u16;
            let pixel = (red << 11) | (green << 5) | blue;
            pixels.extend_from_slice(&pixel.to_le_bytes());
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

    let session = D3d12Session::start(
        FRAME_WIDTH,
        FRAME_HEIGHT,
        FRAME_WIDTH * 3,
        FRAME_HEIGHT * 3,
        2,
    )?;
    session.submit(frame)?;

    println!("D3D12 session is running; close the native window to exit.");
    while session.is_running() {
        thread::sleep(Duration::from_millis(10));
    }

    println!(
        "Last presented sequence: {}; canceled pending frames: {}",
        session.last_presented_sequence(),
        session.cancelled_pending_frames()
    );
    session.shutdown()?;
    Ok(())
}
