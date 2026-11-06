# Auditoria de equivalência — SMS nativo e áudio (2026-11-06)

- **Baseline auditada:** `main` em `edbec9272efe87a6a408bac74283d86a91d87b23`.
- **PR documental relacionada:** [#606](https://github.com/leonardo201800478/sega-emulator/pull/606).
- **Arquivo Python auditado:** `tests/test_sms_native_audio.py` (9 funções).
- **Resultado:** nenhum teste Python removido nesta etapa. A classificação separa equivalência do núcleo SMS de contratos de adapter/host que ainda dependem da aplicação Rust.

## Evidências Rust consultadas

- `native/sms_core/src/machine.rs`
  - `cpu_out_routes_psg_register_writes_to_independent_sms_core`
  - `pal_clock_and_generated_audio_samples_are_native_and_deterministic`
  - `console_pause_switch_enters_the_z80_nmi_vector`
  - `runtime_sprite_limit_policy_updates_vdp_and_machine_config`
  - `cpu_in_instruction_reads_sms_controller_port`
- `native/sms_core/src/temporal_machine.rs`
  - `immediate_out_to_psg_commits_at_the_io_boundary_and_restores`
  - `immediate_in_reads_controller_and_out_updates_io_control`
  - `step_frame_returns_a_complete_sms1_rgb24_frame`
  - `frame_step_limit_is_reported_without_faking_vdp_progress`
  - `synthetic_rom_produces_visible_sms1_pixels_and_nonzero_psg_samples`
  - `step_frame_wires_vdp_vblank_irq_to_z80_and_services_halt_loop`
- `native/audio_output/src/lib.rs`
  - testes de validação WASAPI/XAudio2 antes de acessar dispositivo.
  - testes de PCM mono→estéreo e balance.
- `native/audio_runtime/src/session.rs`
  - sessão DSP independente de Python/PyO3/Qt, com sink injetável para testes headless.

A presença desses testes comprova contratos específicos, não toda a equivalência do arquivo Python.

## Matriz por teste Python

| Teste Python | Classificação | Evidência / ação necessária |
|---|---|---|
| `test_sms_binding_uses_native_psg_and_ntsc_clock` | **ALREADY_COVERED — núcleo** | `cpu_out_routes_psg_register_writes_to_independent_sms_core` valida registradores, atenuação e clock NTSC. `synthetic_rom_produces_visible_sms1_pixels_and_nonzero_psg_samples` valida amostras no caminho temporal. Conferir explicitamente a taxa 44.100 Hz em Rust antes de remover o caso integral. |
| `test_sms_machine_rejects_unknown_native_audio_backend` | **MIGRATE** | Seleção de backend pertence à aplicação/host Rust, não ao núcleo SMS. Implementar enum tipado e rejeição de seleção não suportada em `native/sega_app`/audio host; manter erro diagnosticável e sessão carregável. Testes de validação de configuração WASAPI/XAudio2 não cobrem, por si só, a seleção de backend desconhecido. |
| `test_sms_pal_profile_selects_pal_psg_clock` | **ALREADY_COVERED** | `pal_clock_and_generated_audio_samples_are_native_and_deterministic` verifica clock PAL de 3.546.895 Hz e igualdade das amostras de duas máquinas. |
| `test_sms_brazil_pal_m_profile_uses_ntsc_like_clock` | **MIGRATE** | Política Brasil/PAL-M deve ser resolvida em perfil de mídia/aplicação Rust e testada sem adapter Python. O teste atual verifica simultaneamente a binding e o adapter Python; não se deve introduzir uma regra regional duplicada no core SMS. |
| `test_sms_adapter_routes_console_pause_to_z80_nmi` | **ALREADY_COVERED** | `console_pause_switch_enters_the_z80_nmi_vector` verifica vetor 0x0066 e stack após NMI no core Rust. |
| `test_sms_frame_runner_exposes_native_framebuffer_and_controller` | **ALREADY_COVERED — núcleo / parcial** | `step_frame_returns_a_complete_sms1_rgb24_frame` e `immediate_in_reads_controller_and_out_updates_io_control` cobrem frame e input no temporal runtime. A API da aplicação deve ser testada separadamente; o teste Python de binding não será mantido como destino final. |
| `test_sms_adapter_exposes_qt_frame_and_controller_contract` | **REPLACE** | Contrato do adapter Qt não deve sobreviver. Substituir por teste da aplicação egui/eframe consumindo framebuffer e input diretamente das APIs Rust; core temporal já tem testes de frame e controle. |
| `test_sms_sprite_limit_setting_is_exposed_by_native_and_python_adapters` | **ALREADY_COVERED — núcleo** | `runtime_sprite_limit_policy_updates_vdp_and_machine_config` verifica estado default, desativação e reativação no core Rust. A exposição pela aplicação fica no teste de UI/API Rust. |
| `test_sms_adapter_rejects_non_boolean_sprite_limit_value` | **REMOVE — fronteira Python obsoleta** | Rust recebe `bool` estaticamente em `set_sprite_limit_enabled(bool)`; o caso verifica coerção dinâmica específica de Python e não representa um comportamento de hardware. A API Rust tipada deve continuar coberta por seus testes funcionais. |

## Critérios para fechar este domínio

1. Acrescentar/confirmar teste Rust explícito para sample rate 44.100 Hz no caminho SMS.
2. Implementar e testar a seleção tipada de backend de áudio na aplicação Rust; falha do endpoint deve ser diagnosticável, mas não impedir o carregamento da ROM/sessão quando o áudio for opcional.
3. Migrar a resolução de região Brasil/PAL-M para o serviço de mídia/perfil Rust, com vetores que distingam PAL-M de PAL e NTSC.
4. Criar testes da aplicação egui/eframe para apresentação de framebuffer, input, estado de áudio e preferências, sem exigir dispositivo físico em testes headless.
5. Executar os gates Rust no SHA candidato e registrar testes físicos Windows como NOT RUN quando não houver host/dispositivo real.
6. Só então remover `tests/test_sms_native_audio.py`.

## Estado

- Auditoria dos 9 nomes/contratos: **PASS** (leitura estática das fontes).
- Equivalência total do arquivo: **NOT COMPLETE**.
- Implementação de lacunas identificadas: **NOT STARTED** nesta auditoria.
- Execução dos testes Rust: **NOT RUN** nesta auditoria.
- Remoção do arquivo Python: **NOT AUTHORIZED BY EVIDENCE / NOT STARTED**.

Não inferir equivalência apenas porque existe uma função Rust com nome semelhante.