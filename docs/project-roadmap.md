# Roadmap mestre — Sega Emulator

**Status:** rota normativa de implementação.
**Baseline de reconciliação:** main `9731b37098e7937fa92fffc15007a7c79aac4e4c` (2026-11-08).
**Regra:** o roadmap define sequência e gates; o [status dos componentes](component-status.md) registra o estado observado de código, PRs e validações. Antes de cada tarefa, auditar SHA/main, PRs, workflows e arquivos relevantes.

**Política arquitetural:** [ADR 0010 — produto 100% Rust e SMS temporal](adr/0010-rust-only-sms-temporal-first.md).
**Política de fidelidade:** [ADR-0016 — fidelidade observável e temporização proporcional](architecture/adr/0016-observable-fidelity-and-proportional-timing.md).

## 1. Diretriz vigente

O produto é integralmente Rust, com interface desktop egui/eframe. Python, PySide6/Qt, PyO3 e Maturin não fazem parte do produto nem podem ser reintroduzidos no runtime, testes, tooling, scripts, CI ou distribuição.

O primeiro runtime executável é o Master System temporal, composto pelo Z80 compartilhado, barramento/memória/mapper, PSG, VDP e MachineScheduler. O Scheduler é a autoridade única da ordem temporal. A GUI não executa hardware, avança clocks ou realiza callbacks por instrução.

O shell egui, runtime SMS, viewport D3D12, áudio WASAPI/XAudio2 e controles teclado/SDL3 foram integrados pelas PRs #613 e #614. A integração no código não equivale a aceite físico: a qualidade sonora continua bloqueada pelas issues #640 e #659 e pelos gates de host ainda não executados na baseline.

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

**Prioridade operacional atual: confiabilidade de áudio.** Antes de retomar expansão de plataforma ou tarefas não bloqueadoras do SMS, executar a trilha AUDIO-00 a AUDIO-10 do [scorecard de áudio](audio/audio-reliability-scorecard.md). O confronto das quatro auditorias independentes está em [auditoria consolidada](audits/audio-reliability-consolidated-2026-11-08.md).

### 3.1 Estado integrado

- Produto e shell nativos Rust/egui.
- Runtime SMS temporal, worker, viewport D3D12, áudio WASAPI/XAudio2 e controles teclado/SDL3 integrados pelas PRs #613 e #614.
- Componentes compartilhados sem runtime atômico SMS como fallback.
- Melhoria de fidelidade SN76489 integrada pela PR #615.
- Correção de ciclo de saída/epoch do resampler integrada pela PR #658.
- Amostragem SN76489 em posições fracionárias do clock de saída integrada pela PR #661.
- MachineScheduler com testes de determinismo, codec e restauração; a campanha longa documentada em [evidência de 2026-11-07](evidence/scheduler-stress-long-2026-11-07.md) é evidência do Scheduler isolado, não de áudio físico ou TEMPORAL_PASS sistêmico.

### 3.2 Sprint transversal AUDIO — prioridade imediata

A issue #659 relata áudio distorcido, fora de ritmo e aparentemente lento em WASAPI e XAudio2, mesmo quando os underruns são raros ou não observados. Portanto, investigar primeiro a geração PCM compartilhada, não presumir que a FIFO ou um backend isolado seja a causa.

1. **AUDIO-00 — baseline e contrato:** fixar SHA, dispositivo, taxa, cenário, unidades, epochs e semântica dos contadores.
2. **AUDIO-01 — geração e taxa efetiva:** rastrear PSG → conversão → resampler → blocos PCM; medir taxa, cadência, fase, amplitude e clipping.
3. **AUDIO-02 — resampler/epoch:** provar continuidade de fase, equivalência por chunking e comportamento em flush/reset/load.
4. **AUDIO-03 — transporte/backends:** corrigir underflow, starvation XAudio2, flush, falha terminal e submit; separar métricas por camada.
5. **AUDIO-04 — lifecycle:** validar Pause/Resume/Stop/Reset/Load/Close/StepFrame e atualizar snapshots após transições confirmadas.
6. **AUDIO-05 — validação física:** comparar WASAPI e XAudio2 no Windows com a mesma sequência PCM, captura de loopback e métricas sincronizadas.

**Correções de primeira rodada:** a [PR #666](https://github.com/leonardo201800478/sega-emulator/pull/666) implementa rejeição de escrita após falha terminal, contabilidade de flush/submissão, starvation XAudio2 separado e snapshots do worker após lifecycle. O HEAD `29916f7ddb27a069b385a57e53d04760de5e01ed` passou workspace fmt/Clippy/testes, Linux audio-output e Windows WASAPI/XAudio2; a PR está aberta, não integrada. Isso fecha parte dos achados estáticos, mas não determina a causa da distorção/ritmo lento reportada em #659.

**Gate de saída:** causa raiz demonstrada para #640/#659; ausência de perda/duplicação silenciosa; taxa/cadência coerentes; testes automatizados PASS; validação física dos dois backends PASS; regressão SG-1000 preservada. Até lá, áudio permanece BLOCKED para aceite de release e novas plataformas ficam congeladas. Não alterar clocks do PSG/MachineScheduler nem aumentar buffers por tentativa e erro.

### 3.3 PRs e issues relevantes

- **PRs #613 e #614:** MERGED; integram runtime/viewport e áudio/input. Merge não equivale a aceite físico.
- **PR #657:** CLOSED / NOT MERGED; não atribuir a ela estado de integração.
- **PR #666:** OPEN, CI SUCCESS no HEAD `29916f7d`; correções iniciais de falha/telemetria aguardam merge; validação física NOT RUN.
- **PRs #643, #644 e #645:** OPEN, base antiga e `mergeable=false`; não fazem parte da main e exigem reconciliação independente.
- **Issues #640 e #659:** OPEN; underruns e qualidade sonora sem causa raiz confirmada.
- **Issue #641:** OPEN; corrupção visual, trilha independente de áudio.

### 3.4 Gates de aceitação SMS

- ROM sintética/homebrew carregada sem depender de ROM comercial versionada.
- Z80 compartilhado como única CPU.
- Um único MachineScheduler coordenando os componentes temporais.
- Framebuffer real apresentado pelo host gráfico.
- PCM não silencioso, com taxa/cadência/fase validadas, entregue ao backend nativo e validado em endpoint físico.
- Métricas distinguem FIFO underflow, endpoint starvation, frames em voo, flush e reprodução efetiva.
- Falhas terminais WASAPI/XAudio2 são propagadas; nenhuma escrita pode retornar sucesso sem consumidor ativo.
- Input nativo validado com teclado e gamepad.
- Start/pause/resume/stop/reset/close sem threads órfãs.
- Execução contínua versus em blocos com equivalência demonstrada.
- Snapshot/restore determinístico dos estados que integram a máquina.
- Regressões e gates Rust/Windows no mesmo SHA final.

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

- [Scorecard de confiabilidade de áudio](audio/audio-reliability-scorecard.md)
- [Auditoria consolidada das análises independentes](audits/audio-reliability-consolidated-2026-11-08.md)

- [Mapa de documentação](README.md)
- [Status atual](project-status-2026-11-07.md)
- [Status por componente](component-status.md)
- [Política de CI](ci-validation-policy.md)
- [Política de desenvolvimento e validação](development-and-validation-policy.md)
- [Scorecard Z80](z80-completion-scorecard.md)
- [Matriz DD/FD](z80-dd-fd-semantic-matrix.md)
- [Ledger de evidências](evidence/README.md)
