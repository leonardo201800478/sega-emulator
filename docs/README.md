# Documentação do Sega Emulator

Este diretório reúne decisões normativas, contratos arquiteturais, roadmap, sprints, estado operacional, auditorias e evidências.

## Precedência

1. ADRs vigentes: ADR 0010 (runtime/hardware), ADR 0011 (desktop/hosts) e ADR-0016 (fidelidade temporal).
2. Escopo, roadmap e políticas permanentes.
3. Código/testes integrados na main.
4. Status datados vinculados a SHA.
5. PRs não integradas e documentos históricos.

PR Draft, branch, documentação ou CI parcial não alteram o estado integrado da main.

## Documentos principais

- [README do projeto](../README.md)
- [Escopo](project-scope.md)
- [Roadmap mestre](project-roadmap.md)
- [Sprint R1](sprints/2026-11-r1-slint-wgpu-vulkan-poc.md)
- [Política de CI e validação](ci-validation-policy.md)
- [Status 2026-11-11](project-status-2026-11-11.md)
- [Instruções permanentes](project-instructions-prompt.md)
- [ADR 0011 — Slint/WGPU/CPAL](adr/0011-slint-wgpu-cpal-desktop-platform.md)
- [ADRs normativas](adr/README.md)
- [Arquitetura geral](architecture.md)
- [Scheduler](architecture/scheduler/README.md)
- [Fidelidade temporal](architecture/adr/0016-observable-fidelity-and-proportional-timing.md)

## Áudio

- [Plano Modern Hi-Fi — SMS e SG-1000](audio/modern-hifi-audio-roadmap.md)
- [Estudo de reconstrução PSG H2](audio/psg-h2-reconstruction-study.md)
- [Checkpoint de continuidade H2](audio/modern-hifi-h2-continuity-checkpoint.md)

## Z80

- [Scorecard](z80-completion-scorecard.md)
- [Checklist mestre](z80-master-audit-checklist.md)
- [Matriz DD/FD](z80-dd-fd-semantic-matrix.md)
- [Auditoria ED](z80-ed-opcode-audit.md)
- [Comportamentos não documentados](z80-undocumented-behavior-research.md)

E7 deve estar demonstravelmente fechado antes de E8/ZEXDOC/ZEXALL.

## Evidências e histórico

evidence/ contém resultados reproduzíveis; audits/ investigações; hardware/ referências; milestones/ marcos históricos; releases/ notas; sprints/ objetivos e critérios temporários.

Documentos de implementações anteriores permanecem como evidência histórica auditável. Não os tratar como arquitetura-alvo quando conflitarem com ADR 0011. A retirada do código transitório exige R3/R5, inventário de consumidores e CI verde; não apagar evidências históricas nem reescrever resultados passados.
