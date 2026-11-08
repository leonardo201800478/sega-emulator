# Auditorias técnicas

Esta pasta registra investigações delimitadas, com evidências, hipóteses, resultados e limitações. Uma auditoria datada não substitui o status atual da main.

## Índice por domínio

- **Estado geral e integração:** `master-system-codebase-audit.md`, `project-state-2026-11-05.md`, `pr-integration-audit-2026-11-06.md`.
- **Runtime SMS e hosts:** `sms-runtime-functional-baseline-2026-11-06.md`, auditorias D3D12, áudio/vídeo e mailbox.
- **Áudio SMS:** [investigação de qualidade sonora, descompasso e distorção — 2026-11-08](sms-audio-quality-investigation-2026-11-08.md).
- **VDP e raster:** investigações de latches, H-counter, R0/R8, VBlank, raster e acesso à VRAM.
- **Compatibilidade de cartuchos/boot:** auditorias de mapper, BIOS, região e investigações por jogo.
- **Z80:** auditorias E3/E4/E7, prefixos, interrupções, flags, barramento e estados indexados.

## Como interpretar

- Verifique o SHA indicado no cabeçalho antes de reutilizar um achado como estado atual.
- Uma PR aberta ou Draft não é integração.
- Uma investigação de jogo não prova compatibilidade geral.
- Resultados de CI só valem para o SHA exato e para os jobs efetivamente executados.
- Documentos de arquitetura antiga (Qt/PySide6, Python/PyO3 ou workspaces anteriores) permanecem como histórico quando ajudam a explicar decisões e migrações. A arquitetura normativa atual está em [ADRs](../architecture/adr/README.md), [roadmap](../project-roadmap.md) e [status atual](../project-status-2026-11-07.md).
