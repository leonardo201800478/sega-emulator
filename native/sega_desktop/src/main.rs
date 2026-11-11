#![deny(unsafe_code)]

#[cfg(windows)]
mod windows_poc {
    use std::{
        cell::{Cell, RefCell},
        error::Error,
        rc::Rc,
        time::Duration,
    };

    use slint::{
        GraphicsAPI, RenderingState, Timer, TimerMode,
        wgpu_30::{WGPUConfiguration, WGPUSettings, wgpu},
    };

    const TEXTURE_WIDTH: u32 = 320;
    const TEXTURE_HEIGHT: u32 = 240;
    const FRAME_INTERVAL: Duration = Duration::from_millis(16);

    slint::slint! {
        export component MainWindow inherits Window {
            title: "Sega Emulator — Slint / WGPU 30 / Vulkan R1";
            preferred-width: 960px;
            preferred-height: 720px;
            background: #101820;

            in-out property <image> gpu-texture;
            in-out property <string> gpu-status;
            in-out property <string> frame-status;

            VerticalLayout {
                padding: 24px;
                spacing: 16px;

                Text {
                    text: "R1 — Slint + WGPU 30";
                    color: #f2f5f7;
                    font-size: 26px;
                }

                Text {
                    text: "Textura WGPU externa, atualizada continuamente";
                    color: #b9c5ce;
                    font-size: 15px;
                }

                Image {
                    source: root.gpu-texture;
                    width: 640px;
                    height: 480px;
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

    /// Retorna a extensão 2D da textura de demonstração.
    fn texture_extent() -> wgpu::Extent3d {
        wgpu::Extent3d {
            width: TEXTURE_WIDTH,
            height: TEXTURE_HEIGHT,
            depth_or_array_layers: 1,
        }
    }

    /// Descreve o layout tightly-packed RGBA8 usado na transferência para a GPU.
    fn texture_data_layout() -> wgpu::TexelCopyBufferLayout {
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(TEXTURE_WIDTH * 4),
            rows_per_image: Some(TEXTURE_HEIGHT),
        }
    }

    /// Gera um padrão sintético RGBA8 com barras coloridas e um marcador móvel.
    fn render_test_pattern(frame: u64) -> Vec<u8> {
        let mut pixels = vec![0_u8; (TEXTURE_WIDTH * TEXTURE_HEIGHT * 4) as usize];
        let marker_x = ((frame.wrapping_mul(3)) % u64::from(TEXTURE_WIDTH)) as u32;

        for y in 0..TEXTURE_HEIGHT {
            for x in 0..TEXTURE_WIDTH {
                let bar = (x * 8 / TEXTURE_WIDTH) as u8;
                let gradient = ((y + (frame as u32 & 0xff)) & 0xff) as u8;
                let mut rgba: [u8; 4] = match bar {
                    0 => [230, 55, 65, 255],
                    1 => [235, 145, 45, 255],
                    2 => [230, 215, 65, 255],
                    3 => [70, 190, 100, 255],
                    4 => [55, 175, 205, 255],
                    5 => [65, 105, 220, 255],
                    6 => [145, 85, 205, 255],
                    _ => [215, 105, 165, 255],
                };

                rgba[1] = rgba[1].saturating_add(gradient / 5);
                if x.abs_diff(marker_x) < 3 {
                    rgba = [255, 255, 255, 255];
                }

                let offset = ((y * TEXTURE_WIDTH + x) * 4) as usize;
                pixels[offset..offset + 4].copy_from_slice(&rgba);
            }
        }

        pixels
    }

    /// Cria uma textura importável pelo Slint, usando o device e queue do próprio renderer.
    fn create_imported_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<slint::Image, Box<dyn Error>> {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sega-r1-external-test-texture"),
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

        let initial_pixels = render_test_pattern(0);
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
    ) -> Result<(), Box<dyn Error>> {
        let app_weak = app.as_weak();

        app.window().set_rendering_notifier(move |state, graphics_api| {
            match (state, graphics_api) {
                (
                    RenderingState::RenderingSetup,
                    GraphicsAPI::WGPU30 { device, queue, .. },
                ) => {
                    let Some(app) = app_weak.upgrade() else {
                        return;
                    };

                    let adapter_info = device.adapter_info();
                    if adapter_info.backend != wgpu::Backend::Vulkan {
                        app.set_gpu_status(
                            format!(
                                "FALHA: backend esperado Vulkan; recebido {:?} ({})",
                                adapter_info.backend, adapter_info.name
                            )
                            .into(),
                        );
                        return;
                    }

                    match create_imported_texture(device, queue) {
                        Ok(image) => {
                            app.set_gpu_texture(image);
                            app.set_gpu_status(
                                format!(
                                    "PASS API: Vulkan | adapter: {} | Slint/WGPU 30 compartilham device/queue",
                                    adapter_info.name
                                )
                                .into(),
                            );
                            *queue_slot.borrow_mut() = Some(queue.clone());
                        }
                        Err(error) => {
                            app.set_gpu_status(
                                format!("FALHA ao importar textura WGPU: {error}").into(),
                            );
                        }
                    }
                }
                (RenderingState::RenderingTeardown, _) => {
                    *queue_slot.borrow_mut() = None;
                    if let Some(app) = app_weak.upgrade() {
                        app.set_gpu_texture(slint::Image::default());
                    }
                }
                (RenderingState::RenderingSetup, _) => {
                    if let Some(app) = app_weak.upgrade() {
                        app.set_gpu_status(
                            "FALHA: o renderer Slint não forneceu GraphicsAPI::WGPU30.".into(),
                        );
                    }
                }
                _ => {}
            }
        })?;

        Ok(())
    }

    /// Atualiza a textura externa na fila compartilhada e solicita uma nova composição Slint.
    fn start_texture_animation(
        app: &MainWindow,
        queue_slot: Rc<RefCell<Option<wgpu::Queue>>>,
    ) -> Timer {
        let timer = Timer::default();
        let app_weak = app.as_weak();
        let frame_counter = Rc::new(Cell::new(0_u64));
        let timer_counter = Rc::clone(&frame_counter);

        timer.start(TimerMode::Repeated, FRAME_INTERVAL, move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };

            let Some(queue) = queue_slot.borrow().as_ref().cloned() else {
                return;
            };

            let Some(texture) = app.get_gpu_texture().to_wgpu_30_texture() else {
                return;
            };

            let frame = timer_counter.get().wrapping_add(1);
            timer_counter.set(frame);

            let pixels = render_test_pattern(frame);
            queue.write_texture(
                texture.as_image_copy(),
                &pixels,
                texture_data_layout(),
                texture_extent(),
            );

            app.set_frame_status(format!("Atualizações da textura: {frame}").into());
            app.window().request_redraw();
        });

        timer
    }

    /// Executa a POC isolada; nenhum runtime, worker ou Scheduler de emulação é iniciado.
    pub fn run() -> Result<(), Box<dyn Error>> {
        select_vulkan_backend()?;

        let app = MainWindow::new()?;
        app.set_gpu_status("Aguardando inicialização do renderer WGPU 30...".into());
        app.set_frame_status("Aguardando a primeira apresentação.".into());

        let queue_slot = Rc::new(RefCell::new(None));
        attach_gpu_texture(&app, Rc::clone(&queue_slot))?;
        let _texture_timer = start_texture_animation(&app, queue_slot);

        app.run()?;
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_pattern_has_rgba8_extent_and_opaque_alpha() {
            let pixels = render_test_pattern(0);
            assert_eq!(pixels.len(), (TEXTURE_WIDTH * TEXTURE_HEIGHT * 4) as usize);
            assert!(
                pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|pixel| pixel[3] == 255)
            );
        }

        #[test]
        fn test_pattern_changes_between_frames() {
            assert_ne!(render_test_pattern(0), render_test_pattern(1));
        }
    }
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    windows_poc::run()
}

#[cfg(not(windows))]
fn main() {
    println!("sega-desktop R1 currently targets Windows x64 with Vulkan.");
}
