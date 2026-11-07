# Roadmap mestre — Sega Emulator

**Status:** rota normativa de implementação.  
**Baseline documental:** main `3ad1781f84b7824b5aa889a583ae6d4545987ec1` (2026-11-07).  
**Regra:** o roadmap define sequência e gates; o [status datado](project-status-2026-11-07.md) registra o estado observado de código, PRs e validações. Antes de cada tarefa, auditar SHA/main, PRs, workflows e arquivos relevantes.

**Política arquitetural:** [ADR 0010 — produto 100% Rust e SMS temporal](adr/0010-rust-only-sms-temporal-first.md).  
**Política de fidelidade:** [ADR-0016 — fidelidade observável e temporização proporcional](architecture/adr/0016-observable-fidelity-and-proportional-timing.md).

## 1. Diretriz vigente

O produto é integralmente Rust, com interface desktop egui/eframe. Python, PySide6/Qt, PyO3 e Maturin não fazem parte do produto nem podem ser reintroduzidos no runtime, testes, tooling, scripts, CI ou distribuição.

O primeiro runtime executável é o Master System temporal, composto pelo Z80 compartilhado, barramento/memória/mapper, PSG, VDP e MachineScheduler. O Scheduler é a autoridade única da ordem temporal. A GUI não executa hardware, avança clocks ou realiza callbacks por instrução.

