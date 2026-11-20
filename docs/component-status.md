# Status dos componentes — revisão 2026-11-16

**Baseline:** `main` `f8315ed5a95d6ef1d7cc02aa2edd813bb1d6b434`.
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
| BIOS SMS | Implementação candidata na PR #928 | Teste físico com BIOS real, handoff e início do cartucho pendentes |
| SG-1000 | Core funcional legado e coordenador/barramento temporal inicial | Na main, coordenador registra apenas Z80; barramento rejeita PortRead/PortWrite; Sg1000Machine ainda usa cpu.step() e avança VDP/PSG em fronteiras de instrução. PR #981 é candidata P2.1, ainda não integrada; P2.2 deve conectar Z80MicroSequencer/M1 ao próprio Sg1000Machine |
| Game Gear | Scaffold/planejamento inicial | Auditoria independente e especificação própria após SG-1000 |
| YM2413 | Modelo arquitetural e documentação existentes | Após Game Gear, implementar somente YM2413 integrado do Master System japonês; FM Sound Unit externo do Mark III excluído |
| Sega Card/My Card | Mídia de software documentada | Preservar carregamento e slots nativos SMS/Mark III; modelar seleção/arbitragem conforme geração após Game Gear |
| Card Catcher C-1000 | Adaptador físico histórico documentado | Excluído da implementação; não transportar lógica para o core SG-1000 |
| 3D Glasses / outros periféricos | Referências técnicas disponíveis | Especificação e implementação na fase P4, por compatibilidade de console |
| Debugger SMS | Parcial e em evolução | Captura de duração fixa, snapshots CPU/VDP, exportação e usabilidade; não bloqueia hardware |
| R5 desktop | Parcial | Biblioteca, configurações consolidadas, HUD/telemetria e limpeza segura de legado |
| HUD/OSD operacional | Parcial / planejada | FPS visibility/text size já documentados; overlay posicionável, catálogo de mensagens, fila/TTL/prioridades e eventos operacionais ainda pendentes; issue #1112 |

## Estados e proveniência

- `VALIDATED`: gates automatizados executados e aprovados no SHA declarado, no escopo declarado.
- `INTERNAL_REVIEW`: revisão interna concluída, sem alegar confirmação externa.
- `NOT_VERIFIED`: não há evidência suficiente.
- `CLOSED_WITH_LIMITATIONS`: escopo praticável encerrado com limitações explícitas.
- Evidência física fornecida pelo usuário deve ser registrada como relato de teste físico, sem inventar SHA, log ou ambiente.
- `SKIPPED` e `NOT RUN` nunca são PASS.

## CI da baseline

Main `f8315ed5a95d6ef1d7cc02aa2edd813bb1d6b434`: workflow push pós-merge #43962737265. Change impact classification, Documentation patch hygiene e Rust quality/unit PASS. Workspace, Z80, Scheduler, SMS1 VDP, CPAL, SDL3, Slint/WGPU/Vulkan e demais jobs de código/runtime foram SKIPPED; portanto, não há nova validação de runtime pós-merge. A PR #980 foi validada em seu HEAD `c83d459ba710785a86ae28632b2c0bcc98e3a05f` pelo run #43959831347: impact, documentation hygiene e Rust quality/unit PASS; demais jobs SKIPPED.
