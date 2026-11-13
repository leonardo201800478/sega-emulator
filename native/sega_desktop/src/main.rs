#![deny(unsafe_code)]

#[cfg(windows)]
mod windows_host {
    use std::{
        cell::{Cell, RefCell},
        collections::HashSet,
        error::Error,
        fs,
        io::{Cursor, Read},
        path::PathBuf,
        process::Command,
        rc::Rc,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::{Duration, Instant},
    };

    use sega_media::{CartridgeCatalog, CartridgeHardwareProfile, CartridgeIdentity};
    use sega_save_runtime::SaveRamStore;
    use sega_sms_core::ControllerButtons;
    use sega_sms_runtime::{
        AnalogOutputConfig, AudioRendererMode, SmsTemporalWorker, SpriteLimitMode,
    };
    use sega_video_runtime::{FrameMailbox, PixelFormat, VideoFrame};
    use std::os::windows::process::CommandExt;
    use zip::ZipArchive;

    use slint::{
        GraphicsAPI, RenderingState, Timer, TimerMode,
        wgpu_30::{WGPUConfiguration, WGPUSettings, wgpu},
    };

    const SMS_WIDTH: u32 = 256;
    const SMS_HEIGHT: u32 = 192;
    const DIAGNOSTICS_INTERVAL: Duration = Duration::from_millis(250);
    const APP_DATA_DIRECTORY: &str = "Sega Emulator";
    const MAX_RECENT_ROMS: usize = 10;
    const MAX_ROM_ARCHIVE_BYTES: usize = 32 * 1024 * 1024;
    const MAX_ROM_BYTES: u64 = 16 * 1024 * 1024;
    const MAX_ZIP_ENTRIES: usize = 4_096;

