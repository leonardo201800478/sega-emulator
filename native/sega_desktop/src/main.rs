#![deny(unsafe_code)]

#[cfg(windows)]
mod windows_poc {
    use std::{
        cell::{Cell, RefCell},
        error::Error,
        fs,
        path::PathBuf,
        rc::Rc,
        time::Duration,
    };

    use sega_sms_runtime::SmsTemporalWorker;
    use sega_video_runtime::{FrameMailbox, PixelFormat, VideoFrame};
    use slint::{
        GraphicsAPI, RenderingState, Timer, TimerMode,
        wgpu_30::{WGPUConfiguration, WGPUSettings, wgpu},
    };

    const SMS_WIDTH: u32 = 256;
    const SMS_HEIGHT: u32 = 192;
    const PRESENT_INTERVAL: Duration = Duration::from_millis(16);

    slint::slint! {
        import { Button } from "std-widgets.slint";

        export component MainWindow inherits Window {
            title: "Sega Emulator — R2 SMS / Slint / WGPU 30 / Vulkan";
            preferred-width: 960px;
            preferred-height: 720px;
            background: #101820;

            in-out property <image> gpu-texture;
            in-out property <string> gpu-status;
            in-out property <string> frame-status;
            in-out property <string> lifecycle-status;
            in-out property <string> rom-status;

            callback start-requested();
            callback pause-requested();

            VerticalLayout {
                padding: 24px;
                spacing: 16px;

                Text {
                    text: "Sega Emulator — R2";
                    color: #f2f5f7;
                    font-size: 26px;
                }

                Text {
                    text: root.rom-status;
                    color: #b9c5ce;
                    font-size: 15px;
                }

                HorizontalLayout {
                    spacing: 8px;
                    Button {
                        text: "Iniciar";
                        clicked => { root.start-requested(); }
                    }
                    Button {
                        text: "Pausar";
                        clicked => { root.pause-requested(); }
                    }
                }

                Image {
                    source: root.gpu-texture;
                    preferred-width: 768px;
                    preferred-height: 576px;
                    min-width: 256px;
                    min-height: 192px;
                    horizontal-stretch: 1;
                    vertical-stretch: 1;
                    image-fit: contain;
                }

                Text {
                    text: root.gpu-status;
                    color: #8bd5ca;
                    font-size: 14px;
                }

                Text {
                    text: root.frame-status;
                    color: #b9c5ce;
                    font-size: 13px;
                }

                Text {
                    text: root.lifecycle-status;
                    color: #b9c5ce;
                    font-size: 12px;
                }
            }
        }
    }

    /// Inicializa o renderer Slint sobre WGPU 30, restringindo a instância ao backend Vulkan.
    fn select_vulkan_backend() -> Result<(), Box<dyn Error>> {
        let mut settings = WGPUSettings::default();
        settings.backends = wgpu::Backends::VULKAN;
        settings.power_preference = wgpu::PowerPreference::HighPerformance;

        slint::BackendSelector::new()
            .backend_name("winit".into())
            .renderer_name("femtovg-wgpu".into())
            .require_wgpu_30(WGPUConfiguration::Automatic(settings))
            .select()?;

        Ok(())
    }

    /// Retorna a extensão 2D da textura SMS em resolução nativa.
    fn texture_extent() -> wgpu::Extent3d {
        wgpu::Extent3d {
            width: SMS_WIDTH,
            height: SMS_HEIGHT,
            depth_or_array_layers: 1,
        }
    }

