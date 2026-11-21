# Status dos componentes — revisão 2026-11-21

**Baseline:** `main` `eac5f8f49025111372183190b8504f476a994aae`.
**Regra:** estado integrado na main separado de PRs, testes físicos e relatos manuais.

| Área | Estado na baseline | Pendência real |
|---|---|---|
| Rust/runtime | Integrado | Gates devem ser vinculados ao SHA de cada mudança |
| Desktop Slint/WGPU/Vulkan | Caminho ativo | Revalidar execução física Vulkan, resize/lifecycle e apresentação sustentada no HEAD atual |
| Host gráfico anterior | Fora de uso ativo | Código remanescente é legado; inventariar consumidores antes de remover |
| CPAL 0.18.2 | Integrado em software pela PR #770 | Reprodução física prolongada, hotplug/reconexão, underruns e negociação de formatos |
| SDL3/input runtime | Primeira integração Slint/SDL3 pela PR #867 | Controles físicos, hotplug real, identidade persistente, remapeamento e perfis |
| Z80 compartilhado | `CLOSED_WITH_LIMITATIONS` no perfil NMOS delimitado | CMOS/clones, WAIT físico, DMA e validação elétrica permanecem não verificados; E8 é complementar |
| MachineScheduler | Integrado, com testes determinísticos | Não confundir stress de software com sincronismo físico sistêmico |
| SMS1/SMS2 | Runtime integrado; correções recentes presentes | BIOS boot aguarda validação física; VDP/HCounter e raster só reabrir com evidência atual |
| SMS sprites | Alinhamento reportado pelo usuário como corrigido | Sem defeito aberto conhecido; reabrir somente por regressão reproduzível |
| Road Rash | Testado fisicamente pelo usuário e reportado OK | Removido do backlog de defeitos; sem SHA/relatório técnico atribuído |
| BIOS SMS | Implementação integrada pela PR #928 | Validação física com BIOS real, handoff e início do cartucho permanecem pendentes |
| SG-1000 | P2.2/P2.3/P2.4 integradas na main pelas PRs #988/#989/#990. `Sg1000Machine` usa `Z80MicroSequencer`; `Sg1000TemporalBus` aceita memória, PortRead e PortWrite; CPU/VDP/PSG avançam no mesmo MachineScheduler; worker Rust e `DesktopSession` reutilizam o host Slint/WGPU/CPAL/SDL3. A máquina atômica antiga está limitada a `cfg(test)` como oracle | P2 não está fechado: gameplay real dos perfis Type-A/Type-B/2 KiB/Othello/The Castle, validação física Windows de vídeo/áudio/input, hotplug/underruns/lifecycle e replay dedicado de eventos ainda não demonstrados |
| Game Gear | Scaffold/planejamento inicial | Auditoria independente e especificação própria após SG-1000 |
| YM2413 | Implementação Rust em andamento; core, PG/EG e serviços temporais em desenvolvimento; PR #1133 aberta para hooks internos de slot | Não declarar FM concluído: serviço PG serial, integração de operadores/canais melódicos, Rhythm Mode, saída digital, perfis SMS e mistura PSG+FM ainda exigem fechamento e validação diferencial |
| Sega Card/My Card | Mídia de software documentada | Preservar carregamento e slots nativos SMS/Mark III; modelar seleção/arbitragem conforme geração após Game Gear |
| Card Catcher C-1000 | Adaptador físico histórico documentado | Excluído da implementação; não transportar lógica para o core SG-1000 |
| 3D Glasses / outros periféricos | Referências técnicas disponíveis | Especificação e implementação na fase P4, por compatibilidade de console |
| Debugger SMS | Parcial e em evolução | Captura de duração fixa, snapshots CPU/VDP, exportação e usabilidade; não bloqueia hardware |
| R5 desktop | Parcial | Biblioteca, configurações consolidadas, HUD/telemetria e limpeza segura de legado |
| HUD/OSD operacional | Parcial / planejada | FPS visibility/text size já documentados; overlay posicionável, catálogo de mensagens, fila/TTL/prioridades e eventos operacionais ainda pendentes; issue #1112 |
| RetroAchievements | Planejado; não implementado | ADR Rust/FFI, hash/identificação SMS e SG-1000, memory maps, cliente/rede, integração por frame, UI, Hardcore e validação por título; ver marcos RA-0 a RA-6 no roadmap mestre |