    /// Ações do teclado do host. Não representam ciclos nem instruções da máquina.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum KeyboardAction {
        Up,
        Down,
        Left,
        Right,
        Button1,
        Button2,
        Nmi,
        Start,
        Pause,
        Stop,
        Reset,
        StepFrame,
    }

    impl KeyboardAction {
        fn from_slint_key(key: &str) -> Option<Self> {
            match key {
                "up" => Some(Self::Up),
                "down" => Some(Self::Down),
                "left" => Some(Self::Left),
                "right" => Some(Self::Right),
                "button1" => Some(Self::Button1),
                "button2" => Some(Self::Button2),
                "nmi" => Some(Self::Nmi),
                "start" => Some(Self::Start),
                "pause" => Some(Self::Pause),
                "stop" => Some(Self::Stop),
                "reset" => Some(Self::Reset),
                "step-frame" => Some(Self::StepFrame),
                _ => None,
            }
        }

        fn is_controller_button(self) -> bool {
            matches!(
                self,
                Self::Up | Self::Down | Self::Left | Self::Right | Self::Button1 | Self::Button2
            )
        }
    }

    /// Mantém níveis de teclas pressionadas e compõe um snapshot completo do controle SMS.
    #[derive(Debug, Default)]
    struct KeyboardInputState {
        pressed: HashSet<KeyboardAction>,
    }

    impl KeyboardInputState {
        /// Atualiza uma tecla e retorna a ação somente quando seu estado realmente mudou.
        fn update(&mut self, key: &str, pressed: bool) -> Option<KeyboardAction> {
            let action = KeyboardAction::from_slint_key(key)?;
            let changed = if pressed {
                self.pressed.insert(action)
            } else {
                self.pressed.remove(&action)
            };
            changed.then_some(action)
        }

        /// Compõe os níveis simultâneos das direções e dos dois botões do Master System.
        fn controller_buttons(&self) -> ControllerButtons {
            ControllerButtons {
                up: self.pressed.contains(&KeyboardAction::Up),
                down: self.pressed.contains(&KeyboardAction::Down),
                left: self.pressed.contains(&KeyboardAction::Left),
                right: self.pressed.contains(&KeyboardAction::Right),
                button_1: self.pressed.contains(&KeyboardAction::Button1),
                button_2: self.pressed.contains(&KeyboardAction::Button2),
            }
        }

        /// Descarta todas as teclas, por exemplo quando a janela perde o foco.
        fn clear(&mut self) {
            self.pressed.clear();
        }
    }

    slint::slint! {
        import { Button, CheckBox, LineEdit, ScrollView, Slider } from "std-widgets.slint";
        component CompactButton inherits Button {
            height: 28px;
        }
        component RecentRomEntry inherits Rectangle {
            in property <string> rom-path;
            callback activated();
            width: 100%;
            height: path_label.preferred-height + 12px;
            background: #263541;
            border-radius: 3px;
            path_label := Text {
                x: 6px; y: 6px;
                width: parent.width - 12px;
                text: root.rom-path;
                color: #f2f5f7;
                font-size: 11px;
                wrap: word-wrap;
            }
            TouchArea {
                width: 100%; height: 100%;
                clicked => { root.activated(); }
            }
        }
        export component MainWindow inherits Window {
            forward-focus: keyboard-focus;
            title: "Sega Emulator — Slint / WGPU 30 / Vulkan";
            default-font-size: 12px;
            preferred-width: 1120px; preferred-height: 860px;
            min-width: 960px; min-height: 720px;
            background: #101820;
            in-out property <image> gpu-texture;
            in-out property <image> play-icon;
            in-out property <image> pause-icon;
            in-out property <image> stop-icon;
            in-out property <image> reset-icon;
            in-out property <image> step-frame-icon;
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
            in-out property <float> audio-volume: 70;
            in-out property <bool> audio-muted: false;
            in-out property <string> audio-status: "Aguardando configuração da saída de áudio.";
            in-out property <string> audio-renderer-mode: "psg";
            in-out property <bool> audio-dsp-preset: false;
            in-out property <bool> audio-hpf-enabled: false;
            in-out property <float> audio-hpf-cutoff: 20;
            in-out property <bool> audio-lpf-enabled: false;
            in-out property <float> audio-lpf-cutoff: 18000;
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
            callback audio-volume-changed(float);
            callback audio-mute-changed(bool);
            callback audio-renderer-changed(string);
            callback audio-dsp-changed(bool, bool, float, bool, float);
            callback keyboard-key-pressed(string);
            callback keyboard-key-released(string);
            callback keyboard-focus-lost();
            keyboard-focus := FocusScope {
                width: 100%; height: 100%;
                key-pressed(event) => {
                    if (event.text == Key.UpArrow) { root.keyboard-key-pressed("up"); accept }
                    else if (event.text == Key.DownArrow) { root.keyboard-key-pressed("down"); accept }
                    else if (event.text == Key.LeftArrow) { root.keyboard-key-pressed("left"); accept }
                    else if (event.text == Key.RightArrow) { root.keyboard-key-pressed("right"); accept }
                    else if (event.text == Key.Z) { root.keyboard-key-pressed("button1"); accept }
                    else if (event.text == Key.X) { root.keyboard-key-pressed("button2"); accept }
                    else if (event.text == Key.P) { root.keyboard-key-pressed("nmi"); accept }
                    else if (event.text == Key.F5) { root.keyboard-key-pressed("start"); accept }
                    else if (event.text == Key.F6) { root.keyboard-key-pressed("pause"); accept }
                    else if (event.text == Key.F7) { root.keyboard-key-pressed("stop"); accept }
                    else if (event.text == Key.F8) { root.keyboard-key-pressed("reset"); accept }
                    else if (event.text == Key.F9) { root.keyboard-key-pressed("step-frame"); accept }
                    else { reject }
                }
                key-released(event) => {
                    if (event.text == Key.UpArrow) { root.keyboard-key-released("up"); accept }
                    else if (event.text == Key.DownArrow) { root.keyboard-key-released("down"); accept }
                    else if (event.text == Key.LeftArrow) { root.keyboard-key-released("left"); accept }
                    else if (event.text == Key.RightArrow) { root.keyboard-key-released("right"); accept }
                    else if (event.text == Key.Z) { root.keyboard-key-released("button1"); accept }
                    else if (event.text == Key.X) { root.keyboard-key-released("button2"); accept }
                    else if (event.text == Key.P) { root.keyboard-key-released("nmi"); accept }
                    else if (event.text == Key.F5) { root.keyboard-key-released("start"); accept }
                    else if (event.text == Key.F6) { root.keyboard-key-released("pause"); accept }
                    else if (event.text == Key.F7) { root.keyboard-key-released("stop"); accept }
                    else if (event.text == Key.F8) { root.keyboard-key-released("reset"); accept }
                    else if (event.text == Key.F9) { root.keyboard-key-released("step-frame"); accept }
                    else { reject }
                }
                focus-lost(_reason) => { root.keyboard-focus-lost(); }
                KeyBinding {
                    keys: @keys(Escape);
                    activated => { root.active-menu = "none"; }
                }
                KeyBinding {
                    keys: @keys(F11);
                    activated => {
                        root.fullscreen-enabled = !root.fullscreen-enabled;
                        root.fullscreen-requested(root.fullscreen-enabled);
                    }
                }
                VerticalLayout {
                padding: 12px; spacing: 8px;
                Text { text: "SEGA EMULATOR"; color: #f2f5f7; font-size: 20px; }
                HorizontalLayout {
                    spacing: 3px;
                    z: 95;
                    CompactButton { text: "Sistema"; width: 66px; clicked => { root.keyboard-focus-lost(); root.active-menu = root.active-menu == "system" ? "none" : "system"; } }
                    CompactButton { text: "Arquivo"; width: 62px; clicked => { root.keyboard-focus-lost(); root.active-menu = root.active-menu == "file" ? "none" : "file"; } }
                    CompactButton { text: "Vídeo"; width: 58px; clicked => { root.keyboard-focus-lost(); root.active-menu = root.active-menu == "video" ? "none" : "video"; } }
                    CompactButton { text: "Áudio"; width: 58px; clicked => { root.keyboard-focus-lost(); root.active-menu = root.active-menu == "audio" ? "none" : "audio"; } }
                    CompactButton { text: "Controles"; width: 76px; clicked => { root.keyboard-focus-lost(); root.active-menu = root.active-menu == "controls" ? "none" : "controls"; } }
                    CompactButton { text: "Opções"; width: 66px; clicked => { root.keyboard-focus-lost(); root.active-menu = root.active-menu == "options" ? "none" : "options"; } }
                    CompactButton { text: "DEBUG"; width: 58px; clicked => { root.keyboard-focus-lost(); root.active-menu = root.active-menu == "debug" ? "none" : "debug"; } }
                }
                HorizontalLayout {
                    spacing: 5px;
                    CompactButton { text: ""; icon: root.play-icon; icon-size: 24px; colorize-icon: true; width: 44px; height: 38px; accessible-role: button; accessible-label: "Iniciar emulação"; enabled: root.has-rom; clicked => { root.emulation-paused = false; root.start-requested(); } }
                    CompactButton { text: ""; icon: root.pause-icon; icon-size: 24px; colorize-icon: true; width: 44px; height: 38px; accessible-role: button; accessible-label: "Pausar emulação"; enabled: root.has-rom; clicked => { root.emulation-paused = true; root.pause-requested(); } }
                    CompactButton { text: ""; icon: root.stop-icon; icon-size: 24px; colorize-icon: true; width: 44px; height: 38px; accessible-role: button; accessible-label: "Parar emulação"; enabled: root.has-rom; clicked => { root.emulation-paused = false; root.stop-requested(); } }
                    CompactButton { text: ""; icon: root.reset-icon; icon-size: 24px; colorize-icon: true; width: 44px; height: 38px; accessible-role: button; accessible-label: "Resetar console"; enabled: root.has-rom; clicked => { root.emulation-paused = false; root.reset-requested(); } }
                    CompactButton { text: ""; icon: root.step-frame-icon; icon-size: 24px; colorize-icon: true; width: 44px; height: 38px; accessible-role: button; accessible-label: "Avançar um quadro"; enabled: root.has-rom; clicked => { root.emulation-paused = true; root.pause-requested(); root.step-frame-requested(); } }
                    Text { text: root.rom-status; color: #b9c5ce; font-size: 12px; vertical-alignment: center; }
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
        if root.active-menu != "none": TouchArea {
            x: 0px; y: 74px; width: 100%; height: parent.height - 74px; z: 90;
            clicked => { root.active-menu = "none"; }
        }
        if root.active-menu != "none": Rectangle {
            x: min(max(root.active-menu == "system" ? 12px : root.active-menu == "file" ? 81px : root.active-menu == "video" ? 146px : root.active-menu == "audio" ? 207px : root.active-menu == "controls" ? 268px : root.active-menu == "options" ? 347px : 416px, 8px), parent.width - self.width - 8px);
            y: 88px;
            width: min(parent.width - 16px, root.active-menu == "file" ? 560px : 480px);
            height: min(parent.height - 104px, root.active-menu == "file" ? 480px : root.active-menu == "controls" ? 300px : root.active-menu == "video" ? 230px : root.active-menu == "options" ? 230px : root.active-menu == "system" ? 250px : root.active-menu == "debug" ? 210px : root.active-menu == "audio" ? 480px : 180px);
            z: 100;
            background: #1b2834; border-radius: 5px; border-width: 1px;
            border-color: #405365; min-height: 120px; max-height: 480px; max-width: 560px;
            ScrollView {
                width: 100%; height: 100%;
                content-width: self.width;
                content-height: root.active-menu == "file" ? 160px + root.recent-roms.length * 80px : root.active-menu == "controls" ? 300px : root.active-menu == "system" ? 250px : root.active-menu == "video" ? 230px : root.active-menu == "options" ? 230px : root.active-menu == "debug" ? 210px : root.active-menu == "audio" ? 260px : 180px;
                VerticalLayout {
                    alignment: start;
                    width: 100%;
                    height: root.active-menu == "file" ? 160px + root.recent-roms.length * 80px : root.active-menu == "controls" ? 300px : root.active-menu == "system" ? 250px : root.active-menu == "video" ? 230px : root.active-menu == "options" ? 230px : root.active-menu == "debug" ? 210px : root.active-menu == "audio" ? 260px : 180px;
                    padding: 8px; spacing: 6px;
                        if root.active-menu == "system": VerticalLayout {
                            Text { font-size: 12px; text: "Sistema ativo: Sega Master System"; color: #f2f5f7; }
                            Text { font-size: 12px; text: "Região / padrão de vídeo"; color: #b9c5ce; }
                            FlexboxLayout {
                                flex-direction: row; flex-wrap: wrap; spacing: 4px;
                                CompactButton { text: root.selected-region == "usa" ? "✓ USA / NTSC" : "USA / NTSC"; clicked => { root.selected-region = "usa"; root.preferences-changed(); } }
                                CompactButton { text: root.selected-region == "brazil" ? "✓ Brasil / PAL-M" : "Brasil / PAL-M"; clicked => { root.selected-region = "brazil"; root.preferences-changed(); } }
                                CompactButton { text: root.selected-region == "europe" ? "✓ Europa / PAL" : "Europa / PAL"; clicked => { root.selected-region = "europe"; root.preferences-changed(); } }
                                CompactButton { text: root.selected-region == "japan" ? "✓ Japão / NTSC" : "Japão / NTSC"; clicked => { root.selected-region = "japan"; root.preferences-changed(); } }
                            }
                            Text { font-size: 12px; text: "A região selecionada será aplicada ao carregar a próxima ROM."; color: #b9c5ce; }
                            FlexboxLayout {
                                flex-direction: row; flex-wrap: wrap; spacing: 4px;
                                CompactButton { text: "SG-1000"; enabled: false; }
                                CompactButton { text: "Game Gear"; enabled: false; }
                                CompactButton { text: "Mega Drive / Genesis"; enabled: false; }
                                CompactButton { text: "Sega CD"; enabled: false; }
                                CompactButton { text: "32X"; enabled: false; }
                            }
                            Text { font-size: 12px; text: "Outros sistemas: integração de runtime ainda não concluída."; color: #9baab6; }
                        }
                        if root.active-menu == "file": VerticalLayout {
                            LineEdit { text <=> root.rom-path-input; placeholder-text: "C:\\ROMs\\game.sms"; }
                            HorizontalLayout {
                                CompactButton { text: "Procurar..."; clicked => { root.browse-rom-requested(); } }
                                CompactButton { text: "Carregar ROM"; clicked => { root.load-rom-requested(root.rom-path-input); root.active-menu = "none"; } }
                                CompactButton { text: "Sair"; clicked => { root.exit-requested(); } }
                            }
                            Text { font-size: 12px; text: "Recentes"; color: #b9c5ce; }
                            if root.recent-roms.length == 0: Text { font-size: 12px; text: "Nenhuma ROM recente."; color: #9baab6; }
                            for rom in root.recent-roms: RecentRomEntry {
                                rom-path: rom;
                                activated => { root.load-rom-requested(rom); root.active-menu = "none"; }
                            }
                        }
                        if root.active-menu == "video": VerticalLayout {
                            Text { font-size: 12px; text: "Proporção da imagem"; color: #b9c5ce; }
                            FlexboxLayout {
                                flex-direction: row; flex-wrap: wrap; spacing: 4px;
                                CompactButton { text: root.selected-aspect == "4:3" ? "✓ Original (4:3)" : "Original (4:3)"; clicked => { root.selected-aspect = "4:3"; root.video-width = 768px; root.video-height = 576px; root.preferences-changed(); } }
                                CompactButton { text: root.selected-aspect == "1:1" ? "✓ 1:1" : "1:1"; clicked => { root.selected-aspect = "1:1"; root.video-width = 576px; root.video-height = 576px; root.preferences-changed(); } }
                                CompactButton { text: root.selected-aspect == "16:9" ? "✓ 16:9" : "16:9"; clicked => { root.selected-aspect = "16:9"; root.video-width = 768px; root.video-height = 432px; root.preferences-changed(); } }
                            }
                            CompactButton { text: root.sprite-limit-enabled ? "✓ Limite de sprites ativo" : "Desativar limite de sprites por linha"; clicked => { root.sprite-limit-enabled = !root.sprite-limit-enabled; root.sprite-limit-toggle-requested(root.sprite-limit-enabled); root.preferences-changed(); } }
                            Text { font-size: 12px; text: "O modo sem limite é uma opção de compatibilidade; overflow e temporização do VDP permanecem no hardware emulado."; color: #9baab6; }
                            Text { font-size: 12px; text: "Ajuste aplicado na área de apresentação; o frame original do VDP permanece 256×192."; color: #9baab6; }
                        }
                        if root.active-menu == "audio": VerticalLayout {
                            Text { font-size: 14px; text: "Saída de áudio"; color: #f2f5f7; }
                            Text { font-size: 12px; text: "Backend: CPAL 0.18.2 | dispositivo padrão do Windows"; color: #b9c5ce; }
                            Text { font-size: 12px; text: "Volume mestre: \\{round(root.audio-volume)}%"; color: #f2f5f7; }
                            Slider { minimum: 0; maximum: 100; step: 1; value <=> root.audio-volume; released(value) => { root.audio-volume-changed(value); } }
                            HorizontalLayout {
                                spacing: 5px;
                                CompactButton { text: root.audio-muted ? "Ativar áudio" : "Silenciar"; clicked => { root.audio-muted = !root.audio-muted; root.audio-mute-changed(root.audio-muted); } }
                                CompactButton { text: "Volume 50%"; clicked => { root.audio-volume = 50; root.audio-volume-changed(50); } }
                                CompactButton { text: "Volume 70%"; clicked => { root.audio-volume = 70; root.audio-volume-changed(70); } }
                            }
                            Text { font-size: 14px; text: "Reconstrução digital do PSG"; color: #f2f5f7; }
                            HorizontalLayout {
                                spacing: 5px;
                                CompactButton {
                                    text: root.audio-renderer-mode == "psg" ? "PSG — padrão ✓" : "PSG — padrão";
                                    clicked => {
                                        root.audio-renderer-mode = "psg";
                                        root.audio-renderer-changed("psg");
                                    }
                                }
                                CompactButton {
                                    text: root.audio-renderer-mode == "h2-experimental" ? "H2 — experimental ✓" : "H2 — experimental";
                                    clicked => {
                                        root.audio-renderer-mode = "h2-experimental";
                                        root.audio-renderer-changed("h2-experimental");
                                    }
                                }
                            }
                            Text {
                                font-size: 11px;
                                text: root.audio-renderer-mode == "h2-experimental"
                                    ? "H2 reconstrói transições do mesmo SN76489. Recurso experimental; em caso de falha o worker retorna ao PSG."
                                    : "PSG usa o renderer SN76489 validado e permanece como opção padrão.";
                                color: #d6b879; wrap: word-wrap;
                            }
                            Text { font-size: 14px; text: "Processamento DSP"; color: #f2f5f7; }
                            CheckBox {
                                text: "Preset SMS1 — experimental";
                                checked <=> root.audio-dsp-preset;
                                toggled => {
                                    if (self.checked) {
                                        root.audio-hpf-enabled = true; root.audio-hpf-cutoff = 20;
                                        root.audio-lpf-enabled = true; root.audio-lpf-cutoff = 18000;
                                    } else {
                                        root.audio-hpf-enabled = false; root.audio-lpf-enabled = false;
                                    }
                                    root.audio-dsp-changed(self.checked, root.audio-hpf-enabled, root.audio-hpf-cutoff, root.audio-lpf-enabled, root.audio-lpf-cutoff);
                                }
                            }
                            Text { font-size: 11px; text: "Aproximação ajustável do estágio analógico de saída; não é uma reprodução elétrica calibrada de uma revisão específica da placa."; color: #d6b879; wrap: word-wrap; }
                            CheckBox {
                                text: "Filtro passa-altas / acoplamento AC";
                                checked <=> root.audio-hpf-enabled;
                                toggled => { root.audio-dsp-preset = false; root.audio-dsp-changed(false, self.checked, root.audio-hpf-cutoff, root.audio-lpf-enabled, root.audio-lpf-cutoff); }
                            }
                            Text { font-size: 11px; text: "Corte HPF: \\{round(root.audio-hpf-cutoff)} Hz"; color: #b9c5ce; }
                            Slider {
                                minimum: 5; maximum: 500; step: 5; value <=> root.audio-hpf-cutoff;
                                enabled: root.audio-hpf-enabled;
                                released(value) => { root.audio-dsp-preset = false; root.audio-dsp-changed(false, root.audio-hpf-enabled, value, root.audio-lpf-enabled, root.audio-lpf-cutoff); }
                            }
                            CheckBox {
                                text: "Filtro passa-baixas";
                                checked <=> root.audio-lpf-enabled;
                                toggled => { root.audio-dsp-preset = false; root.audio-dsp-changed(false, root.audio-hpf-enabled, root.audio-hpf-cutoff, self.checked, root.audio-lpf-cutoff); }
                            }
                            Text { font-size: 11px; text: "Corte LPF: \\{round(root.audio-lpf-cutoff)} Hz"; color: #b9c5ce; }
                            Slider {
                                minimum: 4000; maximum: 20000; step: 250; value <=> root.audio-lpf-cutoff;
                                enabled: root.audio-lpf-enabled;
                                released(value) => { root.audio-dsp-preset = false; root.audio-dsp-changed(false, root.audio-hpf-enabled, root.audio-hpf-cutoff, root.audio-lpf-enabled, value); }
                            }
                            CompactButton {
                                text: "Desativar todos os filtros";
                                clicked => { root.audio-dsp-preset = false; root.audio-hpf-enabled = false; root.audio-lpf-enabled = false; root.audio-dsp-changed(false, false, root.audio-hpf-cutoff, false, root.audio-lpf-cutoff); }
                            }
                            Text { font-size: 12px; text: root.audio-status; color: #8bd5ca; wrap: word-wrap; }
                            Text { font-size: 11px; text: "Filtros no worker, após o mixer e antes do reamostrador. Estado contínuo entre blocos; sem alteração no PSG, Scheduler ou clocks."; color: #9baab6; wrap: word-wrap; }
                        }
                        if root.active-menu == "controls": VerticalLayout {
                            Text { font-size: 12px; text: "Teclado — ativo"; color: #8bd5ca; }
                            Text { font-size: 12px; text: "Setas: mover | Z: Botão 1 | X: Botão 2"; color: #f2f5f7; }
                            Text { font-size: 12px; text: "P: solicitar NMI (borda de pressionamento)"; color: #b9c5ce; }
                            Text { font-size: 12px; text: "F5: iniciar | F6: pausar | F7: desligar"; color: #b9c5ce; }
                            Text { font-size: 12px; text: "F8: resetar | F9: avançar quadro | F11: tela cheia"; color: #b9c5ce; }
                            Text { font-size: 12px; text: "Gamepad SDL3 e remapeamento: etapa R4."; color: #9baab6; }
                            CompactButton { text: "Gamepad SDL3 — integração R4"; enabled: false; }
                            CompactButton { text: "Remapear teclado"; enabled: false; }
                            CompactButton { text: "Remapear controle"; enabled: false; }
                        }
                        if root.active-menu == "options": VerticalLayout {
                            Text { font-size: 12px; text: "Overlay de FPS"; color: #f2f5f7; }
                            CompactButton { text: root.show-fps ? "✓ Exibir HUD de FPS" : "Exibir HUD de FPS"; clicked => { root.show-fps = !root.show-fps; root.preferences-changed(); } }
                            CompactButton { text: root.show-status-footer ? "✓ Exibir status no rodapé" : "Exibir status no rodapé"; clicked => { root.show-status-footer = !root.show-status-footer; root.preferences-changed(); } }
                            FlexboxLayout {
                                flex-direction: row; flex-wrap: wrap; spacing: 4px;
                                CompactButton { text: root.fps-size == "small" ? "✓ Pequeno" : "Pequeno"; clicked => { root.fps-size = "small"; root.fps-font-size = 13px; root.preferences-changed(); } }
                                CompactButton { text: root.fps-size == "medium" ? "✓ Médio" : "Médio"; clicked => { root.fps-size = "medium"; root.fps-font-size = 18px; root.preferences-changed(); } }
                                CompactButton { text: root.fps-size == "large" ? "✓ Grande" : "Grande"; clicked => { root.fps-size = "large"; root.fps-font-size = 24px; root.preferences-changed(); } }
                            }
                            Text { font-size: 12px; text: "Posição do HUD: inferior (posição em cantos será concluída após validar o overlay."; color: #9baab6; }
                            CompactButton { text: root.fullscreen-enabled ? "✓ Tela cheia" : "Tela cheia (F11)"; clicked => { root.fullscreen-enabled = !root.fullscreen-enabled; root.fullscreen-requested(root.fullscreen-enabled); } }
                        }
                        if root.active-menu == "debug": VerticalLayout {
                            Text { font-size: 12px; text: "Diagnóstico do runtime SMS"; color: #f2f5f7; }
                            CompactButton { text: "Capturar snapshot do Z80"; enabled: root.has-rom; clicked => { root.debug-snapshot-requested(); } }
                            CompactButton { text: root.trace-enabled ? "Desativar trace" : "Ativar trace"; enabled: root.has-rom; clicked => { root.trace-enabled = !root.trace-enabled; root.trace-toggle-requested(root.trace-enabled); } }
                            Text { font-size: 12px; text: root.debug-status; color: #b9c5ce; }
                            Text { font-size: 12px; text: "Debugger com registradores, breakpoints e memória será migrado em etapa própria; a GUI não executa instruções."; color: #9baab6; }
                        }
                    }
                }
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
        audio_volume: u8,
        audio_muted: bool,
        audio_renderer_mode: String,
        audio_dsp_preset: bool,
        audio_hpf_enabled: bool,
        audio_hpf_cutoff_hz: u16,
        audio_lpf_enabled: bool,
        audio_lpf_cutoff_hz: u16,
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
                audio_volume: 70,
                audio_muted: false,
                audio_renderer_mode: "psg".to_owned(),
                audio_dsp_preset: false,
                audio_hpf_enabled: false,
                audio_hpf_cutoff_hz: 20,
                audio_lpf_enabled: false,
                audio_lpf_cutoff_hz: 18_000,
            }
        }
    }

    /// Resolve o diretório de dados por usuário no Windows sem criar diretórios durante leitura.
    fn app_data_directory() -> Option<PathBuf> {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join(APP_DATA_DIRECTORY))
    }

    /// Cria o armazenamento SRM dentro do diretório de dados do usuário.
    fn cartridge_save_store() -> Result<Arc<SaveRamStore>, String> {
        let root = app_data_directory()
            .ok_or_else(|| "APPDATA não está definido para armazenar SRAM".to_owned())?
            .join("saves");
        Ok(Arc::new(SaveRamStore::new(root)))
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
                "audio_volume" => {
                    if let Ok(value) = value.trim().parse::<u8>()
                        && value <= 100
                    {
                        preferences.audio_volume = value;
                    }
                }
                "audio_muted" => {
                    if let Ok(value) = value.trim().parse() {
                        preferences.audio_muted = value;
                    }
                }
                "audio_renderer_mode" if ["psg", "h2-experimental"].contains(&value.trim()) => {
                    preferences.audio_renderer_mode = value.trim().to_owned();
                }
                "audio_dsp_preset" => {
                    if let Ok(value) = value.trim().parse() {
                        preferences.audio_dsp_preset = value;
                    }
                }
                "audio_hpf_enabled" => {
                    if let Ok(value) = value.trim().parse() {
                        preferences.audio_hpf_enabled = value;
                    }
                }
                "audio_hpf_cutoff_hz" => {
                    if let Ok(value) = value.trim().parse::<u16>()
                        && (5..=500).contains(&value)
                    {
                        preferences.audio_hpf_cutoff_hz = value;
                    }
                }
                "audio_lpf_enabled" => {
                    if let Ok(value) = value.trim().parse() {
                        preferences.audio_lpf_enabled = value;
                    }
                }
                "audio_lpf_cutoff_hz" => {
                    if let Ok(value) = value.trim().parse::<u16>()
                        && (4_000..=20_000).contains(&value)
                    {
                        preferences.audio_lpf_cutoff_hz = value;
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
            "region={}\naspect={}\nshow_fps={}\nshow_status_footer={}\nfps_size={}\nsprite_limit_enabled={}\naudio_volume={}\naudio_muted={}\naudio_renderer_mode={}\naudio_dsp_preset={}\naudio_hpf_enabled={}\naudio_hpf_cutoff_hz={}\naudio_lpf_enabled={}\naudio_lpf_cutoff_hz={}\n",
            preferences.region,
            preferences.aspect,
            preferences.show_fps,
            preferences.show_status_footer,
            preferences.fps_size,
            preferences.sprite_limit_enabled,
            preferences.audio_volume,
            preferences.audio_muted,
            preferences.audio_renderer_mode,
            preferences.audio_dsp_preset,
            preferences.audio_hpf_enabled,
            preferences.audio_hpf_cutoff_hz,
            preferences.audio_lpf_enabled,
            preferences.audio_lpf_cutoff_hz
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
        queue_slot: Arc<Mutex<Option<wgpu::Queue>>>,
        mailbox: FrameMailbox,
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
                            *queue_slot.lock().unwrap() = Some(queue.clone());

                            let app_weak = app_weak.clone();
                            let mailbox = mailbox.clone();
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
                                    if let Err(error) = mailbox.notify_if_pending() {
                                        eprintln!(
                                            "R2_FRAME_NOTIFY renderer_recovery_failed error={error}"
                                        );
                                    }
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

                    *queue_slot.lock().unwrap() = None;

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
                }                _ => {}
            }
        })?;

        Ok(())
    }

    const HOST_PACING_WINDOW_CAPACITY: usize = 512;

    /// Fixed-capacity rolling distribution. Samples are appended without allocation;
    /// sorting happens only when the one-second host telemetry record is emitted.
    #[derive(Clone)]
    struct HostDurationWindow {
        values: [u64; HOST_PACING_WINDOW_CAPACITY],
        len: usize,
        next: usize,
    }

    impl Default for HostDurationWindow {
        fn default() -> Self {
            Self {
                values: [0; HOST_PACING_WINDOW_CAPACITY],
                len: 0,
                next: 0,
            }
        }
    }

    impl HostDurationWindow {
        fn push(&mut self, value_ns: u64) {
            self.values[self.next] = value_ns;
            self.next = (self.next + 1) % HOST_PACING_WINDOW_CAPACITY;
            self.len = (self.len + 1).min(HOST_PACING_WINDOW_CAPACITY);
        }

        fn percentile_ns(&self, percentile: usize) -> u64 {
            if self.len == 0 {
                return 0;
            }
            let mut sorted = self.values;
            sorted[..self.len].sort_unstable();
            let rank = (self.len * percentile).div_ceil(100).max(1) - 1;
            sorted[rank.min(self.len - 1)]
        }

        const fn len(&self) -> usize {
            self.len
        }
    }

    /// Bounded host-side measurements for mailbox consumption and WGPU upload.
    ///
    /// This deliberately stops at queue.write_texture: Slint owns the Winit surface
    /// and physical present operation, so this host cannot claim a GPU-present timestamp.
    struct HostFramePacingWindow {
        started: Instant,
        qpc_clock: Option<sega_video_runtime::diagnostics::QpcClock>,
        last_wakeup: Option<Instant>,
        last_consumed: Option<Instant>,
        last_sequence: u64,
        last_fps_consumed: u64,
        last_fps_sample: Instant,
        wakeups: u64,
        empty_wakeups: u64,
        consumed: u64,
        sequence_gaps: u64,
        wake_interval_sum_ns: u128,
        wake_interval_max_ns: u64,
        consumed_interval_sum_ns: u128,
        consumed_interval_max_ns: u64,
        mailbox_residence_count: u64,
        mailbox_residence_sum_ns: u128,
        mailbox_residence_max_ns: u64,
        conversion_count: u64,
        conversion_sum_ns: u128,
        conversion_max_ns: u64,
        queue_write_count: u64,
        queue_write_sum_ns: u128,
        queue_write_max_ns: u64,
        callback_count: u64,
        callback_sum_ns: u128,
        callback_max_ns: u64,
        mailbox_take_count: u64,
        mailbox_take_sum_ns: u128,
        mailbox_take_max_ns: u64,
        wake_interval_distribution: HostDurationWindow,
        consumed_interval_distribution: HostDurationWindow,
        mailbox_residence_distribution: HostDurationWindow,
        conversion_distribution: HostDurationWindow,
        queue_write_distribution: HostDurationWindow,
        callback_distribution: HostDurationWindow,
        mailbox_take_distribution: HostDurationWindow,
    }

    impl HostFramePacingWindow {
        fn new() -> Self {
            Self {
                started: Instant::now(),
                qpc_clock: sega_video_runtime::diagnostics::QpcClock::new(),
                last_wakeup: None,
                last_consumed: None,
                last_sequence: 0,
                last_fps_consumed: 0,
                last_fps_sample: Instant::now(),
                wakeups: 0,
                empty_wakeups: 0,
                consumed: 0,
                sequence_gaps: 0,
                wake_interval_sum_ns: 0,
                wake_interval_max_ns: 0,
                consumed_interval_sum_ns: 0,
                consumed_interval_max_ns: 0,
                mailbox_residence_count: 0,
                mailbox_residence_sum_ns: 0,
                mailbox_residence_max_ns: 0,
                conversion_count: 0,
                conversion_sum_ns: 0,
                conversion_max_ns: 0,
                queue_write_count: 0,
                queue_write_sum_ns: 0,
                queue_write_max_ns: 0,
                callback_count: 0,
                callback_sum_ns: 0,
                callback_max_ns: 0,
                mailbox_take_count: 0,
                mailbox_take_sum_ns: 0,
                mailbox_take_max_ns: 0,
                wake_interval_distribution: HostDurationWindow::default(),
                consumed_interval_distribution: HostDurationWindow::default(),
                mailbox_residence_distribution: HostDurationWindow::default(),
                conversion_distribution: HostDurationWindow::default(),
                queue_write_distribution: HostDurationWindow::default(),
                callback_distribution: HostDurationWindow::default(),
                mailbox_take_distribution: HostDurationWindow::default(),
            }
        }

        fn record_wakeup(&mut self, now: Instant) {
            self.wakeups = self.wakeups.saturating_add(1);
            if let Some(previous) = self.last_wakeup {
                let elapsed = now
                    .saturating_duration_since(previous)
                    .as_nanos()
                    .min(u128::from(u64::MAX)) as u64;
                self.wake_interval_sum_ns = self
                    .wake_interval_sum_ns
                    .saturating_add(u128::from(elapsed));
                self.wake_interval_max_ns = self.wake_interval_max_ns.max(elapsed);
                self.wake_interval_distribution.push(elapsed);
            }
            self.last_wakeup = Some(now);
        }

        fn record_empty_wakeup(&mut self) {
            self.empty_wakeups = self.empty_wakeups.saturating_add(1);
        }

        fn record_frame(
            &mut self,
            now: Instant,
            sequence: u64,
            published_at: Instant,
        ) -> Option<f64> {
            self.consumed = self.consumed.saturating_add(1);
            if let Some(previous) = self.last_consumed {
                let elapsed = now
                    .saturating_duration_since(previous)
                    .as_nanos()
                    .min(u128::from(u64::MAX)) as u64;
                self.consumed_interval_sum_ns = self
                    .consumed_interval_sum_ns
                    .saturating_add(u128::from(elapsed));
                self.consumed_interval_max_ns = self.consumed_interval_max_ns.max(elapsed);
                self.consumed_interval_distribution.push(elapsed);
            }
            if self.last_sequence > 0 && sequence > self.last_sequence.saturating_add(1) {
                self.sequence_gaps = self
                    .sequence_gaps
                    .saturating_add(sequence - self.last_sequence - 1);
            }
            let residence = now
                .saturating_duration_since(published_at)
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64;
            self.mailbox_residence_count = self.mailbox_residence_count.saturating_add(1);
            self.mailbox_residence_sum_ns = self
                .mailbox_residence_sum_ns
                .saturating_add(u128::from(residence));
            self.mailbox_residence_max_ns = self.mailbox_residence_max_ns.max(residence);
            self.mailbox_residence_distribution.push(residence);
            self.last_sequence = sequence;
            self.last_consumed = Some(now);

            let fps_elapsed = now.saturating_duration_since(self.last_fps_sample);
            if fps_elapsed >= Duration::from_millis(500) {
                let consumed = self.consumed.saturating_sub(self.last_fps_consumed);
                self.last_fps_consumed = self.consumed;
                self.last_fps_sample = now;
                Some(consumed as f64 / fps_elapsed.as_secs_f64())
            } else {
                None
            }
        }

        fn record_conversion(&mut self, elapsed: Duration) {
            self.conversion_count = self.conversion_count.saturating_add(1);
            let nanos = elapsed.as_nanos().min(u128::from(u64::MAX)) as u64;
            self.conversion_sum_ns = self.conversion_sum_ns.saturating_add(u128::from(nanos));
            self.conversion_max_ns = self.conversion_max_ns.max(nanos);
            self.conversion_distribution.push(nanos);
        }

        fn record_queue_write(&mut self, elapsed: Duration) {
            self.queue_write_count = self.queue_write_count.saturating_add(1);
            let nanos = elapsed.as_nanos().min(u128::from(u64::MAX)) as u64;
            self.queue_write_sum_ns = self.queue_write_sum_ns.saturating_add(u128::from(nanos));
            self.queue_write_max_ns = self.queue_write_max_ns.max(nanos);
            self.queue_write_distribution.push(nanos);
        }

        fn record_callback(&mut self, elapsed: Duration) {
            self.callback_count = self.callback_count.saturating_add(1);
            let nanos = elapsed.as_nanos().min(u128::from(u64::MAX)) as u64;
            self.callback_sum_ns = self.callback_sum_ns.saturating_add(u128::from(nanos));
            self.callback_max_ns = self.callback_max_ns.max(nanos);
            self.callback_distribution.push(nanos);
        }

        fn record_mailbox_take(&mut self, elapsed: Duration) {
            self.mailbox_take_count = self.mailbox_take_count.saturating_add(1);
            let nanos = elapsed.as_nanos().min(u128::from(u64::MAX)) as u64;
            self.mailbox_take_sum_ns = self.mailbox_take_sum_ns.saturating_add(u128::from(nanos));
            self.mailbox_take_max_ns = self.mailbox_take_max_ns.max(nanos);
            self.mailbox_take_distribution.push(nanos);
        }

        fn report_if_due(&mut self, worker_slot: &Rc<RefCell<Option<SmsTemporalWorker>>>) {
            let elapsed = self.started.elapsed();
            if elapsed < Duration::from_secs(1) {
                return;
            }

            let (runtime, replaced_frames, machine_clock_hz) = {
                let guard = worker_slot.borrow();
                guard.as_ref().map_or(
                    (sega_sms_runtime::SmsFramePacingMetrics::default(), 0, 0),
                    |worker| {
                        (
                            worker.frame_pacing_metrics(),
                            worker.video_replaced_frames(),
                            worker.machine_clock_hz(),
                        )
                    },
                )
            };
            let audio_output = {
                let guard = worker_slot.borrow();
                guard
                    .as_ref()
                    .map(|worker| (worker.audio_output_metrics(), worker.audio_output_error()))
            };
            // QPC shares the timestamp domain used by PresentMon --qpc_time.
            // This marker is diagnostic only and never participates in frame pacing.
            let (qpc_ticks, qpc_frequency, qpc_available) = self
                .qpc_clock
                .as_ref()
                .and_then(|clock| clock.now().map(|ticks| (ticks, clock.frequency())))
                .map_or((0, 0, false), |(ticks, frequency)| (ticks, frequency, true));
            let average_us = |total_ns: u128, count: u64| -> u64 {
                if count == 0 {
                    0
                } else {
                    (total_ns / u128::from(count) / 1_000).min(u128::from(u64::MAX)) as u64
                }
            };
            eprintln!(
                "R2_FRAME_PACING_HOST qpc_available={} qpc_ticks={} qpc_frequency={} window_ms={} wakeups={} empty_wakeups={} consumed={} sequence_gaps={} wake_interval_avg_us={} wake_interval_max_us={} wake_interval_samples={} wake_interval_p50_us={} wake_interval_p95_us={} wake_interval_p99_us={} wake_interval_jitter_p95_us={} consumed_interval_avg_us={} consumed_interval_max_us={} consumed_interval_samples={} consumed_interval_p50_us={} consumed_interval_p95_us={} consumed_interval_p99_us={} consumed_interval_jitter_p95_us={} mailbox_residence_avg_us={} mailbox_residence_max_us={} mailbox_residence_samples={} mailbox_residence_p50_us={} mailbox_residence_p95_us={} mailbox_residence_p99_us={} convert_avg_us={} convert_max_us={} convert_samples={} convert_p50_us={} convert_p95_us={} convert_p99_us={} queue_write_avg_us={} queue_write_max_us={} queue_write_samples={} queue_write_p50_us={} queue_write_p95_us={} queue_write_p99_us={} callback_avg_us={} callback_max_us={} callback_samples={} callback_p50_us={} callback_p95_us={} callback_p99_us={} mailbox_take_avg_us={} mailbox_take_max_us={} mailbox_take_samples={} mailbox_take_p50_us={} mailbox_take_p95_us={} mailbox_take_p99_us={} mailbox_replaced_total={} worker_samples={} worker_total_samples={} worker_frame_interval_p50_us={} worker_frame_interval_p95_us={} worker_frame_interval_p99_us={} worker_frame_interval_jitter_p95_us={} machine_ticks_per_frame_min={} machine_ticks_per_frame_p50={} machine_ticks_per_frame_max={} worker_step_p50_us={} worker_step_p95_us={} worker_step_p99_us={} video_publish_p95_us={} audio_submit_p95_us={} frame_processing_p50_us={} frame_processing_p95_us={} frame_processing_p99_us={} pacing_wait_budget_p50_us={} pacing_wait_budget_p95_us={} pacing_wait_budget_p99_us={} pacing_wait_elapsed_p50_us={} pacing_wait_elapsed_p95_us={} pacing_wait_elapsed_p99_us={} frame_budget_p50_us={} frame_budget_p95_us={} frame_budget_p99_us={} psg_samples_p50={} psg_samples_p95={} psg_samples_p99={} pacing_timeout_lateness_p50_us={} pacing_timeout_lateness_p95_us={} pacing_timeout_lateness_p99_us={} pacing_timeout_count_total={} pacing_command_wakeups_total={} pacing_command_remaining_p95_us={} command_handler_p95_us={} machine_clock_hz={} physical_present_timestamp=unavailable_slint_owned",
                u8::from(qpc_available),
                qpc_ticks,
                qpc_frequency,
                elapsed.as_millis(),
                self.wakeups,
                self.empty_wakeups,
                self.consumed,
                self.sequence_gaps,
                average_us(self.wake_interval_sum_ns, self.wakeups.saturating_sub(1)),
                self.wake_interval_max_ns / 1_000,
                self.wake_interval_distribution.len(),
                self.wake_interval_distribution.percentile_ns(50) / 1_000,
                self.wake_interval_distribution.percentile_ns(95) / 1_000,
                self.wake_interval_distribution.percentile_ns(99) / 1_000,
                self.wake_interval_distribution
                    .percentile_ns(95)
                    .saturating_sub(self.wake_interval_distribution.percentile_ns(50))
                    / 1_000,
                average_us(
                    self.consumed_interval_sum_ns,
                    self.consumed.saturating_sub(1)
                ),
                self.consumed_interval_max_ns / 1_000,
                self.consumed_interval_distribution.len(),
                self.consumed_interval_distribution.percentile_ns(50) / 1_000,
                self.consumed_interval_distribution.percentile_ns(95) / 1_000,
                self.consumed_interval_distribution.percentile_ns(99) / 1_000,
                self.consumed_interval_distribution
                    .percentile_ns(95)
                    .saturating_sub(self.consumed_interval_distribution.percentile_ns(50))
                    / 1_000,
                average_us(self.mailbox_residence_sum_ns, self.mailbox_residence_count),
                self.mailbox_residence_max_ns / 1_000,
                self.mailbox_residence_distribution.len(),
                self.mailbox_residence_distribution.percentile_ns(50) / 1_000,
                self.mailbox_residence_distribution.percentile_ns(95) / 1_000,
                self.mailbox_residence_distribution.percentile_ns(99) / 1_000,
                average_us(self.conversion_sum_ns, self.conversion_count),
                self.conversion_max_ns / 1_000,
                self.conversion_distribution.len(),
                self.conversion_distribution.percentile_ns(50) / 1_000,
                self.conversion_distribution.percentile_ns(95) / 1_000,
                self.conversion_distribution.percentile_ns(99) / 1_000,
                average_us(self.queue_write_sum_ns, self.queue_write_count),
                self.queue_write_max_ns / 1_000,
                self.queue_write_distribution.len(),
                self.queue_write_distribution.percentile_ns(50) / 1_000,
                self.queue_write_distribution.percentile_ns(95) / 1_000,
                self.queue_write_distribution.percentile_ns(99) / 1_000,
                average_us(self.callback_sum_ns, self.callback_count),
                self.callback_max_ns / 1_000,
                self.callback_distribution.len(),
                self.callback_distribution.percentile_ns(50) / 1_000,
                self.callback_distribution.percentile_ns(95) / 1_000,
                self.callback_distribution.percentile_ns(99) / 1_000,
                average_us(self.mailbox_take_sum_ns, self.mailbox_take_count),
                self.mailbox_take_max_ns / 1_000,
                self.mailbox_take_distribution.len(),
                self.mailbox_take_distribution.percentile_ns(50) / 1_000,
                self.mailbox_take_distribution.percentile_ns(95) / 1_000,
                self.mailbox_take_distribution.percentile_ns(99) / 1_000,
                replaced_frames,
                runtime.sample_count,
                runtime.total_samples,
                runtime.frame_interval_p50_ns / 1_000,
                runtime.frame_interval_p95_ns / 1_000,
                runtime.frame_interval_p99_ns / 1_000,
                runtime
                    .frame_interval_p95_ns
                    .saturating_sub(runtime.frame_interval_p50_ns)
                    / 1_000,
                runtime.machine_ticks_per_frame_min,
                runtime.machine_ticks_per_frame_p50,
                runtime.machine_ticks_per_frame_max,
                runtime.step_frame_p50_ns / 1_000,
                runtime.step_frame_p95_ns / 1_000,
                runtime.step_frame_p99_ns / 1_000,
                runtime.video_publish_p95_ns / 1_000,
                runtime.audio_submit_p95_ns / 1_000,
                runtime.frame_processing_p50_ns / 1_000,
                runtime.frame_processing_p95_ns / 1_000,
                runtime.frame_processing_p99_ns / 1_000,
                runtime.pacing_wait_budget_p50_ns / 1_000,
                runtime.pacing_wait_budget_p95_ns / 1_000,
                runtime.pacing_wait_budget_p99_ns / 1_000,
                runtime.pacing_wait_elapsed_p50_ns / 1_000,
                runtime.pacing_wait_elapsed_p95_ns / 1_000,
                runtime.pacing_wait_elapsed_p99_ns / 1_000,
                runtime.frame_budget_p50_ns / 1_000,
                runtime.frame_budget_p95_ns / 1_000,
                runtime.frame_budget_p99_ns / 1_000,
                runtime.audio_samples_p50,
                runtime.audio_samples_p95,
                runtime.audio_samples_p99,
                runtime.pacing_timeout_lateness_p50_ns / 1_000,
                runtime.pacing_timeout_lateness_p95_ns / 1_000,
                runtime.pacing_timeout_lateness_p99_ns / 1_000,
                runtime.pacing_timeout_count_total,
                runtime.pacing_command_wakeups_total,
                runtime.pacing_command_remaining_p95_ns / 1_000,
                runtime.command_handler_p95_ns / 1_000,
                machine_clock_hz,
            );

            if let Some((audio, output_error)) = audio_output {
                eprintln!(
                    "R3_AUDIO_HEALTH configured={} sample_rate_hz={} queue_capacity_frames={} queue_queued_frames={} queue_high_water_frames={} accepted_frames={} consumed_frames={} dropped_frames={} dropped_blocks={} underrun_frames={} underrun_events={} flushed_frames={} output_error={:?}",
                    u8::from(audio.sample_rate_hz > 0),
                    audio.sample_rate_hz,
                    audio.queue_capacity_frames,
                    audio.queue_queued_frames,
                    audio.queue_high_water_frames,
                    audio.accepted_frames,
                    audio.consumed_frames,
                    audio.dropped_frames,
                    audio.dropped_blocks,
                    audio.underrun_frames,
                    audio.underrun_events,
                    audio.flushed_frames,
                    output_error,
                );
            }

            self.started = Instant::now();
            self.last_wakeup = None;
            self.last_consumed = None;
            self.wakeups = 0;
            self.empty_wakeups = 0;
            self.consumed = 0;
            self.sequence_gaps = 0;
            self.wake_interval_sum_ns = 0;
            self.wake_interval_max_ns = 0;
            self.consumed_interval_sum_ns = 0;
            self.consumed_interval_max_ns = 0;
            self.mailbox_residence_count = 0;
            self.mailbox_residence_sum_ns = 0;
            self.mailbox_residence_max_ns = 0;
            self.conversion_count = 0;
            self.conversion_sum_ns = 0;
            self.conversion_max_ns = 0;
            self.queue_write_count = 0;
            self.queue_write_sum_ns = 0;
            self.queue_write_max_ns = 0;
            self.callback_count = 0;
            self.callback_sum_ns = 0;
            self.callback_max_ns = 0;
            self.mailbox_take_count = 0;
            self.mailbox_take_sum_ns = 0;
            self.mailbox_take_max_ns = 0;
        }
    }

    /// Records the complete UI-thread callback duration, including early-return paths.
    struct HostFrameCallbackGuard {
        host_pacing: Arc<Mutex<HostFramePacingWindow>>,
        started: Instant,
    }

    impl Drop for HostFrameCallbackGuard {
        fn drop(&mut self) {
            self.host_pacing
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .record_callback(self.started.elapsed());
        }
    }

    /// Consume and upload one latest frame after the mailbox signals publication.
    fn present_latest_frame(
        app_weak: &slint::Weak<MainWindow>,
        queue_slot: &Arc<Mutex<Option<wgpu::Queue>>>,
        mailbox: &FrameMailbox,
        host_pacing: &Arc<Mutex<HostFramePacingWindow>>,
    ) {
        let callback_started = Instant::now();
        let _callback_guard = HostFrameCallbackGuard {
            host_pacing: Arc::clone(host_pacing),
            started: callback_started,
        };
        host_pacing
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .record_wakeup(callback_started);

        let Some(app) = app_weak.upgrade() else {
            return;
        };
        let queue = queue_slot
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().cloned());
        let Some(queue) = queue else {
            return;
        };
        let Some(texture) = app.get_gpu_texture().to_wgpu_30_texture() else {
            return;
        };

        let mailbox_take_started = Instant::now();
        let take_result = mailbox.take_latest_with_timestamp();
        host_pacing
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .record_mailbox_take(mailbox_take_started.elapsed());
        let (frame, published_at) = match take_result {
            Ok(Some(frame)) => frame,
            Ok(None) => {
                host_pacing
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .record_empty_wakeup();
                return;
            }
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
        let fps = host_pacing
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .record_frame(now, frame.sequence(), published_at);
        if let Some(fps) = fps {
            app.set_fps_status(format!("FPS VDP: {fps:.1}").into());
        }

        let conversion_started = Instant::now();
        match convert_rgb888_to_rgba8(&frame) {
            Ok(pixels) => {
                host_pacing
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .record_conversion(conversion_started.elapsed());
                let queue_write_started = Instant::now();
                queue.write_texture(
                    texture.as_image_copy(),
                    &pixels,
                    texture_data_layout(),
                    texture_extent(),
                );
                host_pacing
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .record_queue_write(queue_write_started.elapsed());
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
                host_pacing
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .record_conversion(conversion_started.elapsed());
                app.set_frame_status(format!("Falha ao converter frame: {error}").into());
            }
        }
    }

    /// Wake Slint only when a new frame is published; the atomic gate bounds queued callbacks.
    fn start_frame_presentation(
        app: &MainWindow,
        queue_slot: Arc<Mutex<Option<wgpu::Queue>>>,
        mailbox: FrameMailbox,
        host_pacing: Arc<Mutex<HostFramePacingWindow>>,
    ) -> Result<(), sega_video_runtime::FrameMailboxError> {
        let app_weak = app.as_weak();
        let notification_pending = Arc::new(AtomicBool::new(false));
        let notifier: Arc<dyn Fn(FrameMailbox) + Send + Sync> =
            Arc::new(move |published_mailbox| {
                if notification_pending.swap(true, Ordering::AcqRel) {
                    return;
                }

                let callback_pending = Arc::clone(&notification_pending);
                let callback_app = app_weak.clone();
                let callback_queue = Arc::clone(&queue_slot);
                let callback_mailbox = published_mailbox;
                let callback_metrics = Arc::clone(&host_pacing);
                let result = slint::invoke_from_event_loop(move || {
                    // Clear before taking the frame. A concurrent publication either
                    // becomes the frame consumed below or queues one harmless follow-up.
                    callback_pending.store(false, Ordering::Release);
                    present_latest_frame(
                        &callback_app,
                        &callback_queue,
                        &callback_mailbox,
                        &callback_metrics,
                    );
                });
                if let Err(error) = result {
                    notification_pending.store(false, Ordering::Release);
                    eprintln!("R2_FRAME_NOTIFY enqueue_failed error={error}");
                }
            });

        mailbox.set_frame_available_notifier(Some(Arc::clone(&notifier)))?;
        mailbox.notify_if_pending()?;
        Ok(())
    }

    /// Keeps runtime error handling and telemetry reporting off the frame delivery path.
    fn start_frame_diagnostics(
        app: &MainWindow,
        host_pacing: Arc<Mutex<HostFramePacingWindow>>,
        worker_slot: Rc<RefCell<Option<SmsTemporalWorker>>>,
    ) -> Timer {
        let timer = Timer::default();
        let app_weak = app.as_weak();
        timer.start(TimerMode::Repeated, DIAGNOSTICS_INTERVAL, move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            let renderer_fallback = worker_slot
                .borrow()
                .as_ref()
                .and_then(|worker| worker.runtime_error())
                .filter(|error| error.starts_with("H2 renderer"));
            if let Some(error) = renderer_fallback
                && app.get_audio_renderer_mode().as_str() != "psg"
            {
                app.set_audio_renderer_mode("psg".into());
                app.set_audio_status(error.into());
                app.invoke_preferences_changed();
            }
            host_pacing
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .report_if_due(&worker_slot);
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
            $dialog.Filter = 'ROM Sega 8-bit (*.sms;*.bin;*.zip)|*.sms;*.bin;*.zip|Todos os arquivos (*.*)|*.*'
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

    /// Read a raw SMS ROM or a ZIP containing exactly one supported ROM entry.
    ///
    /// ZIP data is decompressed in memory only. No archive path is ever written to disk,
    /// and entry count, archive size and uncompressed ROM size are bounded.
    fn read_rom_image(path: &PathBuf) -> Result<Vec<u8>, String> {
        let is_zip = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"));
        let max_size = if is_zip {
            MAX_ROM_ARCHIVE_BYTES as u64
        } else {
            MAX_ROM_BYTES
        };
        let metadata = fs::metadata(path)
            .map_err(|error| format!("não foi possível consultar o arquivo ROM: {error}"))?;
        if metadata.len() > max_size {
            return Err(format!("arquivo ROM excede o limite de {max_size} bytes"));
        }

        let bytes =
            fs::read(path).map_err(|error| format!("não foi possível ler a ROM: {error}"))?;
        if is_zip {
            extract_rom_from_zip(&bytes)
        } else {
            Ok(bytes)
        }
    }

    /// Extract a single .sms/.bin entry without writing archive-controlled paths.
    fn extract_rom_from_zip(bytes: &[u8]) -> Result<Vec<u8>, String> {
        if bytes.len() > MAX_ROM_ARCHIVE_BYTES {
            return Err(format!(
                "arquivo ZIP excede o limite de {MAX_ROM_ARCHIVE_BYTES} bytes"
            ));
        }

        let mut archive = ZipArchive::new(Cursor::new(bytes))
            .map_err(|error| format!("arquivo ZIP inválido: {error}"))?;
        if archive.len() > MAX_ZIP_ENTRIES {
            return Err(format!(
                "arquivo ZIP contém mais de {MAX_ZIP_ENTRIES} entradas"
            ));
        }

        let mut candidates = Vec::new();
        for index in 0..archive.len() {
            let entry = archive
                .by_index(index)
                .map_err(|error| format!("não foi possível inspecionar entrada ZIP: {error}"))?;
            if entry.is_dir() {
                continue;
            }
            let name = entry
                .name()
                .map_err(|error| format!("não foi possível ler o nome da entrada ZIP: {error}"))?
                .into_owned();
            let is_rom = std::path::Path::new(&name)
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("sms") || extension.eq_ignore_ascii_case("bin")
                });
            if is_rom {
                candidates.push((index, name, entry.size()));
            }
        }

        if candidates.is_empty() {
            return Err("ZIP não contém uma ROM .sms ou .bin.".to_owned());
        }
        if candidates.len() != 1 {
            return Err(
                "ZIP contém múltiplas ROMs; mantenha somente uma entrada .sms ou .bin.".to_owned(),
            );
        }

        let (index, name, expected_size) = candidates.pop().expect("one candidate was checked");
        if expected_size == 0 || expected_size > MAX_ROM_BYTES {
            return Err(format!(
                "ROM compactada '{name}' tem tamanho inválido ou excede {MAX_ROM_BYTES} bytes"
            ));
        }

        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("não foi possível abrir ROM '{name}': {error}"))?;
        let mut rom = Vec::with_capacity(expected_size as usize);
        entry
            .by_ref()
            .take(MAX_ROM_BYTES + 1)
            .read_to_end(&mut rom)
            .map_err(|error| format!("falha ao descompactar ROM '{name}': {error}"))?;
        if rom.len() as u64 != expected_size {
            return Err(format!(
                "tamanho descompactado inconsistente para ROM '{name}'"
            ));
        }
        Ok(rom)
    }

    /// Returns whether physical audio must be bypassed for a video-only diagnostic capture.
    fn video_only_capture_requested(value: Option<&str>) -> bool {
        value.is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    }

    fn video_only_capture_enabled() -> bool {
        video_only_capture_requested(std::env::var("SEGA_EMULATOR_VIDEO_ONLY").ok().as_deref())
    }

    /// Converte volume mestre e mute no nível efetivo enviado ao DSP de saída.
    fn effective_audio_volume(volume: u8, muted: bool) -> u8 {
        if muted { 0 } else { volume.min(100) }
    }

    /// Converte as preferências da GUI em configuração DSP validada pelo runtime.
    fn audio_dsp_config(
        preset: bool,
        hpf_enabled: bool,
        hpf_cutoff: f64,
        lpf_enabled: bool,
        lpf_cutoff: f64,
    ) -> AnalogOutputConfig {
        if preset {
            return AnalogOutputConfig::sms1_experimental();
        }
        AnalogOutputConfig {
            enabled: hpf_enabled || lpf_enabled,
            high_pass_hz: hpf_enabled.then_some(hpf_cutoff),
            low_pass_hz: lpf_enabled.then_some(lpf_cutoff),
        }
    }

    /// Cria a sessão CPAL com volume e configuração DSP persistidos.
    fn configure_sms_audio(
        worker: &SmsTemporalWorker,
        volume: u8,
        dsp_config: AnalogOutputConfig,
        renderer_mode: &str,
    ) {
        let renderer = if renderer_mode == "h2-experimental" {
            AudioRendererMode::H2Experimental
        } else {
            AudioRendererMode::Psg
        };
        if let Err(error) = worker.set_audio_renderer(renderer) {
            eprintln!("R3_AUDIO renderer selection failed; PSG remains the fallback: {error}");
        }
        if video_only_capture_enabled() {
            eprintln!(
                "R3_AUDIO skipped: SEGA_EMULATOR_VIDEO_ONLY enabled; digital PSG generation remains active"
            );
            return;
        }
        const BUFFER_FRAMES: u32 = 512;
        const QUEUE_BLOCKS: usize = 8;
        let result = worker.configure_audio_output(
            move || -> Result<Box<dyn sega_sms_runtime::AudioSession>, String> {
                let sink = sega_audio_output::NativeCpalOutput::new(BUFFER_FRAMES, QUEUE_BLOCKS)?;
                eprintln!(
                    "R3_AUDIO CPAL output opened: sample_rate={} Hz, channels={}, format={:?}, default_config_fallback={}, queue_capacity={} frames",
                    sink.sample_rate_hz(),
                    sink.channels(),
                    sink.sample_format(),
                    sink.used_default_config_fallback(),
                    BUFFER_FRAMES as usize * QUEUE_BLOCKS
                );
                let mut session = sink.into_audio_session(volume)?;
                session.set_analog_output_config(dsp_config)?;
                Ok(Box::new(session))
            },
        );
        if let Err(error) = result {
            eprintln!("R3_AUDIO CPAL unavailable; continuing without physical audio: {error}");
        }
    }

    const DIAGNOSTIC_SRAM_CAPACITY_ENV: &str = "SEGA_SMS_DIAGNOSTIC_SRAM_BYTES";

    /// Analisa a capacidade SRAM volátil solicitada explicitamente para diagnóstico.
    /// A opção nunca cria contexto de persistência nem altera perfis do catálogo.
    fn parse_diagnostic_sram_capacity(value: Option<&str>) -> Result<Option<usize>, String> {
        let Some(value) = value else {
            return Ok(None);
        };
        let capacity = value.trim().parse::<usize>().map_err(|_| {
            format!("{DIAGNOSTIC_SRAM_CAPACITY_ENV} deve ser 8192, 16384 ou 32768 bytes")
        })?;
        if !matches!(capacity, 0x2000 | 0x4000 | 0x8000) {
            return Err(format!(
                "{DIAGNOSTIC_SRAM_CAPACITY_ENV} deve ser 8192, 16384 ou 32768 bytes; recebido {capacity}"
            ));
        }
        Ok(Some(capacity))
    }

    /// Resolve um perfil apenas pela identidade binária e carrega o SRM, se existir.
    fn prepare_cartridge(
        rom: &[u8],
        store: Arc<SaveRamStore>,
    ) -> Result<PreparedCartridge, String> {
        let identity = CartridgeIdentity::from_bytes(rom);
        let catalog = CartridgeCatalog::sms_default()
            .map_err(|error| format!("falha ao carregar catálogo SMS: {error}"))?;
        let profile = catalog
            .lookup_runtime_profile(&identity)
            .map_err(|error| format!("falha ao identificar cartucho SMS: {error}"))?
            .cloned();

        let Some(profile) = profile else {
            let diagnostic_capacity = parse_diagnostic_sram_capacity(
                std::env::var(DIAGNOSTIC_SRAM_CAPACITY_ENV).ok().as_deref(),
            )?;
            if let Some(capacity) = diagnostic_capacity {
                eprintln!(
                    "SRAM_PROFILE status=diagnostic-override sha1={} sha256={} capacity={} persistent=false restored=false",
                    identity.sha1_hex(),
                    identity.sha256_hex(),
                    capacity
                );
                return Ok(PreparedCartridge {
                    sram_capacity: Some(capacity),
                    initial_sram: None,
                    save_context: None,
                });
            }
            eprintln!(
                "SRAM_PROFILE status=unknown sha1={} sha256={}",
                identity.sha1_hex(),
                identity.sha256_hex()
            );
            return Ok(PreparedCartridge {
                sram_capacity: None,
                initial_sram: None,
                save_context: None,
            });
        };

        let initial_sram = store
            .load(&identity, &profile)
            .map_err(|error| format!("não foi possível validar/restaurar o SRM: {error}"))?;
        let save_context = profile
            .requires_persistent_store()
            .then(|| CartridgeSaveContext {
                identity: identity.clone(),
                profile: profile.clone(),
                store,
            });
        eprintln!(
            "SRAM_PROFILE status=verified sha1={} sha256={} capacity={} persistent={} restored={}",
            identity.sha1_hex(),
            identity.sha256_hex(),
            profile.ram_capacity_bytes().unwrap_or_default(),
            profile.requires_persistent_store(),
            initial_sram.is_some()
        );
        Ok(PreparedCartridge {
            sram_capacity: profile.ram_capacity_bytes(),
            initial_sram,
            save_context,
        })
    }

    /// Cria o worker já configurado com o perfil físico e o snapshot SRM restaurado.
    fn create_sms_worker(
        rom: Vec<u8>,
        settings: &SmsWorkerSettings,
        save_store: Arc<SaveRamStore>,
    ) -> Result<(SmsTemporalWorker, Option<CartridgeSaveContext>), String> {
        let prepared = prepare_cartridge(&rom, save_store)?;
        let worker = SmsTemporalWorker::new_with_cartridge_sram_size(
            rom,
            &settings.region,
            prepared.sram_capacity,
            prepared.initial_sram,
        )?;
        configure_sms_audio(
            &worker,
            settings.audio_volume,
            settings.dsp_config,
            &settings.renderer_mode,
        );
        worker.set_diagnostics_enabled(true);
        Ok((worker, prepared.save_context))
    }

    /// Persistência executada no host, após snapshot imutável obtido no worker.
    /// O Scheduler e o caminho da CPU nunca executam operações de filesystem.
    fn persist_worker_sram(
        worker: &SmsTemporalWorker,
        context: Option<&CartridgeSaveContext>,
    ) -> Result<(), String> {
        let Some(context) = context else {
            return Ok(());
        };
        let snapshot = worker
            .export_cartridge_sram()?
            .ok_or_else(|| "perfil persistente não expôs SRAM ao worker".to_owned())?;
        let store = Arc::clone(&context.store);
        let identity = context.identity.clone();
        let profile = context.profile.clone();
        let save = thread::Builder::new()
            .name("sms-sram-save".to_owned())
            .spawn(move || {
                store
                    .save(&identity, &profile, &snapshot)
                    .map_err(|error| error.to_string())
            })
            .map_err(|error| format!("não foi possível iniciar gravação SRM: {error}"))?;
        save.join()
            .map_err(|_| "thread de gravação SRM encerrou inesperadamente".to_owned())??;
        eprintln!(
            "SRAM_SAVE status=written sha256={} bytes={}",
            context.identity.sha256_hex(),
            context.profile.ram_capacity_bytes().unwrap_or_default()
        );
        Ok(())
    }

    /// Substitui a máquina sem permitir dois produtores simultâneos no FrameMailbox.
    fn replace_sms_worker(
        path: &PathBuf,
        worker_slot: &Rc<RefCell<Option<SmsTemporalWorker>>>,
        save_context_slot: &Rc<RefCell<Option<CartridgeSaveContext>>>,
        save_store: Arc<SaveRamStore>,
        mailbox: &FrameMailbox,
        settings: &SmsWorkerSettings,
    ) -> Result<(), String> {
        let rom = read_rom_image(path)?;
        let (next, next_save_context) = create_sms_worker(rom, settings, save_store)?;

        let previous = worker_slot.borrow_mut().take();
        if let Some(previous) = previous {
            let was_running = previous.is_running();
            if let Err(error) = previous.pause() {
                *worker_slot.borrow_mut() = Some(previous);
                return Err(error);
            }
            let previous_context = save_context_slot.borrow().clone();
            if let Err(error) = persist_worker_sram(&previous, previous_context.as_ref()) {
                if was_running {
                    let _ = previous.start();
                }
                *worker_slot.borrow_mut() = Some(previous);
                return Err(format!(
                    "ROM anterior preservada; falha ao gravar SRAM: {error}"
                ));
            }
            if let Err(error) = previous.detach_video_mailbox() {
                if was_running {
                    let _ = previous.start();
                }
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
            *save_context_slot.borrow_mut() = next_save_context;
            drop(previous);
        } else {
            next.attach_video_mailbox(mailbox.clone())?;
            *worker_slot.borrow_mut() = Some(next);
            *save_context_slot.borrow_mut() = next_save_context;
        }

        Ok(())
    }

    /// Contexto host-side que associa um worker ao arquivo SRM da ROM exata.
    #[derive(Clone)]
    struct CartridgeSaveContext {
        identity: CartridgeIdentity,
        profile: CartridgeHardwareProfile,
        store: Arc<SaveRamStore>,
    }

    struct PreparedCartridge {
        sram_capacity: Option<usize>,
        initial_sram: Option<Vec<u8>>,
        save_context: Option<CartridgeSaveContext>,
    }

    struct SmsWorkerSettings {
        region: String,
        audio_volume: u8,
        dsp_config: AnalogOutputConfig,
        renderer_mode: String,
    }

    /// Material Icons are decoded to Slint SVG images once during application startup.
    struct TransportIcons {
        play: slint::Image,
        pause: slint::Image,
        stop: slint::Image,
        reset: slint::Image,
        step_frame: slint::Image,
    }

    fn load_transport_icons() -> Result<TransportIcons, slint::LoadImageError> {
        use md_icons_classic::filled;

        Ok(TransportIcons {
            play: slint::Image::load_from_svg_data(filled::ICON_PLAY_ARROW.as_bytes())?,
            pause: slint::Image::load_from_svg_data(filled::ICON_PAUSE.as_bytes())?,
            stop: slint::Image::load_from_svg_data(filled::ICON_STOP.as_bytes())?,
            reset: slint::Image::load_from_svg_data(filled::ICON_REPLAY.as_bytes())?,
            step_frame: slint::Image::load_from_svg_data(filled::ICON_SKIP_NEXT.as_bytes())?,
        })
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
        let save_store = cartridge_save_store()?;
        let worker_settings = SmsWorkerSettings {
            region: region.clone(),
            audio_volume: effective_audio_volume(preferences.audio_volume, preferences.audio_muted),
            dsp_config: audio_dsp_config(
                preferences.audio_dsp_preset,
                preferences.audio_hpf_enabled,
                f64::from(preferences.audio_hpf_cutoff_hz),
                preferences.audio_lpf_enabled,
                f64::from(preferences.audio_lpf_cutoff_hz),
            ),
            renderer_mode: preferences.audio_renderer_mode.clone(),
        };
        let mut initial_save_context = None;
        let worker = if let Some(path) = rom_path.as_ref() {
            let (worker, context) = create_sms_worker(
                read_rom_image(path)?,
                &worker_settings,
                Arc::clone(&save_store),
            )?;
            initial_save_context = context;
            worker.attach_video_mailbox(mailbox.clone())?;
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
        let save_context = Rc::new(RefCell::new(initial_save_context));
        let recent_rom = Rc::new(RefCell::new(rom_path.clone()));

        let app = MainWindow::new()?;
        let transport_icons = load_transport_icons()?;
        app.set_play_icon(transport_icons.play);
        app.set_pause_icon(transport_icons.pause);
        app.set_stop_icon(transport_icons.stop);
        app.set_reset_icon(transport_icons.reset);
        app.set_step_frame_icon(transport_icons.step_frame);
        app.set_gpu_status("Aguardando inicialização do renderer WGPU 30...".into());
        app.set_frame_status("Aguardando o primeiro frame completo do VDP.".into());
        app.set_lifecycle_status("Renderer setup: 0 | teardown: 0".into());
        app.set_selected_region(region.clone().into());
        app.set_show_fps(preferences.show_fps);
        app.set_show_status_footer(preferences.show_status_footer);
        app.set_sprite_limit_enabled(preferences.sprite_limit_enabled);
        app.set_audio_volume(f32::from(preferences.audio_volume));
        app.set_audio_muted(preferences.audio_muted);
        app.set_audio_renderer_mode(preferences.audio_renderer_mode.clone().into());
        app.set_audio_dsp_preset(preferences.audio_dsp_preset);
        app.set_audio_hpf_enabled(preferences.audio_hpf_enabled);
        app.set_audio_hpf_cutoff(f32::from(preferences.audio_hpf_cutoff_hz));
        app.set_audio_lpf_enabled(preferences.audio_lpf_enabled);
        app.set_audio_lpf_cutoff(f32::from(preferences.audio_lpf_cutoff_hz));
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
        app.set_audio_status(if video_only_capture_enabled() {
            "Saída física desativada pelo modo de captura somente vídeo.".into()
        } else if worker
            .borrow()
            .as_ref()
            .is_some_and(|worker| worker.audio_output_metrics().sample_rate_hz > 0)
        {
            "CPAL ativo — volume aplicado no DSP de saída.".into()
        } else if worker.borrow().is_some() {
            "Saída CPAL indisponível; consulte R3_AUDIO nos logs.".into()
        } else {
            "Carregue uma ROM SMS para inicializar a saída CPAL.".into()
        });
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
        let stop_save_context = Rc::clone(&save_context);
        let stop_app = app.as_weak();
        app.on_stop_requested(move || {
            let result = stop_worker.borrow().as_ref().map_or(Ok(()), |worker| {
                let stop_result = worker.stop();
                let save_result =
                    persist_worker_sram(worker, stop_save_context.borrow().as_ref());
                match (stop_result, save_result) {
                    (Ok(()), Ok(())) => Ok(()),
                    (Err(stop_error), Err(save_error)) => Err(format!(
                        "{stop_error}; a persistência da SRAM também falhou: {save_error}"
                    )),
                    (Err(stop_error), Ok(())) => Err(stop_error),
                    (Ok(()), Err(save_error)) => {
                        Err(format!("máquina desligada, mas a SRAM não foi gravada: {save_error}"))
                    }
                }
            });
            if let Some(app) = stop_app.upgrade() {
                match result {
                    Ok(()) => {
                        app.set_emulation_paused(false);
                        app.set_rom_status(
                            "Máquina desligada (POWER OFF). ROM mantida; pressione Iniciar para ligar.".into(),
                        );
                    }
                    Err(error) => {
                        app.set_rom_status(format!("Falha ao desligar a máquina: {error}").into());
                        eprintln!("R2_CONTROL action=stop error={error}");
                    }
                }
            }
        });

        let reset_worker = Rc::clone(&worker);
        let reset_app = app.as_weak();
        app.on_reset_requested(move || {
            let result = reset_worker.borrow().as_ref().map_or(Ok(()), |worker| {
                worker.reset().and_then(|()| worker.start())
            });
            if let Some(app) = reset_app.upgrade() {
                match result {
                    Ok(()) => {
                        app.set_emulation_paused(false);
                        app.set_rom_status("Console reiniciado; execução retomada.".into());
                    }
                    Err(error) => {
                        app.set_rom_status(format!("Falha ao reiniciar o console: {error}").into());
                        eprintln!("R2_CONTROL action=reset error={error}");
                    }
                }
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

        let keyboard_state = Rc::new(RefCell::new(KeyboardInputState::default()));

        let keyboard_press_state = Rc::clone(&keyboard_state);
        let keyboard_press_worker = Rc::clone(&worker);
        let keyboard_press_app = app.as_weak();
        app.on_keyboard_key_pressed(move |key: slint::SharedString| {
            let Some(app) = keyboard_press_app.upgrade() else {
                return;
            };
            if app.get_active_menu().as_str() != "none" || !app.get_has_rom() {
                return;
            }

            let action = keyboard_press_state.borrow_mut().update(key.as_str(), true);
            let Some(action) = action else {
                return;
            };

            if action.is_controller_button() {
                let buttons = keyboard_press_state.borrow().controller_buttons();
                if let Some(worker) = keyboard_press_worker.borrow().as_ref()
                    && let Err(error) = worker.set_controller_at(1, buttons, Instant::now())
                {
                    eprintln!("SLINT_KEYBOARD action=keyboard_controller_press error={error}");
                }
            }

            match action {
                KeyboardAction::Nmi => {
                    if let Some(worker) = keyboard_press_worker.borrow().as_ref()
                        && let Err(error) = worker.request_nmi()
                    {
                        eprintln!("SLINT_KEYBOARD action=keyboard_nmi error={error}");
                    }
                }
                KeyboardAction::Start => app.invoke_start_requested(),
                KeyboardAction::Pause => app.invoke_pause_requested(),
                KeyboardAction::Stop => app.invoke_stop_requested(),
                KeyboardAction::Reset => app.invoke_reset_requested(),
                KeyboardAction::StepFrame => {
                    app.set_emulation_paused(true);
                    app.invoke_pause_requested();
                    app.invoke_step_frame_requested();
                }
                _ => {}
            }
        });

        let keyboard_release_state = Rc::clone(&keyboard_state);
        let keyboard_release_worker = Rc::clone(&worker);
        app.on_keyboard_key_released(move |key: slint::SharedString| {
            let action = keyboard_release_state
                .borrow_mut()
                .update(key.as_str(), false);
            if action.is_some_and(KeyboardAction::is_controller_button) {
                let buttons = keyboard_release_state.borrow().controller_buttons();
                if let Some(worker) = keyboard_release_worker.borrow().as_ref()
                    && let Err(error) = worker.set_controller_at(1, buttons, Instant::now())
                {
                    eprintln!("SLINT_KEYBOARD action=keyboard_controller_release error={error}");
                }
            }
        });

        let keyboard_focus_state = Rc::clone(&keyboard_state);
        let keyboard_focus_worker = Rc::clone(&worker);
        app.on_keyboard_focus_lost(move || {
            let previous_buttons = {
                let mut state = keyboard_focus_state.borrow_mut();
                let previous = state.controller_buttons();
                state.clear();
                previous
            };
            if previous_buttons != ControllerButtons::default()
                && let Some(worker) = keyboard_focus_worker.borrow().as_ref()
                && let Err(error) =
                    worker.set_controller_at(1, ControllerButtons::default(), Instant::now())
            {
                eprintln!("SLINT_KEYBOARD action=keyboard_focus_release error={error}");
            }
        });

        let open_app = app.as_weak();
        let open_worker = Rc::clone(&worker);
        let open_save_context = Rc::clone(&save_context);
        let open_save_store = Arc::clone(&save_store);
        let open_recent = Rc::clone(&recent_rom);
        let open_recent_list = Rc::clone(&recent_roms);
        let open_mailbox = mailbox.clone();
        app.on_load_rom_requested(move |path: slint::SharedString| {
            let path = PathBuf::from(path.as_str());
            if path.as_os_str().is_empty() {
                if let Some(app) = open_app.upgrade() {
                    app.set_rom_status(
                        "Informe o caminho completo da ROM (.sms, .bin ou .zip).".into(),
                    );
                }
                return;
            }

            let (region, sprite_limit_enabled, audio_volume, dsp_config, renderer_mode) = open_app
                .upgrade()
                .map(|app| {
                    (
                        app.get_selected_region().to_string(),
                        app.get_sprite_limit_enabled(),
                        effective_audio_volume(
                            app.get_audio_volume().round().clamp(0.0, 100.0) as u8,
                            app.get_audio_muted(),
                        ),
                        audio_dsp_config(
                            app.get_audio_dsp_preset(),
                            app.get_audio_hpf_enabled(),
                            f64::from(app.get_audio_hpf_cutoff().round() as u16),
                            app.get_audio_lpf_enabled(),
                            f64::from(app.get_audio_lpf_cutoff().round() as u16),
                        ),
                        app.get_audio_renderer_mode().to_string(),
                    )
                })
                .unwrap_or_else(|| {
                    (
                        "usa".to_owned(),
                        true,
                        70,
                        AnalogOutputConfig::default(),
                        "psg".to_owned(),
                    )
                });
            let open_settings = SmsWorkerSettings {
                region,
                audio_volume,
                dsp_config,
                renderer_mode,
            };

            match replace_sms_worker(
                &path,
                &open_worker,
                &open_save_context,
                Arc::clone(&open_save_store),
                &open_mailbox,
                &open_settings,
            ) {
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
                        app.set_audio_status(if video_only_capture_enabled() {
                            "Saída física desativada pelo modo de captura somente vídeo.".into()
                        } else if open_worker
                            .borrow()
                            .as_ref()
                            .is_some_and(|worker| worker.audio_output_metrics().sample_rate_hz > 0)
                        {
                            "CPAL ativo — volume aplicado no DSP de saída.".into()
                        } else {
                            "Saída CPAL indisponível; consulte R3_AUDIO nos logs.".into()
                        });
                        app.set_emulation_paused(false);
                        app.set_rom_status(
                            match start_result {
                                Some(Ok(())) => {
                                    format!("ROM: {} | execução iniciada", path.display())
                                }
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
        let close_save_context = Rc::clone(&save_context);
        app.on_close_rom_requested(move || {
            let previous = close_worker.borrow_mut().take();
            if let Some(previous) = previous {
                let was_running = previous.is_running();
                if let Err(error) = previous.pause() {
                    if was_running {
                        let _ = previous.start();
                    }
                    *close_worker.borrow_mut() = Some(previous);
                    if let Some(app) = close_app.upgrade() {
                        app.set_rom_status(
                            format!("Falha ao pausar antes de salvar SRAM: {error}").into(),
                        );
                    }
                    return;
                }
                if let Err(error) =
                    persist_worker_sram(&previous, close_save_context.borrow().as_ref())
                {
                    if was_running {
                        let _ = previous.start();
                    }
                    *close_worker.borrow_mut() = Some(previous);
                    if let Some(app) = close_app.upgrade() {
                        app.set_rom_status(
                            format!("ROM mantida; falha ao gravar SRAM: {error}").into(),
                        );
                    }
                    return;
                }
                let _ = previous.stop();
                let _ = previous.detach_video_mailbox();
                drop(previous);
                *close_save_context.borrow_mut() = None;
            }
            if let Some(app) = close_app.upgrade() {
                app.set_has_rom(false);
                app.set_emulation_paused(false);
                app.set_audio_status("Sem ROM carregada; volume e mute permanecem salvos.".into());
                app.set_rom_status("Nenhuma ROM carregada.".into());
                app.set_debug_status("Máquina fechada.".into());
            }
        });

        let exit_recent = Rc::clone(&recent_roms);
        let exit_worker = Rc::clone(&worker);
        let exit_save_context = Rc::clone(&save_context);
        let exit_prefs_app = app.as_weak();
        app.on_exit_requested(move || {
            if let Some(active_worker) = exit_worker.borrow().as_ref() {
                let was_running = active_worker.is_running();
                if let Err(error) = active_worker.pause() {
                    if was_running {
                        let _ = active_worker.start();
                    }
                    if let Some(app) = exit_prefs_app.upgrade() {
                        app.set_rom_status(
                            format!("Saída cancelada; falha ao pausar a máquina: {error}").into(),
                        );
                    }
                    return;
                }
                if let Err(error) =
                    persist_worker_sram(active_worker, exit_save_context.borrow().as_ref())
                {
                    if was_running {
                        let _ = active_worker.start();
                    }
                    if let Some(app) = exit_prefs_app.upgrade() {
                        app.set_rom_status(
                            format!("Saída cancelada; falha ao gravar SRAM: {error}").into(),
                        );
                    }
                    return;
                }
            }
            let _ = save_recent_roms(&exit_recent.borrow());
            if let Some(app) = exit_prefs_app.upgrade() {
                let prefs = DesktopPreferences {
                    region: app.get_selected_region().to_string(),
                    aspect: app.get_selected_aspect().to_string(),
                    show_fps: app.get_show_fps(),
                    show_status_footer: app.get_show_status_footer(),
                    fps_size: app.get_fps_size().to_string(),
                    sprite_limit_enabled: app.get_sprite_limit_enabled(),
                    audio_volume: app.get_audio_volume().round().clamp(0.0, 100.0) as u8,
                    audio_muted: app.get_audio_muted(),
                    audio_renderer_mode: app.get_audio_renderer_mode().to_string(),
                    audio_dsp_preset: app.get_audio_dsp_preset(),
                    audio_hpf_enabled: app.get_audio_hpf_enabled(),
                    audio_hpf_cutoff_hz: app.get_audio_hpf_cutoff().round().clamp(5.0, 500.0)
                        as u16,
                    audio_lpf_enabled: app.get_audio_lpf_enabled(),
                    audio_lpf_cutoff_hz: app.get_audio_lpf_cutoff().round().clamp(4_000.0, 20_000.0)
                        as u16,
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
                audio_volume: app.get_audio_volume().round().clamp(0.0, 100.0) as u8,
                audio_muted: app.get_audio_muted(),
                audio_renderer_mode: app.get_audio_renderer_mode().to_string(),
                audio_dsp_preset: app.get_audio_dsp_preset(),
                audio_hpf_enabled: app.get_audio_hpf_enabled(),
                audio_hpf_cutoff_hz: app.get_audio_hpf_cutoff().round().clamp(5.0, 500.0) as u16,
                audio_lpf_enabled: app.get_audio_lpf_enabled(),
                audio_lpf_cutoff_hz: app.get_audio_lpf_cutoff().round().clamp(4_000.0, 20_000.0)
                    as u16,
            };
            if let Err(error) = save_desktop_preferences(&prefs) {
                eprintln!("R2_PREFERENCES save_failed error={error}");
            }
        });

        let audio_volume_worker = Rc::clone(&worker);
        let audio_volume_app = app.as_weak();
        app.on_audio_volume_changed(move |value| {
            let volume = value.round().clamp(0.0, 100.0) as u8;
            let Some(app) = audio_volume_app.upgrade() else {
                return;
            };
            let effective = effective_audio_volume(volume, app.get_audio_muted());
            let result = audio_volume_worker
                .borrow()
                .as_ref()
                .map(|worker| worker.set_audio_volume(effective));
            match result {
                Some(Ok(())) => app.set_audio_status(
                    format!(
                        "Volume mestre aplicado: {volume}%{}.",
                        if effective == 0 { " (silenciado)" } else { "" }
                    )
                    .into(),
                ),
                Some(Err(error)) => {
                    app.set_audio_status(format!("Falha ao aplicar volume: {error}").into())
                }
                None => app.set_audio_status(
                    "Volume salvo; será aplicado ao carregar uma ROM SMS.".into(),
                ),
            }
            app.invoke_preferences_changed();
        });

        let audio_mute_worker = Rc::clone(&worker);
        let audio_mute_app = app.as_weak();
        app.on_audio_mute_changed(move |muted| {
            let Some(app) = audio_mute_app.upgrade() else {
                return;
            };
            let volume = app.get_audio_volume().round().clamp(0.0, 100.0) as u8;
            let effective = effective_audio_volume(volume, muted);
            let result = audio_mute_worker
                .borrow()
                .as_ref()
                .map(|worker| worker.set_audio_volume(effective));
            match result {
                Some(Ok(())) => app.set_audio_status(if muted {
                    "Áudio silenciado; volume escolhido preservado.".into()
                } else {
                    format!("Áudio reativado em {volume}%.").into()
                }),
                Some(Err(error)) => {
                    app.set_audio_status(format!("Falha ao alterar áudio: {error}").into())
                }
                None => app.set_audio_status(
                    "Preferência salva; será aplicada ao carregar uma ROM SMS.".into(),
                ),
            }
            app.invoke_preferences_changed();
        });

        let audio_dsp_worker = Rc::clone(&worker);
        let audio_dsp_app = app.as_weak();
        app.on_audio_dsp_changed(
            move |preset, hpf_enabled, hpf_cutoff, lpf_enabled, lpf_cutoff| {
                let Some(app) = audio_dsp_app.upgrade() else {
                    return;
                };
                let config = audio_dsp_config(
                    preset,
                    hpf_enabled,
                    f64::from(hpf_cutoff.round() as u16),
                    lpf_enabled,
                    f64::from(lpf_cutoff.round() as u16),
                );
                let result = audio_dsp_worker
                    .borrow()
                    .as_ref()
                    .map(|worker| worker.set_audio_dsp_config(config));
                match result {
                    Some(Ok(())) => app.set_audio_status(if config.enabled {
                        "DSP aplicado no worker; filtros ativos na saída PCM.".into()
                    } else {
                        "DSP analógico adicional desativado; saída original preservada.".into()
                    }),
                    Some(Err(error)) => {
                        app.set_audio_status(format!("Falha ao aplicar DSP: {error}").into())
                    }
                    None => app.set_audio_status(
                        "Preferências DSP salvas; serão aplicadas ao carregar uma ROM SMS.".into(),
                    ),
                }
                app.invoke_preferences_changed();
            },
        );

        let audio_renderer_worker = Rc::clone(&worker);
        let audio_renderer_app = app.as_weak();
        app.on_audio_renderer_changed(move |value| {
            let mode = if value.as_str() == "h2-experimental" {
                AudioRendererMode::H2Experimental
            } else {
                AudioRendererMode::Psg
            };
            let Some(app) = audio_renderer_app.upgrade() else {
                return;
            };
            let result = audio_renderer_worker
                .borrow()
                .as_ref()
                .map(|worker| worker.set_audio_renderer(mode));
            match result {
                Some(Ok(())) => app.set_audio_status(match mode {
                    AudioRendererMode::Psg => "Renderer PSG selecionado.".into(),
                    AudioRendererMode::H2Experimental => "H2 experimental selecionado; fallback automático para PSG em caso de falha.".into(),
                }),
                Some(Err(error)) => {
                    app.set_audio_renderer_mode("psg".into());
                    app.set_audio_status(format!("H2 indisponível; PSG preservado: {error}").into());
                }
                None => app.set_audio_status(
                    "Preferência do renderer salva; será aplicada ao iniciar uma ROM SMS.".into(),
                ),
            }
            app.invoke_preferences_changed();
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

        let queue_slot = Arc::new(Mutex::new(None));
        let setup_counter = Rc::new(Cell::new(0_u64));
        let teardown_counter = Rc::new(Cell::new(0_u64));
        attach_gpu_texture(
            &app,
            Arc::clone(&queue_slot),
            mailbox.clone(),
            setup_counter,
            teardown_counter,
        )?;
        let host_pacing = Arc::new(Mutex::new(HostFramePacingWindow::new()));
        let _frame_diagnostics_timer =
            start_frame_diagnostics(&app, Arc::clone(&host_pacing), Rc::clone(&worker));
        start_frame_presentation(&app, Arc::clone(&queue_slot), mailbox.clone(), host_pacing)?;

        let run_result = app.run();
        let final_preferences = DesktopPreferences {
            region: app.get_selected_region().to_string(),
            aspect: app.get_selected_aspect().to_string(),
            show_fps: app.get_show_fps(),
            show_status_footer: app.get_show_status_footer(),
            fps_size: app.get_fps_size().to_string(),
            sprite_limit_enabled: app.get_sprite_limit_enabled(),
            audio_volume: app.get_audio_volume().round().clamp(0.0, 100.0) as u8,
            audio_muted: app.get_audio_muted(),
            audio_renderer_mode: app.get_audio_renderer_mode().to_string(),
            audio_dsp_preset: app.get_audio_dsp_preset(),
            audio_hpf_enabled: app.get_audio_hpf_enabled(),
            audio_hpf_cutoff_hz: app.get_audio_hpf_cutoff().round().clamp(5.0, 500.0) as u16,
            audio_lpf_enabled: app.get_audio_lpf_enabled(),
            audio_lpf_cutoff_hz: app.get_audio_lpf_cutoff().round().clamp(4_000.0, 20_000.0) as u16,
        };
        let _ = save_desktop_preferences(&final_preferences);
        let _ = save_recent_roms(&recent_roms.borrow());

        let mut final_save_error = None;
        if let Some(active_worker) = worker.borrow_mut().take() {
            if let Err(error) = active_worker.pause() {
                eprintln!("SRAM_SAVE pause_before_exit_failed error={error}");
            }
            if let Err(error) = persist_worker_sram(&active_worker, save_context.borrow().as_ref())
            {
                final_save_error = Some(error);
            }
            let _ = active_worker.stop();
            let _ = active_worker.detach_video_mailbox();
            drop(active_worker);
        }
        *save_context.borrow_mut() = None;
        let _ = mailbox.close();
        app.set_gpu_texture(slint::Image::default());
        if let Some(error) = final_save_error {
            return Err(error.into());
        }
        run_result?;
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn diagnostic_sram_capacity_accepts_only_supported_volatile_sizes() {
            assert_eq!(parse_diagnostic_sram_capacity(None).unwrap(), None);
            assert_eq!(
                parse_diagnostic_sram_capacity(Some("8192")).unwrap(),
                Some(0x2000)
            );
            assert_eq!(
                parse_diagnostic_sram_capacity(Some("16384")).unwrap(),
                Some(0x4000)
            );
            assert_eq!(
                parse_diagnostic_sram_capacity(Some("32768")).unwrap(),
                Some(0x8000)
            );
            assert!(parse_diagnostic_sram_capacity(Some("4096")).is_err());
            assert!(parse_diagnostic_sram_capacity(Some("invalid")).is_err());
        }

        #[test]
        fn master_volume_is_bounded_and_mute_is_independent() {
            assert_eq!(effective_audio_volume(70, false), 70);
            assert_eq!(effective_audio_volume(70, true), 0);
            assert_eq!(effective_audio_volume(0, false), 0);
            assert_eq!(effective_audio_volume(100, false), 100);
            assert_eq!(effective_audio_volume(255, false), 100);
        }

        #[test]
        fn default_audio_preferences_start_at_a_moderate_volume() {
            let preferences = DesktopPreferences::default();
            assert_eq!(preferences.audio_volume, 70);
            assert!(!preferences.audio_muted);
        }

        #[test]
        fn keyboard_mapping_recognizes_gameplay_and_transport_actions() {
            assert_eq!(
                KeyboardAction::from_slint_key("up"),
                Some(KeyboardAction::Up)
            );
            assert_eq!(
                KeyboardAction::from_slint_key("button1"),
                Some(KeyboardAction::Button1)
            );
            assert_eq!(
                KeyboardAction::from_slint_key("nmi"),
                Some(KeyboardAction::Nmi)
            );
            assert_eq!(
                KeyboardAction::from_slint_key("start"),
                Some(KeyboardAction::Start)
            );
            assert_eq!(
                KeyboardAction::from_slint_key("step-frame"),
                Some(KeyboardAction::StepFrame)
            );
            assert_eq!(KeyboardAction::from_slint_key("unknown"), None);
        }

        #[test]
        fn keyboard_state_composes_simultaneous_directions_and_buttons() {
            let mut state = KeyboardInputState::default();
            assert_eq!(state.update("up", true), Some(KeyboardAction::Up));
            assert_eq!(state.update("right", true), Some(KeyboardAction::Right));
            assert_eq!(state.update("button1", true), Some(KeyboardAction::Button1));
            assert_eq!(state.update("button2", true), Some(KeyboardAction::Button2));

            let buttons = state.controller_buttons();
            assert!(buttons.up && buttons.right);
            assert!(buttons.button_1 && buttons.button_2);
            assert!(!buttons.down && !buttons.left);
        }

        #[test]
        fn keyboard_state_ignores_repeats_and_releases_only_the_matching_key() {
            let mut state = KeyboardInputState::default();
            assert_eq!(state.update("up", true), Some(KeyboardAction::Up));
            assert_eq!(state.update("up", true), None);
            assert_eq!(state.update("right", true), Some(KeyboardAction::Right));
            assert_eq!(state.update("up", false), Some(KeyboardAction::Up));

            let buttons = state.controller_buttons();
            assert!(!buttons.up);
            assert!(buttons.right);
            assert_eq!(state.update("right", false), Some(KeyboardAction::Right));
            assert_eq!(state.controller_buttons(), ControllerButtons::default());
        }

        #[test]
        fn keyboard_transport_actions_are_edge_triggered_until_key_release() {
            let mut state = KeyboardInputState::default();
            assert_eq!(state.update("start", true), Some(KeyboardAction::Start));
            assert_eq!(state.update("start", true), None);
            assert_eq!(state.update("start", false), Some(KeyboardAction::Start));
            assert_eq!(state.update("start", true), Some(KeyboardAction::Start));
        }

        #[test]
        fn keyboard_focus_loss_releases_all_held_controller_buttons() {
            let mut state = KeyboardInputState::default();
            state.update("left", true);
            state.update("button1", true);
            assert_ne!(state.controller_buttons(), ControllerButtons::default());

            state.clear();

            assert_eq!(state.controller_buttons(), ControllerButtons::default());
        }

        #[test]
        fn video_only_capture_flag_accepts_only_explicit_true_values() {
            assert!(video_only_capture_requested(Some("1")));
            assert!(video_only_capture_requested(Some("true")));
            assert!(video_only_capture_requested(Some("TRUE")));
            assert!(!video_only_capture_requested(None));
            assert!(!video_only_capture_requested(Some("0")));
            assert!(!video_only_capture_requested(Some("false")));
            assert!(!video_only_capture_requested(Some("")));
        }

        #[test]
        fn host_duration_window_reports_nearest_rank_percentiles() {
            let mut samples = HostDurationWindow::default();
            for value in [10, 20, 30, 40, 50] {
                samples.push(value);
            }

            assert_eq!(samples.len(), 5);
            assert_eq!(samples.percentile_ns(50), 30);
            assert_eq!(samples.percentile_ns(95), 50);
            assert_eq!(samples.percentile_ns(99), 50);
        }

        #[test]
        fn host_duration_window_retains_only_the_latest_512_samples() {
            let mut samples = HostDurationWindow::default();
            for value in 1..=513 {
                samples.push(value);
            }

            assert_eq!(samples.len(), HOST_PACING_WINDOW_CAPACITY);
            assert_eq!(samples.percentile_ns(50), 257);
            assert_eq!(samples.percentile_ns(95), 488);
            assert_eq!(samples.percentile_ns(99), 508);
        }

        #[test]
        fn host_frame_pacing_records_wakeup_and_consumption_distributions() {
            let mut window = HostFramePacingWindow::new();
            let start = Instant::now();
            for (index, micros) in [10, 20, 30, 40, 50].into_iter().enumerate() {
                window.record_wakeup(start + Duration::from_micros((index as u64) * micros));
            }
            assert_eq!(window.wake_interval_distribution.len(), 4);
            assert!(window.wake_interval_distribution.percentile_ns(95) > 0);

            assert_eq!(
                window.record_frame(start + Duration::from_millis(1), 1, start),
                None
            );
            assert_eq!(
                window.record_frame(
                    start + Duration::from_millis(2),
                    2,
                    start + Duration::from_millis(1),
                ),
                None
            );
            assert_eq!(window.consumed_interval_distribution.len(), 1);
            assert_eq!(
                window.consumed_interval_distribution.percentile_ns(50),
                1_000_000
            );
            assert_eq!(window.mailbox_residence_distribution.len(), 2);
        }

        #[test]
        fn host_frame_pacing_fps_counts_consumed_frames_not_skipped_sequences() {
            let mut window = HostFramePacingWindow::new();
            let start = Instant::now();
            window.last_fps_sample = start;
            window.last_fps_consumed = 0;

            assert_eq!(
                window.record_frame(start + Duration::from_millis(150), 1, start),
                None
            );
            assert_eq!(
                window.record_frame(
                    start + Duration::from_millis(300),
                    3,
                    start + Duration::from_millis(150),
                ),
                None
            );
            assert_eq!(
                window.record_frame(
                    start + Duration::from_millis(450),
                    5,
                    start + Duration::from_millis(300),
                ),
                None
            );
            let fps = window
                .record_frame(
                    start + Duration::from_millis(600),
                    8,
                    start + Duration::from_millis(450),
                )
                .unwrap();

            // Four frames were consumed in 600 ms; skipped sequence numbers
            // must not inflate the displayed presentation rate.
            assert!((fps - (4.0 / 0.6)).abs() < 0.001);
            assert_eq!(window.sequence_gaps, 4);
        }

        #[test]
        fn host_frame_pacing_window_records_callback_and_mailbox_take_costs() {
            let mut window = HostFramePacingWindow::new();
            window.record_callback(Duration::from_micros(120));
            window.record_mailbox_take(Duration::from_micros(8));

            assert_eq!(window.callback_count, 1);
            assert_eq!(window.callback_sum_ns, 120_000);
            assert_eq!(window.callback_max_ns, 120_000);
            assert_eq!(window.mailbox_take_count, 1);
            assert_eq!(window.mailbox_take_sum_ns, 8_000);
            assert_eq!(window.mailbox_take_max_ns, 8_000);
        }

        #[test]
        fn classic_material_transport_icons_load_as_slint_svg_images() {
            let icons = load_transport_icons().unwrap();
            for icon in [
                icons.play,
                icons.pause,
                icons.stop,
                icons.reset,
                icons.step_frame,
            ] {
                assert!(icon.size().width > 0);
                assert!(icon.size().height > 0);
            }
        }

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

        fn zip_archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
            use std::io::Write;
            use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

            let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
            let options =
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            for (name, contents) in entries {
                writer.start_file(*name, options).unwrap();
                writer.write_all(contents).unwrap();
            }
            writer.finish().unwrap().into_inner()
        }

        #[test]
        fn zip_rom_loader_reads_single_sms_entry_in_memory() {
            let bytes = zip_archive(&[("games/sonic.sms", b"SMS-ROM")]);
            assert_eq!(extract_rom_from_zip(&bytes).unwrap(), b"SMS-ROM");
        }
        #[test]
        fn zip_rom_loader_rejects_archives_without_roms_or_with_multiple_roms() {
            let no_rom = zip_archive(&[("readme.txt", b"not a rom")]);
            assert!(extract_rom_from_zip(&no_rom).is_err());

            let multiple = zip_archive(&[("sonic.sms", b"first"), ("alex-kidd.bin", b"second")]);
            assert!(extract_rom_from_zip(&multiple).is_err());
        }

        #[test]
        fn zip_rom_loader_rejects_invalid_archive_bytes() {
            assert!(extract_rom_from_zip(b"not a ZIP archive").is_err());
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
