#![deny(unsafe_code)]

#[cfg(windows)]
mod windows_host {
    use std::{
        cell::{Cell, RefCell},
        error::Error,
        fs,
        path::PathBuf,
        process::Command,
        rc::Rc,
        time::{Duration, Instant},
    };

    use sega_sms_runtime::{SmsTemporalWorker, SpriteLimitMode};
    use sega_video_runtime::{FrameMailbox, PixelFormat, VideoFrame};
    use std::os::windows::process::CommandExt;

    use slint::{
        GraphicsAPI, RenderingState, Timer, TimerMode,
        wgpu_30::{WGPUConfiguration, WGPUSettings, wgpu},
    };

    const SMS_WIDTH: u32 = 256;
    const SMS_HEIGHT: u32 = 192;
    const PRESENT_INTERVAL: Duration = Duration::from_millis(16);
    const APP_DATA_DIRECTORY: &str = "Sega Emulator";
    const MAX_RECENT_ROMS: usize = 10;

    slint::slint! {
        import { Button, LineEdit } from "std-widgets.slint";
        export component MainWindow inherits Window {
            title: "Sega Emulator — Slint / WGPU 30 / Vulkan";
            preferred-width: 1120px; preferred-height: 860px; background: #101820;
            in-out property <image> gpu-texture;
            in-out property <string> gpu-status;
            in-out property <string> frame-status;
            in-out property <string> lifecycle-status;
            in-out property <string> rom-status;
            in-out property <string> active-menu: "none";
            in-out property <string> selected-region: "usa";
            in-out property <string> selected-aspect: "4:3";
            in-out property <string> recent-rom-path: "";
            in-out property <[string]> recent-roms: [];
            in-out property <string> fps-size: "medium";
            in-out property <length> fps-font-size: 18px;
            in-out property <string> rom-path-input: "";
            in-out property <string> debug-status: "Debugger ainda não consultado.";
            in-out property <bool> has-rom: false;
            in-out property <bool> emulation-paused: false;
            in-out property <bool> sprite-limit-enabled: true;
            in-out property <bool> show-fps: true;
            in-out property <bool> show-status-footer: true;
            in-out property <bool> trace-enabled: false;
            in-out property <bool> fullscreen-enabled: false;
            in-out property <string> fps-status: "FPS: --";
            in-out property <length> video-width: 768px;
            in-out property <length> video-height: 576px;
            callback start-requested();
            callback pause-requested();
            callback stop-requested();
            callback reset-requested();
            callback step-frame-requested();
            callback load-rom-requested(string);
            callback browse-rom-requested();
            callback close-rom-requested();
            callback exit-requested();
            callback debug-snapshot-requested();
            callback trace-toggle-requested(bool);
            callback sprite-limit-toggle-requested(bool);
            callback fullscreen-requested(bool);
            callback video-area-clicked();
            callback preferences-changed();
            FocusScope {
                width: 100%; height: 100%;
                KeyBinding {
                    keys: @keys(F11);
                    activated => {
                        root.fullscreen-enabled = !root.fullscreen-enabled;
                        root.fullscreen-requested(root.fullscreen-enabled);
                    }
                }
                VerticalLayout {
                padding: 18px; spacing: 10px;
                Text { text: "SEGA EMULATOR"; color: #f2f5f7; font-size: 25px; }
                HorizontalLayout {
                    spacing: 5px;
                    Button { text: "Sistema"; clicked => { root.emulation-paused = true; root.pause-requested(); root.active-menu = "system"; } }
                    Button { text: "Arquivo"; clicked => { root.emulation-paused = true; root.pause-requested(); root.active-menu = "file"; } }
                    Button { text: "Vídeo"; clicked => { root.emulation-paused = true; root.pause-requested(); root.active-menu = "video"; } }
                    Button { text: "Áudio"; clicked => { root.emulation-paused = true; root.pause-requested(); root.active-menu = "audio"; } }
                    Button { text: "Controles"; clicked => { root.emulation-paused = true; root.pause-requested(); root.active-menu = "controls"; } }
                    Button { text: "Opções"; clicked => { root.emulation-paused = true; root.pause-requested(); root.active-menu = "options"; } }
                    Button { text: "DEBUG"; clicked => { root.emulation-paused = true; root.pause-requested(); root.active-menu = "debug"; } }
                    Button { text: "Fechar menu"; clicked => { root.active-menu = "none"; } }
                }
                if root.active-menu != "none": Rectangle {
                    background: #1b2834; border-radius: 6px; border-width: 1px;
                    border-color: #405365; min-height: 170px; max-height: 280px;
                    max-width: 820px; cross-axis-self-alignment: center;
                    VerticalLayout {
                        padding: 12px; spacing: 7px;
                        if root.active-menu == "system": VerticalLayout {
                            Text { text: "Sistema ativo: Sega Master System"; color: #f2f5f7; }
                            Text { text: "Região / padrão de vídeo"; color: #b9c5ce; }
                            HorizontalLayout {
                                Button { text: root.selected-region == "usa" ? "✓ USA / NTSC" : "USA / NTSC"; clicked => { root.selected-region = "usa"; root.preferences-changed(); } }
                                Button { text: root.selected-region == "brazil" ? "✓ Brasil / PAL-M" : "Brasil / PAL-M"; clicked => { root.selected-region = "brazil"; root.preferences-changed(); } }
                                Button { text: root.selected-region == "europe" ? "✓ Europa / PAL" : "Europa / PAL"; clicked => { root.selected-region = "europe"; root.preferences-changed(); } }
                                Button { text: root.selected-region == "japan" ? "✓ Japão / NTSC" : "Japão / NTSC"; clicked => { root.selected-region = "japan"; root.preferences-changed(); } }
                            }
                            Text { text: "A região selecionada será aplicada ao carregar a próxima ROM."; color: #b9c5ce; }
                            HorizontalLayout {
                                Button { text: "SG-1000"; enabled: false; }
                                Button { text: "Game Gear"; enabled: false; }
                                Button { text: "Mega Drive / Genesis"; enabled: false; }
                                Button { text: "Sega CD"; enabled: false; }
                                Button { text: "32X"; enabled: false; }
                            }
                            Text { text: "Outros sistemas: integração de runtime ainda não concluída."; color: #9baab6; }
                        }
                        if root.active-menu == "file": VerticalLayout {
                            LineEdit { text <=> root.rom-path-input; placeholder-text: "C:\\ROMs\\game.sms"; }
                            HorizontalLayout {
                                Button { text: "Procurar..."; clicked => { root.browse-rom-requested(); } }
                                Button { text: "Carregar ROM"; clicked => { root.load-rom-requested(root.rom-path-input); root.active-menu = "none"; } }
                                Button { text: "Fechar ROM"; enabled: root.has-rom; clicked => { root.close-rom-requested(); root.active-menu = "none"; } }
                                Button { text: "Sair"; clicked => { root.exit-requested(); } }
                            }
                            Text { text: "Recentes"; color: #b9c5ce; }
                            if root.recent-roms.length == 0: Text { text: "Nenhuma ROM recente."; color: #9baab6; }
                            for rom in root.recent-roms: Button {
                                text: rom;
                                clicked => { root.load-rom-requested(rom); root.active-menu = "none"; }
                            }
                        }
                        if root.active-menu == "video": VerticalLayout {
                            Text { text: "Proporção da imagem"; color: #b9c5ce; }
                            HorizontalLayout {
                                Button { text: root.selected-aspect == "4:3" ? "✓ Original (4:3)" : "Original (4:3)"; clicked => { root.selected-aspect = "4:3"; root.video-width = 768px; root.video-height = 576px; root.preferences-changed(); } }
                                Button { text: root.selected-aspect == "1:1" ? "✓ 1:1" : "1:1"; clicked => { root.selected-aspect = "1:1"; root.video-width = 576px; root.video-height = 576px; root.preferences-changed(); } }
                                Button { text: root.selected-aspect == "16:9" ? "✓ 16:9" : "16:9"; clicked => { root.selected-aspect = "16:9"; root.video-width = 768px; root.video-height = 432px; root.preferences-changed(); } }
                            }
                            Button { text: root.sprite-limit-enabled ? "✓ Limite de sprites ativo" : "Desativar limite de sprites por linha"; clicked => { root.sprite-limit-enabled = !root.sprite-limit-enabled; root.sprite-limit-toggle-requested(root.sprite-limit-enabled); root.preferences-changed(); } }
                            Text { text: "O modo sem limite é uma opção de compatibilidade; overflow e temporização do VDP permanecem no hardware emulado."; color: #9baab6; }
                            Text { text: "Ajuste aplicado na área de apresentação; o frame original do VDP permanece 256×192."; color: #9baab6; }
                        }
                        if root.active-menu == "audio": VerticalLayout {
                            Text { text: "Configuração de áudio"; color: #f2f5f7; }
                            Button { text: "Áudio ativo"; enabled: false; }
                            HorizontalLayout {
                                Button { text: "WASAPI Shared"; enabled: false; }
                                Button { text: "XAudio2"; enabled: false; }
                            }
                            Text { text: "Volume: 100%"; color: #b9c5ce; }
                            Text { text: "Saída física, dispositivos e volume serão conectados na etapa R3 (CPAL 0.18.2)."; color: #9baab6; }
                        }
                        if root.active-menu == "controls": VerticalLayout {
                            Text { text: "Dispositivo de entrada"; color: #f2f5f7; }
                            Button { text: "Gamepad SDL3 — integração R4"; enabled: false; }
                            Text { text: "Teclado"; color: #b9c5ce; }
                            Text { text: "Cima / Baixo / Esquerda / Direita"; color: #9baab6; }
                            Text { text: "Botão 1 / Botão 2 / Solicitar NMI"; color: #9baab6; }
                            Text { text: "Iniciar / Pausar / Parar / Reset / Avançar quadro"; color: #9baab6; }
                            Button { text: "Remapear teclado"; enabled: false; }
                            Button { text: "Remapear controle"; enabled: false; }
                            Text { text: "Remapeamento e leitura física aguardam a integração SDL3 da R4."; color: #9baab6; }
                        }
                        if root.active-menu == "options": VerticalLayout {
                            Text { text: "Overlay de FPS"; color: #f2f5f7; }
                            Button { text: root.show-fps ? "✓ Exibir HUD de FPS" : "Exibir HUD de FPS"; clicked => { root.show-fps = !root.show-fps; root.preferences-changed(); } }
                            Button { text: root.show-status-footer ? "✓ Exibir status no rodapé" : "Exibir status no rodapé"; clicked => { root.show-status-footer = !root.show-status-footer; root.preferences-changed(); } }
                            HorizontalLayout {
                                Button { text: root.fps-size == "small" ? "✓ Pequeno" : "Pequeno"; clicked => { root.fps-size = "small"; root.fps-font-size = 13px; root.preferences-changed(); } }
                                Button { text: root.fps-size == "medium" ? "✓ Médio" : "Médio"; clicked => { root.fps-size = "medium"; root.fps-font-size = 18px; root.preferences-changed(); } }
                                Button { text: root.fps-size == "large" ? "✓ Grande" : "Grande"; clicked => { root.fps-size = "large"; root.fps-font-size = 24px; root.preferences-changed(); } }
                            }
                            Text { text: "Posição do HUD: inferior (posição em cantos será concluída após validar o overlay."; color: #9baab6; }
                            Button { text: root.fullscreen-enabled ? "✓ Tela cheia" : "Tela cheia (F11)"; clicked => { root.fullscreen-enabled = !root.fullscreen-enabled; root.fullscreen-requested(root.fullscreen-enabled); } }
                        }
                        if root.active-menu == "debug": VerticalLayout {
                            Text { text: "Diagnóstico do runtime SMS"; color: #f2f5f7; }
                            Button { text: "Capturar snapshot do Z80"; enabled: root.has-rom; clicked => { root.debug-snapshot-requested(); } }
                            Button { text: root.trace-enabled ? "Desativar trace" : "Ativar trace"; enabled: root.has-rom; clicked => { root.trace-enabled = !root.trace-enabled; root.trace-toggle-requested(root.trace-enabled); } }
                            Text { text: root.debug-status; color: #b9c5ce; }
                            Text { text: "Debugger com registradores, breakpoints e memória será migrado em etapa própria; a GUI não executa instruções."; color: #9baab6; }
                        }
                    }
                }
                HorizontalLayout {
                    spacing: 8px;
                    Button { text: "Iniciar"; enabled: root.has-rom; clicked => { root.emulation-paused = false; root.start-requested(); } }
                    Button { text: "Pausar"; enabled: root.has-rom; clicked => { root.emulation-paused = true; root.emulation-paused = true; root.pause-requested(); } }
                    Button { text: "Parar"; enabled: root.has-rom; clicked => { root.emulation-paused = false; root.stop-requested(); } }
                    Button { text: "Reset"; enabled: root.has-rom; clicked => { root.emulation-paused = false; root.reset-requested(); } }
                    Button { text: "Avançar um quadro"; enabled: root.has-rom; clicked => { root.emulation-paused = true; root.step-frame-requested(); } }
                    Text { text: root.rom-status; color: #b9c5ce; vertical-alignment: center; }
                }
                Rectangle {
                    horizontal-stretch: 1; vertical-stretch: 1;
                    min-width: 256px; min-height: 192px;
                    background: #05080b;
                    Image {
                        source: root.gpu-texture;
                        width: min(parent.width, parent.height * (root.selected-aspect == "1:1" ? 1.0 : root.selected-aspect == "16:9" ? 16.0 / 9.0 : 4.0 / 3.0));
                        height: min(parent.height, parent.width / (root.selected-aspect == "1:1" ? 1.0 : root.selected-aspect == "16:9" ? 16.0 / 9.0 : 4.0 / 3.0));
                        x: (parent.width - self.width) / 2;
                        y: (parent.height - self.height) / 2;
                        image-fit: fill;
                    }
                    TouchArea {
                        width: 100%; height: 100%;
                        clicked => { root.video-area-clicked(); }
                    }
                }
                if root.show-fps: Text { text: root.fps-status; color: #8bd5ca; font-size: root.fps-font-size; }
                if root.show-status-footer: Text { text: root.gpu-status; color: #8bd5ca; }
                if root.show-status-footer: Text { text: root.frame-status; color: #b9c5ce; }
                if root.show-status-footer: Text { text: root.lifecycle-status; color: #b9c5ce; }
                }
            }
        }
    }

    /// Preferências persistidas exclusivamente para a camada de apresentação.
    #[derive(Debug, Clone)]
    struct DesktopPreferences {
        region: String,
        aspect: String,
        show_fps: bool,
        show_status_footer: bool,
        fps_size: String,
        sprite_limit_enabled: bool,
    }

    impl Default for DesktopPreferences {
        fn default() -> Self {
            Self {
                region: "usa".to_owned(),
                aspect: "4:3".to_owned(),
                show_fps: true,
                show_status_footer: true,
                fps_size: "medium".to_owned(),
                sprite_limit_enabled: true,
            }
        }
    }

    /// Resolve o diretório de dados por usuário no Windows sem criar diretórios durante leitura.
    fn app_data_directory() -> Option<PathBuf> {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join(APP_DATA_DIRECTORY))
    }

    /// Carrega preferências com fallback individual para valores seguros e conhecidos.
    fn load_desktop_preferences() -> DesktopPreferences {
        let mut preferences = DesktopPreferences::default();
        let Some(directory) = app_data_directory() else {
            return preferences;
        };
        let Ok(contents) = fs::read_to_string(directory.join("slint_ui_settings.txt")) else {
            return preferences;
        };

        for line in contents.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "region" if ["usa", "brazil", "europe", "japan"].contains(&value.trim()) => {
                    preferences.region = value.trim().to_owned();
                }
                "aspect" if ["4:3", "1:1", "16:9"].contains(&value.trim()) => {
                    preferences.aspect = value.trim().to_owned();
                }
                "show_fps" => {
                    if let Ok(value) = value.trim().parse() {
                        preferences.show_fps = value;
                    }
                }
                "show_status_footer" => {
                    if let Ok(value) = value.trim().parse() {
                        preferences.show_status_footer = value;
                    }
                }
                "fps_size" if ["small", "medium", "large"].contains(&value.trim()) => {
                    preferences.fps_size = value.trim().to_owned();
                }
                "sprite_limit_enabled" => {
                    if let Ok(value) = value.trim().parse() {
                        preferences.sprite_limit_enabled = value;
                    }
                }
                _ => {}
            }
        }
        preferences
    }

    /// Salva as preferências em arquivo simples, sem serializar estado do hardware.
    fn save_desktop_preferences(preferences: &DesktopPreferences) -> Result<(), String> {
        let directory =
            app_data_directory().ok_or_else(|| "APPDATA não está definido".to_owned())?;
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let contents = format!(
            "region={}\naspect={}\nshow_fps={}\nshow_status_footer={}\nfps_size={}\nsprite_limit_enabled={}\n",
            preferences.region,
            preferences.aspect,
            preferences.show_fps,
            preferences.show_status_footer,
            preferences.fps_size,
            preferences.sprite_limit_enabled
        );
        fs::write(directory.join("slint_ui_settings.txt"), contents)
            .map_err(|error| error.to_string())
    }

    /// Carrega até dez ROMs recentes, removendo entradas duplicadas e vazias.
    fn load_recent_roms() -> Vec<PathBuf> {
        let Some(directory) = app_data_directory() else {
            return Vec::new();
        };
        let Ok(contents) = fs::read_to_string(directory.join("recent_roms.txt")) else {
            return Vec::new();
        };
        let mut recent = Vec::new();
        for line in contents.lines().filter(|line| !line.trim().is_empty()) {
            let path = PathBuf::from(line);
            if !recent.contains(&path) {
                recent.push(path);
            }
            if recent.len() == MAX_RECENT_ROMS {
                break;
            }
        }
        recent
    }

    /// Persiste a lista recente, mantendo a ROM mais nova no início.
    fn save_recent_roms(recent: &[PathBuf]) -> Result<(), String> {
        let directory =
            app_data_directory().ok_or_else(|| "APPDATA não está definido".to_owned())?;
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let contents = recent
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(directory.join("recent_roms.txt"), contents).map_err(|error| error.to_string())
    }

    /// Move uma ROM para o início da lista e limita o histórico a dez entradas.
    fn record_recent_rom(recent: &mut Vec<PathBuf>, path: PathBuf) {
        recent.retain(|entry| entry != &path);
        recent.insert(0, path);
        recent.truncate(MAX_RECENT_ROMS);
        if let Err(error) = save_recent_roms(recent) {
            eprintln!("R2_PREFERENCES recent_roms_save_failed error={error}");
        }
    }

    /// Converte a lista de caminhos em um modelo imutável para o menu Slint.
    fn recent_roms_model(recent: &[PathBuf]) -> Vec<slint::SharedString> {
        recent
            .iter()
            .map(|path| path.display().to_string().into())
            .collect()
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
            for rgb in row.as_chunks::<3>().0 {
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
        let mut last_sequence = 0_u64;
        let mut last_sample = Instant::now();

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

            let now = Instant::now();
            let elapsed = now.saturating_duration_since(last_sample);
            if elapsed >= Duration::from_millis(500) {
                let produced = u32::try_from(frame.sequence().saturating_sub(last_sequence))
                    .unwrap_or(u32::MAX);
                let fps = f64::from(produced) / elapsed.as_secs_f64();
                app.set_fps_status(format!("FPS VDP: {fps:.1}").into());
                last_sequence = frame.sequence();
                last_sample = now;
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

    /// Abre o diálogo nativo de seleção de arquivos do Windows para ROMs SMS.
    ///
    /// O PowerShell 5.1 e o Windows Forms fazem parte do Windows; o diálogo usa
    /// o seletor comum do Shell. O processo auxiliar não recebe entrada do usuário.
    fn pick_rom_file() -> Result<Option<PathBuf>, String> {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const SCRIPT: &str = r#"
            Add-Type -AssemblyName System.Windows.Forms
            [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
            $dialog = New-Object System.Windows.Forms.OpenFileDialog
            $dialog.Title = 'Selecionar ROM do Sega Master System'
            $dialog.Filter = 'ROM Sega 8-bit (*.sms;*.bin)|*.sms;*.bin|Todos os arquivos (*.*)|*.*'
            $dialog.Multiselect = $false
            $dialog.CheckFileExists = $true
            if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
                [Console]::Out.Write($dialog.FileName)
            }
        "#;

        let output = Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-STA",
                "-WindowStyle",
                "Hidden",
                "-Command",
                SCRIPT,
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|error| format!("não foi possível iniciar o seletor do Windows: {error}"))?;

        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }

        let selected = String::from_utf8_lossy(&output.stdout);
        let selected = selected.trim_end_matches(['\r', '\n']);
        if selected.is_empty() {
            Ok(None)
        } else {
            Ok(Some(PathBuf::from(selected)))
        }
    }

    /// Cria um worker SMS e conecta o mailbox de apresentação antes de publicá-lo na GUI.
    fn create_sms_worker(
        rom: Vec<u8>,
        region: &str,
        mailbox: &FrameMailbox,
    ) -> Result<SmsTemporalWorker, String> {
        let worker = SmsTemporalWorker::new(rom, region)?;
        worker.attach_video_mailbox(mailbox.clone())?;
        Ok(worker)
    }

    /// Substitui a máquina sem permitir dois produtores simultâneos no FrameMailbox.
    fn replace_sms_worker(
        path: &PathBuf,
        region: &str,
        worker_slot: &Rc<RefCell<Option<SmsTemporalWorker>>>,
        mailbox: &FrameMailbox,
    ) -> Result<(), String> {
        let rom = fs::read(path).map_err(|error| format!("não foi possível ler a ROM: {error}"))?;
        let next = SmsTemporalWorker::new(rom, region)?;

        let previous = worker_slot.borrow_mut().take();
        if let Some(previous) = previous {
            let was_running = previous.is_running();
            if let Err(error) = previous.pause() {
                *worker_slot.borrow_mut() = Some(previous);
                return Err(error);
            }
            if let Err(error) = previous.detach_video_mailbox() {
                *worker_slot.borrow_mut() = Some(previous);
                return Err(error);
            }
            if let Err(error) = next.attach_video_mailbox(mailbox.clone()) {
                let _ = previous.attach_video_mailbox(mailbox.clone());
                if was_running {
                    let _ = previous.start();
                }
                *worker_slot.borrow_mut() = Some(previous);
                return Err(error);
            }

            *worker_slot.borrow_mut() = Some(next);
            drop(previous);
        } else {
            next.attach_video_mailbox(mailbox.clone())?;
            *worker_slot.borrow_mut() = Some(next);
        }

        Ok(())
    }

    /// Executa o host SMS; a GUI envia somente comandos coarse-grained ao worker.
    pub fn run() -> Result<(), Box<dyn Error>> {
        let mut args = std::env::args_os().skip(1);
        let rom_path = args.next().map(PathBuf::from);
        let preferences = load_desktop_preferences();
        let region = args
            .next()
            .and_then(|value| value.into_string().ok())
            .unwrap_or_else(|| preferences.region.clone());
        let recent_roms = Rc::new(RefCell::new(load_recent_roms()));

        select_vulkan_backend()?;
        let mailbox = FrameMailbox::new();
        let worker = if let Some(path) = rom_path.as_ref() {
            let worker = create_sms_worker(fs::read(path)?, &region, &mailbox)?;
            if !preferences.sprite_limit_enabled {
                worker.set_sprite_limit_mode(SpriteLimitMode::UnlockedCompatibility)?;
            }
            Some(worker)
        } else {
            None
        };
        if let Some(path) = rom_path.as_ref() {
            record_recent_rom(&mut recent_roms.borrow_mut(), path.clone());
        }
        let worker = Rc::new(RefCell::new(worker));
        let recent_rom = Rc::new(RefCell::new(rom_path.clone()));

        let app = MainWindow::new()?;
        app.set_gpu_status("Aguardando inicialização do renderer WGPU 30...".into());
        app.set_frame_status("Aguardando o primeiro frame completo do VDP.".into());
        app.set_lifecycle_status("Renderer setup: 0 | teardown: 0".into());
        app.set_selected_region(region.clone().into());
        app.set_show_fps(preferences.show_fps);
        app.set_show_status_footer(preferences.show_status_footer);
        app.set_sprite_limit_enabled(preferences.sprite_limit_enabled);
        app.set_fps_size(preferences.fps_size.clone().into());
        app.set_fps_font_size(match preferences.fps_size.as_str() {
            "small" => 13.0,
            "large" => 24.0,
            _ => 18.0,
        });
        app.set_recent_roms(slint::ModelRc::new(slint::VecModel::from(
            recent_roms_model(&recent_roms.borrow()),
        )));
        match preferences.aspect.as_str() {
            "1:1" => {
                app.set_selected_aspect("1:1".into());
                app.set_video_width(576.0);
                app.set_video_height(576.0);
            }
            "16:9" => {
                app.set_selected_aspect("16:9".into());
                app.set_video_width(768.0);
                app.set_video_height(432.0);
            }
            _ => {
                app.set_selected_aspect("4:3".into());
                app.set_video_width(768.0);
                app.set_video_height(576.0);
            }
        }
        app.set_has_rom(worker.borrow().is_some());
        if let Some(path) = rom_path.as_ref() {
            app.set_rom_status(format!("ROM: {} | região: {region}", path.display()).into());
            app.set_recent_rom_path(path.display().to_string().into());
            app.set_rom_path_input(path.display().to_string().into());
        } else {
            app.set_rom_status(
                "Nenhuma ROM carregada — informe o caminho em Arquivo > Carregar ROM.".into(),
            );
        }

        if let Some(initial_worker) = worker.borrow().as_ref() {
            initial_worker.start()?;
        }

        let start_worker = Rc::clone(&worker);
        let start_app = app.as_weak();
        app.on_start_requested(move || {
            if let Some(worker) = start_worker.borrow().as_ref()
                && let Err(error) = worker.start()
            {
                eprintln!("R2_CONTROL action=start error={error}");
            } else if let Some(app) = start_app.upgrade() {
                app.set_emulation_paused(false);
            }
        });

        let pause_worker = Rc::clone(&worker);
        let pause_app = app.as_weak();
        app.on_pause_requested(move || {
            if let Some(worker) = pause_worker.borrow().as_ref()
                && let Err(error) = worker.pause()
            {
                eprintln!("R2_CONTROL action=pause error={error}");
            } else if let Some(app) = pause_app.upgrade() {
                app.set_emulation_paused(true);
            }
        });

        let stop_worker = Rc::clone(&worker);
        let stop_app = app.as_weak();
        app.on_stop_requested(move || {
            if let Some(worker) = stop_worker.borrow().as_ref()
                && let Err(error) = worker.stop()
            {
                eprintln!("R2_CONTROL action=stop error={error}");
            } else if let Some(app) = stop_app.upgrade() {
                app.set_emulation_paused(false);
            }
        });

        let reset_worker = Rc::clone(&worker);
        app.on_reset_requested(move || {
            if let Some(worker) = reset_worker.borrow().as_ref()
                && let Err(error) = worker.reset()
            {
                eprintln!("R2_CONTROL action=reset error={error}");
            }
        });

        let step_worker = Rc::clone(&worker);
        app.on_step_frame_requested(move || {
            if let Some(worker) = step_worker.borrow().as_ref()
                && let Err(error) = worker.step_frame(100_000)
            {
                eprintln!("R2_CONTROL action=step_frame error={error}");
            }
        });

        let open_app = app.as_weak();
        let open_worker = Rc::clone(&worker);
        let open_recent = Rc::clone(&recent_rom);
        let open_recent_list = Rc::clone(&recent_roms);
        let open_mailbox = mailbox.clone();
        app.on_load_rom_requested(move |path: slint::SharedString| {
            let path = PathBuf::from(path.as_str());
            if path.as_os_str().is_empty() {
                if let Some(app) = open_app.upgrade() {
                    app.set_rom_status("Informe o caminho completo da ROM (.sms ou .bin).".into());
                }
                return;
            }

            let (region, sprite_limit_enabled) = open_app
                .upgrade()
                .map(|app| {
                    (
                        app.get_selected_region().to_string(),
                        app.get_sprite_limit_enabled(),
                    )
                })
                .unwrap_or_else(|| ("usa".to_owned(), true));

            match replace_sms_worker(&path, &region, &open_worker, &open_mailbox) {
                Ok(()) => {
                    let sprite_mode = if sprite_limit_enabled {
                        SpriteLimitMode::HardwareLimit
                    } else {
                        SpriteLimitMode::UnlockedCompatibility
                    };
                    let start_result = open_worker.borrow().as_ref().map(|worker| {
                        worker
                            .set_sprite_limit_mode(sprite_mode)
                            .and_then(|()| worker.reset())
                            .and_then(|()| worker.start())
                    });
                    *open_recent.borrow_mut() = Some(path.clone());
                    record_recent_rom(&mut open_recent_list.borrow_mut(), path.clone());
                    if let Some(app) = open_app.upgrade() {
                        app.set_recent_roms(slint::ModelRc::new(slint::VecModel::from(
                            recent_roms_model(&open_recent_list.borrow()),
                        )));
                        app.set_has_rom(true);
                        app.set_emulation_paused(false);
                        app.set_rom_status(
                            match start_result {
                                Some(Ok(())) => format!(
                                    "ROM: {} | região: {region} | execução iniciada",
                                    path.display()
                                ),
                                Some(Err(error)) => {
                                    format!("ROM: {} | falha ao iniciar: {error}", path.display())
                                }
                                None => {
                                    "ROM carregada, mas o worker não está disponível.".to_owned()
                                }
                            }
                            .into(),
                        );
                        app.set_recent_rom_path(path.display().to_string().into());
                        app.set_rom_path_input(path.display().to_string().into());
                        app.set_debug_status(
                            "ROM carregada; snapshot ainda não solicitado.".into(),
                        );
                    }
                }
                Err(error) => {
                    if let Some(app) = open_app.upgrade() {
                        app.set_rom_status(format!("Falha ao carregar ROM: {error}").into());
                    }
                }
            }
        });

        let browse_app = app.as_weak();
        app.on_browse_rom_requested(move || match pick_rom_file() {
            Ok(Some(path)) => {
                if let Some(app) = browse_app.upgrade() {
                    let selected_path = path.to_string_lossy().into_owned();
                    app.set_rom_path_input(selected_path.clone().into());
                    app.set_active_menu("none".into());
                    app.invoke_load_rom_requested(selected_path.into());
                }
            }
            Ok(None) => {}
            Err(error) => {
                if let Some(app) = browse_app.upgrade() {
                    app.set_rom_status(format!("Seletor de arquivos indisponível: {error}").into());
                }
            }
        });

        let close_app = app.as_weak();
        let close_worker = Rc::clone(&worker);
        app.on_close_rom_requested(move || {
            if let Some(previous) = close_worker.borrow_mut().take() {
                let _ = previous.stop();
                let _ = previous.detach_video_mailbox();
                drop(previous);
            }
            if let Some(app) = close_app.upgrade() {
                app.set_has_rom(false);
                app.set_emulation_paused(false);
                app.set_rom_status("Nenhuma ROM carregada.".into());
                app.set_debug_status("Máquina fechada.".into());
            }
        });

        let exit_recent = Rc::clone(&recent_roms);
        let exit_prefs_app = app.as_weak();
        app.on_exit_requested(move || {
            let _ = save_recent_roms(&exit_recent.borrow());
            if let Some(app) = exit_prefs_app.upgrade() {
                let prefs = DesktopPreferences {
                    region: app.get_selected_region().to_string(),
                    aspect: app.get_selected_aspect().to_string(),
                    show_fps: app.get_show_fps(),
                    show_status_footer: app.get_show_status_footer(),
                    fps_size: app.get_fps_size().to_string(),
                    sprite_limit_enabled: app.get_sprite_limit_enabled(),
                };
                let _ = save_desktop_preferences(&prefs);
                let _ = app.window().hide();
            }
        });

        let debug_app = app.as_weak();
        let debug_worker = Rc::clone(&worker);
        app.on_debug_snapshot_requested(move || {
            let worker_guard = debug_worker.borrow();
            let Some(worker) = worker_guard.as_ref() else {
                return;
            };
            let result = worker.debug_snapshot().map(|snapshot| {
                format!(
                    "PC={:04X} SP={:04X} AF={:04X} BC={:04X} DE={:04X} HL={:04X} | frame={} ticks={} T-states={}",
                    snapshot.pc,
                    snapshot.sp,
                    snapshot.af,
                    snapshot.bc,
                    snapshot.de,
                    snapshot.hl,
                    snapshot.frame_number,
                    snapshot.machine_time_ticks,
                    snapshot.instruction_tstates
                )
            });
            if let Some(app) = debug_app.upgrade() {
                app.set_debug_status(
                    result
                        .unwrap_or_else(|error| format!("Falha no snapshot: {error}"))
                        .into(),
                );
            }
        });

        let preferences_app = app.as_weak();
        app.on_preferences_changed(move || {
            let Some(app) = preferences_app.upgrade() else {
                return;
            };
            let prefs = DesktopPreferences {
                region: app.get_selected_region().to_string(),
                aspect: app.get_selected_aspect().to_string(),
                show_fps: app.get_show_fps(),
                show_status_footer: app.get_show_status_footer(),
                fps_size: app.get_fps_size().to_string(),
                sprite_limit_enabled: app.get_sprite_limit_enabled(),
            };
            if let Err(error) = save_desktop_preferences(&prefs) {
                eprintln!("R2_PREFERENCES save_failed error={error}");
            }
        });

        let fullscreen_app = app.as_weak();
        app.on_fullscreen_requested(move |enabled| {
            if let Some(app) = fullscreen_app.upgrade() {
                app.window().set_fullscreen(enabled);
            }
        });

        let video_click_worker = Rc::clone(&worker);
        let video_click_app = app.as_weak();
        app.on_video_area_clicked(move || {
            let paused = video_click_app
                .upgrade()
                .is_some_and(|app| app.get_has_rom() && app.get_emulation_paused());
            if paused && let Some(worker) = video_click_worker.borrow().as_ref() {
                match worker.start() {
                    Ok(()) => {
                        if let Some(app) = video_click_app.upgrade() {
                            app.set_emulation_paused(false);
                        }
                    }
                    Err(error) => {
                        if let Some(app) = video_click_app.upgrade() {
                            app.set_rom_status(
                                format!("Falha ao retomar a emulação: {error}").into(),
                            );
                        }
                    }
                }
            }
        });

        let sprite_worker = Rc::clone(&worker);
        let sprite_app = app.as_weak();
        app.on_sprite_limit_toggle_requested(move |enabled| {
            let mode = if enabled {
                SpriteLimitMode::HardwareLimit
            } else {
                SpriteLimitMode::UnlockedCompatibility
            };
            if let Some(worker) = sprite_worker.borrow().as_ref()
                && let Err(error) = worker.set_sprite_limit_mode(mode)
            {
                eprintln!("R2_VIDEO action=sprite_limit_toggle error={error}");
                if let Some(app) = sprite_app.upgrade() {
                    app.set_sprite_limit_enabled(!enabled);
                    app.set_rom_status(
                        format!("Falha ao alterar limite de sprites: {error}").into(),
                    );
                }
            }
        });

        let trace_worker = Rc::clone(&worker);
        app.on_trace_toggle_requested(move |enabled| {
            if let Some(worker) = trace_worker.borrow().as_ref()
                && let Err(error) = worker.set_trace_enabled(enabled)
            {
                eprintln!("R2_CONTROL action=trace_toggle error={error}");
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
        let final_preferences = DesktopPreferences {
            region: app.get_selected_region().to_string(),
            aspect: app.get_selected_aspect().to_string(),
            show_fps: app.get_show_fps(),
            show_status_footer: app.get_show_status_footer(),
            fps_size: app.get_fps_size().to_string(),
            sprite_limit_enabled: app.get_sprite_limit_enabled(),
        };
        let _ = save_desktop_preferences(&final_preferences);
        let _ = save_recent_roms(&recent_roms.borrow());

        if let Some(worker) = worker.borrow_mut().take() {
            let _ = worker.pause();
            let _ = worker.detach_video_mailbox();
            drop(worker);
        }
        let _ = mailbox.close();
        app.set_gpu_texture(slint::Image::default());
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
