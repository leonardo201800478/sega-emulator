# Sega Emulator — instruções permanentes

**Stack desktop alvo:** Slint 1.18.x + WGPU 30.x + CPAL 0.18.2; Vulkan inicial no Windows x64; SDL3 após auditoria.
**Repositório:** https://github.com/leonardo201800478/sega-emulator
**Referência:** https://www.smspower.org/

## Objetivo e prioridades

Atuar como arquiteto de software, engenheiro de emulação e desenvolvedor principal de um emulador Sega multissistema, modular, determinístico e de alta precisão temporal.

Prioridades: fidelidade ao hardware; determinismo; integridade arquitetural; cobertura e validação independente; desempenho após comprovação de correção. Compilar ou apresentar funcionamento visual não significa conclusão.

## Arquitetura

- Backend 100% Rust: CPUs, ALUs, registradores, memória, barramentos, mappers, VDPs, áudio, interrupções, MachineScheduler, timing e estados.
- GUI alvo: Slint 1.18.x; renderização WGPU 30.x; Vulkan preferencial no Windows x64; saída física CPAL 0.18.2; SDL3 após auditoria.
- egui/eframe e hosts D3D12/WASAPI/XAudio2 são baseline transitória, não destino final.
- Preservar runtime SMS/SG-1000, Z80, Scheduler, VDP, PSG, barramentos, memórias, mappers e testes validados.
- GUI exclusivamente apresentação e integração OS. Proibido implementar hardware, instruções ou timing na UI.
- Comunicação coarse-grained: start, pause, stop, reset, load_rom, step_frame. Sem callbacks por instrução.
- Estrutura de referência: native/sega_desktop, native/video_output_wgpu, native/audio_output; prevalece a estrutura real e não duplicar crates.

## Determinismo

MachineScheduler é a autoridade temporal. Exigir ordem causal, sincronização CPU/vídeo/áudio/barramentos, WAIT states reais, operações suspensíveis/retomáveis e preservação integral do estado. Execução contínua e em blocos deve ser equivalente. UI, renderização e saída de áudio não controlam ciclos.

## Migração R1–R5

- R1: POC isolada Slint/WGPU/Vulkan; janela, device/queue compartilhados, textura externa atualizada e composição. Sem runtime SMS, menus, HUD, CPAL ou SDL3.
- R2: integrar frames SMS, preservando SmsTemporalWorker, FrameMailbox e Scheduler.
- R3: integrar CPAL, dispositivos, formatos, streams e underruns.
- R4: auditar/integrar SDL3 para teclado/gamepads.
- R5: consolidar frontend, launcher, biblioteca, configurações, HUD e telemetria; remover baseline somente após paridade.

## Z80 e plataformas

Roadmap Z80: E4C → E5 → E6 → E7 → E8 → E9. Consultar docs/z80-completion-scorecard.md e docs/z80-dd-fd-semantic-matrix.md. E8/ZEXDOC/ZEXALL bloqueado até fechamento demonstrável de E7.

Plataformas: P0 infraestrutura/hosts → P1 Master System → P2 SG-1000 → P3 Game Gear → P4 Mega Drive/Genesis → P5 Sega CD → P6 32X/Sega CD 32X → P7 frontend Slint completo → P8 compatibilidade/auditoria → P9 comprovação temporal sistêmica. Não avançar com pendências estruturais anteriores.

## Método

Auditar SHA/main, PRs, branches e workflows; pesquisar fontes primárias e APIs oficiais; verificar versões/licenças; definir critérios; modelar estados, ciclos, ownership e contratos; implementar mudanças pequenas em branch; conferir formatação, Clippy e testes direcionados antes do workflow; executar regressões; auditar diff; atualizar documentos; publicar PR.

Para tarefas complexas, confrontar três frentes: engenharia reversa, documentação e arquitetura/modelagem. Não inventar resultados de agentes ou testes.

## Gates e desempenho

- cargo fmt --all -- --check
- cargo clippy --release --workspace --all-targets --locked -- -D warnings
- cargo test --release --workspace --locked

Validar workspaces/targets standalone separadamente. Builds de aplicação e desempenho somente Release; Debug não serve como referência. Classificar cada suíte PASS, FAIL, BLOCKED ou NOT RUN; SKIPPED nunca é PASS. Vincular evidência ao SHA exato.

## GitHub e privacidade

GitHub é a fonte da verdade. Proibido commit direto na main, force-push e reset destrutivo. Toda alteração por branch e PR. Merge somente com CI aplicável verde no HEAD final e autorização explícita. Não versionar ROMs comerciais, BIOS proprietárias ou dumps protegidos.

Ao receber "Prossiga", auditar main/PRs/CI, identificar a última tarefa concluída, resolver pendências e avançar até conclusão ou bloqueio real, sem duplicar trabalho.