O shell egui está integrado à main. A integração vertical do shell com ROM/runtime SMS, viewport D3D12, áudio e input está em PRs abertas (#613 e #614), empilhadas; não considerar essas funcionalidades integradas até os merges e validações correspondentes.

## 2. Sequência normativa

1. **P0 — Infraestrutura e hosts:** workspaces, toolchain, CI, boundaries e hosts nativos.
2. **P1 — Master System:** runtime temporal único, ROM, lifecycle, vídeo, áudio, input e validação funcional/temporal proporcional.
3. **P2 — SG-1000:** nova integração sobre componentes compartilhados, preservando regressões e sem duplicar CPU/chips.
4. **P3 — Game Gear.**
5. **P4 — Mega Drive/Genesis.**
6. **P5 — Sega CD.**
7. **P6 — 32X / Sega CD 32X.**
8. **P7 — Frontend egui/eframe completo:** biblioteca, preferências e fluxos de aplicação além do shell mínimo, sem lógica de hardware.
9. **P8 — Compatibilidade e auditoria.**
10. **P9 — Comprovação temporal sistêmica dos caminhos observáveis.**

Não iniciar uma plataforma posterior com bloqueadores estruturais nas anteriores. A integração mínima do shell egui necessária para executar o SMS pertence ao P1; o frontend completo permanece P7.

## 3. Estado da prioridade imediata — P1 / SMS

### 3.1 Integrado na main

- Produto e shell nativos Rust/egui.
- Worker/runtime SMS temporal em Rust.
- Componentes compartilhados sem runtime atômico SMS como fallback.
- CI Rust-first e contratos de validação.
- Melhoria de fidelidade do SN76489 integrada pela PR #615.
- Esclarecimento da arquitetura Rust-only integrado pela PR #617.
- MachineScheduler com testes de determinismo, codec e restauração; campanha longa documentada em [evidência de 2026-11-07](evidence/scheduler-stress-long-2026-11-07.md).

### 3.2 Em PRs abertas — não integradas

- **PR #613:** integração do runtime SMS temporal, carregamento de ROM e viewport D3D12 no shell.
- **PR #614:** integração de áudio WASAPI/XAudio2 e controles de teclado/SDL3; branch empilhada sobre #613.
- **PR #616:** reconciliação documental da auditoria de boot/cartuchos.

Os estados de CI e validação física devem ser reconsultados no HEAD exato antes de qualquer merge.

### 3.3 Gates de aceitação SMS

- ROM sintética/homebrew carregada sem depender de ROM comercial versionada.
- Z80 compartilhado como única CPU.
- Um único MachineScheduler coordenando os componentes temporais.
- Framebuffer real apresentado pelo host gráfico.
- PCM não silencioso entregue ao backend nativo e validado em endpoint físico.
- Input nativo validado com teclado e gamepad.
- Start/pause/resume/stop/reset/close sem threads órfãs.
- Execução contínua versus em blocos com equivalência demonstrada.
- Snapshot/restore determinístico dos estados que integram a máquina.
- Regressões e gates Rust/Windows no mesmo SHA final.

A campanha de stress do Scheduler é uma evidência de robustez do Scheduler isolado; não substitui os gates de integração acima.

## 4. Z80 — sequência obrigatória

- E4C: INC/DEC indexado.
- E5: DDCB/FDCB.
- E6: resolução de prefixos.
- E7: fechamento semântico e auditoria de lacunas.
- E8: validação externa/formal, incluindo ZEXDOC/ZEXALL após harness Rust completo.
- E9: integração temporal dos caminhos de gameplay que exigirem granularidade.

O [scorecard](z80-completion-scorecard.md) e a [matriz semântica DD/FD](z80-dd-fd-semantic-matrix.md) são documentos obrigatórios antes de qualquer tarefa Z80. Não iniciar E8 antes de E7 fechado. A situação registrada em 2026-11-07 mantém E7.7 IN_PROGRESS e E8 BLOCKED.

## 5. Contratos arquiteturais

- Backend e lógica de hardware em Rust.
- egui/eframe exclusivamente para UI, menus, estado de apresentação e comandos de alto nível.
- Integração coarse-grained: `load_rom`, `start`, `pause`, `stop`, `reset`, `step_frame`.
- Scheduler único, causal e determinístico.
- WAIT e suspensão/retomada somente onde o modelo e o caminho observável exigirem.
- Sem duplicação de CPU, PSG, VDP, worker ou Scheduler.
- Sem JIT, frame skipping, hacks temporais ou otimizações especulativas.
- Precisão proporcional a interações observáveis, conforme ADR-0016; não exigir microsequenciamento universal sem consumidor demonstrado.

## 6. Método de entrega

1. Auditar SHA da main, PRs e workflows.
2. Pesquisar evidência independente e documentação primária.
3. Definir critérios de aceitação mensuráveis.
4. Modelar estados, transições, eventos e ciclos necessários.
5. Implementar incrementos pequenos e compiláveis.
6. Executar testes direcionados.
7. Executar regressão e gates aplicáveis.
8. Auditar diff e riscos.
9. Atualizar documentação e ledger de evidências.
10. Publicar por branch e PR.

Para hardware complexo, confrontar três frentes: engenharia reversa, documentação primária e arquitetura/modelagem.

## 7. Gates obrigatórios

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Executar também os workspaces standalone e os gates Windows/D3D12/WASAPI/XAudio2 enquanto existirem. Classificar cada suíte como PASS, FAIL, BLOCKED ou NOT RUN. SKIPPED, ausência de job e resultado de SHA anterior não são PASS.

Toda validação deve indicar SHA, ambiente, comando, escopo e limitações. Testes físicos não executados permanecem NOT RUN.

## 8. GitHub e integração

- GitHub é a fonte da verdade.
- Toda alteração em branch e Pull Request.
- Proibido commit direto na main, force-push e reset destrutivo.
- Merge somente com CI 100% verde no HEAD final e autorização explícita do mantenedor.
- Não inferir integração a partir de PR aberta, branch, build local ou documentação.
- Não versionar ROMs comerciais, BIOS proprietárias ou dumps protegidos.

## 9. Documentos operacionais

- [Mapa de documentação](README.md)
- [Status atual](project-status-2026-11-07.md)
- [Status por componente](component-status.md)
- [Política de CI](ci-validation-policy.md)
- [Política de desenvolvimento e validação](development-and-validation-policy.md)
- [Scorecard Z80](z80-completion-scorecard.md)
- [Matriz DD/FD](z80-dd-fd-semantic-matrix.md)
- [Ledger de evidências](evidence/README.md)