    /// Descreve o layout tightly-packed RGBA8 exigido pelo upload WGPU.
    fn texture_data_layout() -> wgpu::TexelCopyBufferLayout {
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(SMS_WIDTH * 4),
            rows_per_image: Some(SMS_HEIGHT),
        }
    }

    /// Converte um frame imutável RGB888 em RGBA8, respeitando o stride de cada linha.
    fn convert_rgb888_to_rgba8(frame: &VideoFrame) -> Result<Vec<u8>, String> {
        if frame.pixel_format() != PixelFormat::Rgb888 {
            return Err(format!(
                "formato de frame incompatível: esperado RGB888, recebido {:?}",
                frame.pixel_format()
            ));
        }

        let width = usize::try_from(frame.width())
            .map_err(|_| "largura do frame não cabe em usize".to_owned())?;
        let height = usize::try_from(frame.height())
            .map_err(|_| "altura do frame não cabe em usize".to_owned())?;
        let output_len = width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| "dimensões do frame excedem o limite de memória".to_owned())?;
        let expected_row_bytes = width
            .checked_mul(3)
            .ok_or_else(|| "largura do frame excede o limite de memória".to_owned())?;

        let mut rgba = Vec::with_capacity(output_len);
        for y in 0..frame.height() {
            let row = frame
                .row(y)
                .ok_or_else(|| format!("linha {y} ausente no frame"))?;
            if row.len() != expected_row_bytes {
                return Err(format!(
                    "linha {y} possui {} bytes; esperado {expected_row_bytes}",
                    row.len()
                ));
            }
            for rgb in row.chunks_exact(3) {
                rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }

        if rgba.len() != output_len {
            return Err(format!(
                "conversão produziu {} bytes; esperado {output_len}",
                rgba.len()
            ));
        }
        Ok(rgba)
    }

    /// Cria uma textura importável pelo Slint, usando o device e queue do próprio renderer.
    fn create_imported_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<slint::Image, Box<dyn Error>> {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sega-r2-sms-vdp-frame"),
            size: texture_extent(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let initial_pixels = vec![0_u8; (SMS_WIDTH * SMS_HEIGHT * 4) as usize];
        queue.write_texture(
            texture.as_image_copy(),
            &initial_pixels,
            texture_data_layout(),
            texture_extent(),
        );

        Ok(slint::Image::try_from(texture)?)
    }

    /// Registra o device/queue WGPU do Slint e publica a textura externa na mesma cena.
    fn attach_gpu_texture(
        app: &MainWindow,
        queue_slot: Rc<RefCell<Option<wgpu::Queue>>>,
        setup_counter: Rc<Cell<u64>>,
        teardown_counter: Rc<Cell<u64>>,
    ) -> Result<(), Box<dyn Error>> {
        let app_weak = app.as_weak();

        app.window().set_rendering_notifier(move |state, graphics_api| {
            match (state, graphics_api) {
                (
                    RenderingState::RenderingSetup,
                    GraphicsAPI::WGPU30 { device, queue, .. },
                ) => {
                    let setup_count = setup_counter.get().wrapping_add(1);
                    setup_counter.set(setup_count);

                    let adapter_info = device.adapter_info();
                    eprintln!(
                        "R2_LIFECYCLE event=RenderingSetup count={setup_count} backend={:?} adapter={}",
                        adapter_info.backend, adapter_info.name
                    );

                    if adapter_info.backend != wgpu::Backend::Vulkan {
                        let app_weak = app_weak.clone();
                        let status = format!(
                            "FALHA: backend esperado Vulkan; recebido {:?} ({})",
                            adapter_info.backend, adapter_info.name
                        );
                        Timer::single_shot(Duration::ZERO, move || {
                            if let Some(app) = app_weak.upgrade() {
                                app.set_gpu_status(status.into());
                            }
                        });
                        return;
                    }

                    match create_imported_texture(device, queue) {
                        Ok(image) => {
                            *queue_slot.borrow_mut() = Some(queue.clone());

                            let app_weak = app_weak.clone();
                            let status = format!(
                                "PASS API: Vulkan | adapter: {} | Slint/WGPU 30 compartilham device/queue",
                                adapter_info.name
                            );
                            let lifecycle_status = format!(
                                "Renderer setup: {setup_count} | teardown: {}",
                                teardown_counter.get()
                            );

                            // Rendering notifiers run while the Winit adapter is borrowed.
                            // Defer Slint property mutations until the notifier has returned.
                            Timer::single_shot(Duration::ZERO, move || {
                                if let Some(app) = app_weak.upgrade() {
                                    app.set_gpu_texture(image);
                                    app.set_gpu_status(status.into());
                                    app.set_lifecycle_status(lifecycle_status.into());
                                }
                            });
                        }
                        Err(error) => {
                            eprintln!("R2_LIFECYCLE event=TextureImportFailed error={error}");
                            let app_weak = app_weak.clone();
                            let status = format!("FALHA ao importar textura WGPU: {error}");
                            Timer::single_shot(Duration::ZERO, move || {
                                if let Some(app) = app_weak.upgrade() {
                                    app.set_gpu_status(status.into());
                                }
                            });
                        }
                    }
                }
                (RenderingState::RenderingTeardown, _) => {
                    let teardown_count = teardown_counter.get().wrapping_add(1);
                    teardown_counter.set(teardown_count);
                    eprintln!(
                        "R2_LIFECYCLE event=RenderingTeardown count={teardown_count}"
                    );

                    *queue_slot.borrow_mut() = None;

                    let app_weak = app_weak.clone();
                    let lifecycle_status = format!(
                        "Renderer setup: {} | teardown: {teardown_count}",
                        setup_counter.get()
                    );

                    // Do not mutate Slint properties from inside RenderingTeardown:
                    // Winit still holds an internal RefCell borrow at this point.
                    Timer::single_shot(Duration::ZERO, move || {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_gpu_texture(slint::Image::default());
                            app.set_lifecycle_status(lifecycle_status.into());
                        }
                    });
                }
                (RenderingState::RenderingSetup, _) => {
                    eprintln!(
                        "R2_LIFECYCLE event=RenderingSetupFailed reason=GraphicsAPI_not_WGPU30"
                    );
                    let app_weak = app_weak.clone();
                    Timer::single_shot(Duration::ZERO, move || {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_gpu_status(
                                "FALHA: o renderer Slint não forneceu GraphicsAPI::WGPU30.".into(),
                            );
                        }
                    });
                }
                _ => {}
            }
        })?;

        Ok(())
    }

    /// Consome somente o frame mais recente e atualiza a textura sem bloquear o worker.
    fn start_frame_presentation(
        app: &MainWindow,
        queue_slot: Rc<RefCell<Option<wgpu::Queue>>>,
        mailbox: FrameMailbox,
    ) -> Timer {
        let timer = Timer::default();
        let app_weak = app.as_weak();

        timer.start(TimerMode::Repeated, PRESENT_INTERVAL, move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            let Some(queue) = queue_slot.borrow().as_ref().cloned() else {
                return;
            };
            let Some(texture) = app.get_gpu_texture().to_wgpu_30_texture() else {
                return;
            };

            let frame = match mailbox.take_latest() {
                Ok(Some(frame)) => frame,
                Ok(None) => return,
                Err(error) => {
                    app.set_frame_status(format!("Erro no mailbox: {error}").into());
                    return;
                }
            };

            if frame.width() != SMS_WIDTH || frame.height() != SMS_HEIGHT {
                app.set_frame_status(
                    format!(
                        "Frame ignorado: {}x{}; esperado {}x{}",
                        frame.width(),
                        frame.height(),
                        SMS_WIDTH,
                        SMS_HEIGHT
                    )
                    .into(),
                );
                return;
            }

            match convert_rgb888_to_rgba8(&frame) {
                Ok(pixels) => {
                    queue.write_texture(
                        texture.as_image_copy(),
                        &pixels,
                        texture_data_layout(),
                        texture_extent(),
                    );
                    app.set_frame_status(
                        format!(
                            "VDP frame: {} | {}x{} RGB888 → RGBA8",
                            frame.sequence(),
                            frame.width(),
                            frame.height()
                        )
                        .into(),
                    );
                    app.window().request_redraw();
                }
                Err(error) => {
                    app.set_frame_status(format!("Falha ao converter frame: {error}").into());
                }
            }
        });
        timer
    }

    /// Carrega uma ROM SMS e conecta o worker ao mailbox de apresentação.
    fn create_sms_worker(
        rom_path: &PathBuf,
        region: &str,
    ) -> Result<(SmsTemporalWorker, FrameMailbox), Box<dyn Error>> {
        let rom = fs::read(rom_path)?;
        let worker = SmsTemporalWorker::new(rom, region)
            .map_err(|error| format!("não foi possível iniciar o runtime SMS: {error}"))?;
        let mailbox = FrameMailbox::new();
        worker
            .attach_video_mailbox(mailbox.clone())
            .map_err(|error| format!("não foi possível conectar o mailbox de vídeo: {error}"))?;
        Ok((worker, mailbox))
    }

    /// Executa o host SMS; a GUI envia somente comandos coarse-grained ao worker.
    pub fn run() -> Result<(), Box<dyn Error>> {
        let mut args = std::env::args_os().skip(1);
        let rom_path = args
            .next()
            .map(PathBuf::from)
            .ok_or("uso: sega-desktop.exe <rom.sms> [usa|brazil|europe|japan]")?;
        let region = args
            .next()
            .and_then(|value| value.into_string().ok())
            .unwrap_or_else(|| "usa".to_owned());

        select_vulkan_backend()?;
        let (worker, mailbox) = create_sms_worker(&rom_path, &region)?;
        let worker = Rc::new(RefCell::new(Some(worker)));

        let app = MainWindow::new()?;
        app.set_gpu_status("Aguardando inicialização do renderer WGPU 30...".into());
        app.set_frame_status("Aguardando o primeiro frame completo do VDP.".into());
        app.set_lifecycle_status("Renderer setup: 0 | teardown: 0".into());
        app.set_rom_status(format!("ROM: {} | região: {region}", rom_path.display()).into());

        let start_worker = Rc::clone(&worker);
        app.on_start_requested(move || {
            if let Some(worker) = start_worker.borrow().as_ref() {
                if let Err(error) = worker.start() {
                    eprintln!("R2_CONTROL action=start error={error}");
                }
            }
        });

        let pause_worker = Rc::clone(&worker);
        app.on_pause_requested(move || {
            if let Some(worker) = pause_worker.borrow().as_ref() {
                if let Err(error) = worker.pause() {
                    eprintln!("R2_CONTROL action=pause error={error}");
                }
            }
        });

        let queue_slot = Rc::new(RefCell::new(None));
        let setup_counter = Rc::new(Cell::new(0_u64));
        let teardown_counter = Rc::new(Cell::new(0_u64));
        attach_gpu_texture(
            &app,
            Rc::clone(&queue_slot),
            setup_counter,
            teardown_counter,
        )?;
        let _frame_timer = start_frame_presentation(&app, queue_slot, mailbox.clone());

        app.run()?;

        if let Some(worker) = worker.borrow().as_ref() {
            let _ = worker.pause();
            let _ = worker.detach_video_mailbox();
        }
        let _ = mailbox.close();
        app.set_gpu_texture(slint::Image::default());
        worker.borrow_mut().take();
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn rgb888_conversion_respects_row_stride_and_sets_opaque_alpha() {
            let frame = VideoFrame::new(
                7,
                2,
                2,
                PixelFormat::Rgb888,
                8,
                vec![
                    255, 0, 0, 0, 255, 0, 90, 91, 0, 0, 255, 255, 255, 255, 92, 93,
                ],
            )
            .unwrap();

            assert_eq!(
                convert_rgb888_to_rgba8(&frame).unwrap(),
                vec![
                    255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
                ]
            );
        }

        #[test]
        fn rgb888_conversion_rejects_other_pixel_formats() {
            let frame =
                VideoFrame::new(1, 1, 1, PixelFormat::Rgba8888, 4, vec![1, 2, 3, 4]).unwrap();
            assert!(
                convert_rgb888_to_rgba8(&frame)
                    .unwrap_err()
                    .contains("esperado RGB888")
            );
        }

        #[test]
        fn sms_worker_publishes_complete_vdp_frames_to_desktop_mailbox() {
            let worker = SmsTemporalWorker::new(vec![0; 0x4000], "usa").unwrap();
            let mailbox = FrameMailbox::new();
            worker.attach_video_mailbox(mailbox.clone()).unwrap();
            worker.step_frame(100_000).unwrap();

            let frame = mailbox.take_latest().unwrap().unwrap();
            assert_eq!(frame.sequence(), 1);
            assert_eq!(frame.width(), SMS_WIDTH);
            assert_eq!(frame.height(), SMS_HEIGHT);
            assert_eq!(frame.pixel_format(), PixelFormat::Rgb888);
            assert_eq!(frame.pixels().len(), (SMS_WIDTH * SMS_HEIGHT * 3) as usize);
            assert!(worker.machine_time_ticks() > 0);
            worker.detach_video_mailbox().unwrap();
        }
    }
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    windows_host::run()
}

#[cfg(not(windows))]
fn main() {
    println!("sega-desktop R2 currently targets Windows x64 with Vulkan.");
}
