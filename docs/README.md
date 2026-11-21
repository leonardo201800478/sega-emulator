# Documentação do Sega Emulator

Este diretório reúne decisões normativas, escopo, roadmap, status, arquitetura, auditorias, evidências e documentos históricos.

## Precedência

1. ADRs normativas vigentes: runtime/ownership, desktop Slint/WGPU/CPAL e fidelidade temporal.
2. project-scope.md e project-roadmap.md para escopo e ordem de plataformas.
3. component-status.md para fotografia de status na baseline identificada.
4. Código/testes integrados na main e workflows executados no SHA correspondente.
5. Auditorias datadas, PRs não integradas e documentos históricos.

PR Draft, branch, documento candidato ou CI parcialmente SKIPPED não alteram o estado integrado da main.

## Documentos de controle

- [Escopo do projeto](project-scope.md)
- [Roadmap mestre](project-roadmap.md)
- [Roadmap de produto desktop — capacidades, UX, mídia e diagnóstico](desktop-product-roadmap.md)
- [Auditoria DP-0 — consentimento e observabilidade](audits/dp0-diagnostics-consent-audit-2026-11-20.md)
- [Status dos componentes](component-status.md)
- [Reorganização e reconciliação 2026-11-16](project-reconciliation-2026-11-16.md)
- [Política de CI e validação](ci-validation-policy.md)
- [Instruções para agentes](../AGENTS.md)
- [ADRs normativas](adr/README.md)
- [Arquitetura geral](architecture.md)
- [Índice de hosts/documentos históricos](architecture/legacy-document-index.md)
- [Scheduler](architecture/scheduler/README.md)

## Vídeo e filtros

- [Contrato de apresentação de vídeo e filtros](architecture/video-presentation-filters-sync.md)
- [Arquitetura do pipeline de filtros](architecture/video-shader-pipeline.md)
- [Arquitetura híbrida nativa Rust + WGSL](architecture/native-video-filters-hybrid-pipeline.md)
- [Catálogo inicial de presets Slang](shaders/README.md)
- [ADR 0014 — pós-processamento retro nativo](adr/0014-retro-shader-pipeline-librashader-wgpu.md)

## Integrações e serviços

- RetroAchievements: trilha planejada e gates RA-0 a RA-6 no [roadmap mestre](project-roadmap.md#retroachievements--integração-transversal-planejada).

## Referências de interface

- [Tela de configurações de vídeo — referência Light/Dark](ui/settings-screen-transfer.md). Assets visuais; não representam funcionalidades implementadas.

## Sistemas e hardware

- [SMS1 × SMS2 — diferenças](hardware/sms1-sms2-hardware-difference-matrix-2026-11-15.md)
- [SMS expansion and accessories roadmap](hardware/sms-expansion-accessories-roadmap.md)
- [SG-1000](hardware/sg1000.md)
- [YM2413/FM model](architecture/ym2413-fm-model.md)
- [SMS firmware/BIOS](architecture/sms-bios-firmware-loading.md)

## Áudio

- [Plano Modern Hi-Fi — SMS e SG-1000](audio/modern-hifi-audio-roadmap.md)
- [Estudo PSG H2](audio/psg-h2-reconstruction-study.md)
- [Scorecard de confiabilidade](audio/audio-reliability-scorecard.md)
- [YM2413 — estratégia equilibrada de execução e áudio](architecture/ym2413-balanced-execution-audio.md). Proposta complementar, subordinada ao modelo canônico e ao scorecard.

## Z80

- [Scorecard](z80-completion-scorecard.md)
- [Checklist mestre](z80-master-audit-checklist.md)
- [Matriz DD/FD](z80-dd-fd-semantic-matrix.md)
- [Auditoria ED](z80-ed-opcode-audit.md)
- [Comportamentos não documentados](z80-undocumented-behavior-research.md)
- [Estado consolidado E7](audits/z80-e7-current-status-2026-11-16.md)
- [Matriz dos 12 critérios E7](audits/z80-e7-12-criteria-matrix-2026-11-16.md)
- [Revisão interna F-022](audits/z80-e7-f022-adversarial-review-2026-11-16.md)
- [Revisão interna F-023](audits/z80-e7-f023-adversarial-review-2026-11-16.md)
- [Revisão interna F-028](audits/z80-e7-f028-adversarial-review-2026-11-16.md)
- [Plano SG-1000 P2](audits/sg1000-p2-atomic-reference-reuse-2026-11-16.md)

Z80 está encerrado com limitações explícitas no perfil NMOS delimitado. E8/ZEXDOC/ZEXALL é validação complementar futura, não gate permanente para iniciar SG-1000.

## Evidências e histórico

- evidence/: resultados reproduzíveis vinculados a SHA.
- audits/: investigações técnicas e limitações.
- hardware/: referências de hardware e pinouts.
- milestones/: marcos históricos.
- releases/: notas de release.
- sprints/: objetivos temporários.

Preservar histórico. Classificar documentos como vigente, histórico, substituído ou obsoleto; não apagar resultados nem atribuir validação de um SHA a outro.