## RetroAchievements — critérios de entrada

Não iniciar a implementação antes de estabilizar os contratos de frame, lifecycle e Save State de SMS/SG-1000. O suporte planejado começa pelos console IDs SMS 11 e SG-1000 33; a identificação usa MD5. A biblioteca de referência `rcheevos` é escrita em C, portanto a integração exige ADR prévia para decidir se uma FFI encapsulada em Rust é compatível com a diretriz Rust-only. Nenhum suporte é considerado disponível enquanto não houver integração e evidência por título. Hardcore permite Save State, mas deve bloquear Load State e outras ações incompatíveis durante a sessão.

## Estados e proveniência

- `VALIDATED`: gates automatizados executados e aprovados no SHA declarado, no escopo declarado.
- `INTERNAL_REVIEW`: revisão interna concluída, sem alegar confirmação externa.
- `NOT_VERIFIED`: não há evidência suficiente.
- `CLOSED_WITH_LIMITATIONS`: escopo praticável encerrado com limitações explícitas.
- Evidência física fornecida pelo usuário deve ser registrada como relato de teste físico, sem inventar SHA, log ou ambiente.
- `SKIPPED` e `NOT RUN` nunca são PASS.

## SG-1000 P2 — rastreabilidade

- P2.2: PR #988, merge `be883792025de3db7eaf4e7b8243293ea90b7e06`; máquina temporal, M1/WAIT, portas e snapshots compostos.
- P2.3: PR #989, merge `a728c448bb656f19650e88e214a9a723bbcc2713`; worker Rust, FrameMailbox, AudioSession e save-state.
- P2.4: PR #990, merge/main `0a7b24fe014d11ec7adbc13c1e7ca79a68d0a315`; DesktopSession, seleção/roteamento multi-sistema e perfis de RAM por hash.
- Auditoria detalhada: [sg1000-p2-post-p24-reconciliation-2026-11-17.md](audits/sg1000-p2-post-p24-reconciliation-2026-11-17.md).
- P2 permanece aberto até validação in-game dos perfis de cartucho e validação física do host. Não iniciar P3/Game Gear antes dos gates de saída.

## CI e proveniência da baseline

- **Main consultada:** `8a80fc784c3b9b2563cea248f5c3ba1b4073dfc9` (20/11/2026).
- **P2.4:** a evidência de push [#44135959914](https://github.com/leonardo201800478/sega-emulator/actions/runs/44135959914) pertence ao SHA `0a7b24fe014d11ec7adbc13c1e7ca79a68d0a315`, não à main atual. PASS naquele SHA: workspace, Slint/WGPU/Vulkan POC, SDL3, PowerShell, docs e Rust quality; CPAL, PSG, Scheduler, Z80 e SMS1 VDP foram SKIPPED.
- **DP0:** PR #1107 merge `bc4664d9f44f6791ad8f8520a57c8744dd647dbf`; PR #1117 merge `eac1d2463637a65d40621066d4ec889d2e062884`. CI push #9431 e SMS Runtime #1716 passaram no merge DP0; CPAL possui validação independente na CI #9418. A auditoria DP0 registra F6–F10 PASS e mantém benchmark físico Windows e smoke test local como NOT RUN.
- A main atual está seis commits à frente do merge #1117. A consulta desta reconciliação não encontrou execução push identificável para o SHA atual; não transferir automaticamente resultados de SHAs anteriores.
- `SKIPPED` e `NOT RUN` nunca são PASS. Esta atualização não declara auditoria integral dos documentos nem validação física do SG-1000.
