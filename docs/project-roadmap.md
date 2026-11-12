# Roadmap mestre — Sega Emulator

**Revisão documental:** 2026-11-12. A fotografia operacional deve ser vinculada ao HEAD consultado no GitHub.
**Decisões:** ADR 0010 (runtime/hardware), ADR 0011 (desktop/hosts) e ADR-0016 (fidelidade temporal).

Duas trilhas coordenadas: evolução dos sistemas Sega (P0–P9) e replataforma desktop/host (R1–R5). A segunda não autoriza reescrever hardware nem antecipar sistemas.

## 1. Diretrizes

- Runtime 100% Rust; MachineScheduler é autoridade de clocks, eventos e ordem causal.
- CPU, VDP, PSG, memória, barramentos, mappers, interrupções e estados pertencem ao runtime.
- Slint é GUI; WGPU é API gráfica; Vulkan é backend inicial preferencial; CPAL é saída física.
- SDL3 será avaliado para input/gamepads após auditoria.
- Não ampliar nem reintroduzir o host desktop anterior. A retirada física dos módulos transitórios ocorre somente após R3/R5 e inventário de consumidores.
- GUI não executa hardware, avança clocks ou usa callbacks por instrução.
- Preservar SmsTemporalWorker, Scheduler, SMS e SG-1000. Não duplicar componentes.
- Sem JIT, frame skipping, hacks temporais ou otimizações especulativas.

## 2. Plataformas — P0 a P9

1. **P0:** infraestrutura/hosts, toolchain, workspaces, CI e contratos.
2. **P1:** Master System temporal, Z80 compartilhado, ROM, lifecycle, vídeo, áudio, input e validação.
3. **P2:** SG-1000, preservando componentes compartilhados e regressões.
4. **P3:** Game Gear.
5. **P4:** Mega Drive/Genesis.
6. **P5:** Sega CD.
7. **P6:** 32X / Sega CD 32X.
8. **P7:** frontend desktop completo em Slint: launcher, biblioteca, configurações, HUD e telemetria.
9. **P8:** compatibilidade e auditoria.
10. **P9:** comprovação temporal sistêmica dos caminhos observáveis.

Não avançar para plataforma posterior com bloqueadores estruturais anteriores. O shell mínimo e as integrações necessárias ao SMS não equivalem ao frontend completo P7.

## 3. Replataforma — R1 a R5

### R1 — POC gráfica mínima (prioridade imediata)

Comprovar no Windows x64: janela Slint; WGPU 30; Vulkan explicitamente selecionado/identificado; mesmo wgpu::Device e wgpu::Queue compartilhados com Slint; textura externa WGPU atualizada continuamente; composição na mesma janela; resize, lifecycle da surface e shutdown limpo; build Release reproduzível.

R1 é isolada. Não integrar SMS real, SmsTemporalWorker, menu de jogo, pausa do Scheduler, HUD, CPAL, SDL3, HDR ou VRR. Não alterar hardware/runtime.

**Gate:** execução real Windows x64 com Vulkan confirmado, textura atualizada e composição na mesma janela. Análise documental não é PASS de integração.

### R2 — vídeo real

Integrar frames SMS por contrato portátil, preservando SmsTemporalWorker, FrameMailbox e Scheduler. Validar formato, ownership, sincronização, lifecycle e apresentação sem alterar timing.

### R3 — áudio CPAL 0.18.2

Enumerar dispositivos/formatos; criar, executar e encerrar streams; reproduzir PCM real; tratar erros, remoção/reconexão e underruns. Callback eficiente. Thread de áudio não controla Scheduler.

### R4 — input SDL3

Auditar APIs; integrar teclado, gamepads, hotplug e mapeamentos; encaminhar eventos sem lógica de hardware no host.

### R5 — frontend

Consolidar native/sega_desktop, native/video_output_wgpu e native/audio_output; implementar launcher, biblioteca, configurações, menus, HUD, logs e telemetria; provar paridade; remover egui/eframe e hosts antigos somente após regressão e aceite Release no HEAD exato.

## 4. Estado operacional

A fotografia de migração está em [legacy-host-retirement.md](architecture/migrations/legacy-host-retirement.md); status de PRs e workflows deve ser reconsultado no GitHub antes de cada merge.

- Main auditada: `82ccad8eb9cd11d78dfddd168f307f90012426b5`.
- PR #745 — ADR 0011 e reconciliação documental: MERGED.
- R1 — POC Slint/WGPU/Vulkan: PR #747 MERGED.
- R2 — frames reais SMS no desktop Slint/WGPU: PR #752 MERGED; preferências persistentes #754 MERGED.
- R3 — CPAL 0.18.2: PENDING.
- R4 — SDL3: PENDING.
- R5 — consolidação e retirada integral dos hosts anteriores: PENDING.

PR aberta, branch ou CI parcial não representa integração funcional. Reauditar GitHub e SHA antes de cada tarefa.

## 5. Z80

E4C → E5 → E6 → E7 → E8 → E9. Consultar docs/z80-completion-scorecard.md e docs/z80-dd-fd-semantic-matrix.md antes de qualquer tarefa. Não iniciar E8/ZEXDOC/ZEXALL antes do fechamento demonstrável de E7. Não reabrir etapas validadas sem evidência de defeito/regressão.

## 6. Método e gates

Auditar SHA/PRs/workflows; pesquisar fontes primárias; definir critérios; modelar estados/ownership/timing; implementar incrementos pequenos; conferir formatação, Clippy e testes antes do workflow; regredir; auditar diff; atualizar documentação/evidências; publicar por PR.

Gates:
- cargo fmt --all -- --check
- cargo clippy --release --workspace --all-targets --locked -- -D warnings
- cargo test --release --workspace --locked

Validar workspaces/targets standalone. Aplicativos e testes de desempenho devem usar Release. Classificar PASS, FAIL, BLOCKED ou NOT RUN; SKIPPED nunca é PASS. Vincular evidências ao SHA exato.

## 7. GitHub

GitHub é a fonte da verdade. Toda alteração em branch/PR. Proibidos commit direto na main, force-push e reset destrutivo. Merge exige CI aplicável verde no HEAD final e autorização explícita. PR aberta, documentação ou CI parcial não significam integração funcional.
