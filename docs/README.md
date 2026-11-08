# Documentação do Sega Emulator

Este diretório reúne decisões normativas, contratos arquiteturais, estado operacional, auditorias técnicas e evidências de validação.

## Como interpretar a documentação

A documentação tem níveis distintos de autoridade. Em caso de conflito, aplicar esta ordem:

1. **ADRs normativas vigentes** — decisões arquiteturais aceitas.
2. **Roadmap mestre e políticas permanentes** — sequência de execução, gates e processo.
3. **Código e testes na `main`** — estado efetivamente integrado.
4. **Status e auditorias datadas** — fotografias vinculadas a um SHA específico.
5. **Documentos históricos, propostas e PRs não integradas** — contexto, não estado atual nem autorização para copiar código.

Uma PR aberta, Draft, branch local, teste em branch ou documentação proposta não altera o estado integrado da `main`.

## Documentos de entrada

| Documento | Finalidade |
|---|---|
| [Status atual do projeto](project-status-2026-11-07.md) | Fotografia histórica de estado em 2026-11-07 |
| [Status dos componentes](component-status.md) | Estado reconciliado da main em 2026-11-08 |
| [Scorecard de confiabilidade de áudio](audio/audio-reliability-scorecard.md) | Prioridades, métricas, gates e aceite de áudio SMS |
| [Auditoria consolidada de áudio](audits/audio-reliability-consolidated-2026-11-08.md) | Confronto das quatro auditorias e evidências |
| [Roadmap mestre](project-roadmap.md) | Sequência normativa das fases e critérios de avanço |
| [Escopo do produto](project-scope.md) | O que pertence ao produto ativo e o que está fora da fase atual |
| [Política de desenvolvimento e validação](development-and-validation-policy.md) | Método de engenharia, fidelidade e gates |
| [Política de CI](ci-validation-policy.md) | Checks obrigatórios e classificação de resultados |
| [Auditoria do código](codebase-audit.md) | Auditoria histórica; não usar como fotografia operacional atual |

## Arquitetura e decisões

- [ADRs normativas do produto](adr/README.md)
- [Arquitetura — visão geral](architecture.md)
- [ADRs de arquitetura](architecture/adr/README.md)
- [Scheduler — contratos, integração e validação](architecture/scheduler/README.md)
- [Runtime SMS e fronteiras nativas](architecture/native-rust-crates.md)
- [Autoridade temporal Rust](architecture/rust-temporal-authority.md)
- [Política de fidelidade observável](architecture/adr/0016-observable-fidelity-and-proportional-timing.md)

## Z80

- [Scorecard de conclusão](z80-completion-scorecard.md)
- [Matriz semântica DD/FD](z80-dd-fd-semantic-matrix.md)
- [Auditoria de opcodes ED](z80-ed-opcode-audit.md)
- [Comportamentos não documentados](z80-undocumented-behavior-research.md)
- [Especificação do manual Zilog](z80-zilog-manual-specification.md)

O fechamento semântico E7 precede E8. ZEXDOC/ZEXALL não podem ser marcados como PASS sem execução de um harness Rust completo e evidência no SHA correspondente.

## Evidências e auditorias

- `evidence/`: resultados reproduzíveis de testes, com SHA, comando, ambiente, resultado e limitações.
- `audits/`: investigações de bugs, compatibilidade, PRs e subsistemas.
- `hardware/`: modelos e referências técnicas de chips e plataformas.
- `milestones/`: marcos históricos.
- `releases/`: notas de release.

Consulte [evidências de validação](evidence/README.md) e [auditorias](audits/README.md).

## Documentação histórica e obsoleta

Documentos sobre Python, PySide6/Qt, PyO3, Maturin, workspaces antigos ou fotografias anteriores à migração foram preservados quando registram decisões, evidências ou contexto útil. Eles não descrevem a arquitetura ativa. Não excluir evidência histórica apenas por estar obsoleta; marque-a como histórica e remova-a somente quando for duplicada, sem referências e sem valor de auditoria.

A arquitetura de produto vigente é Rust + egui/eframe, com SMS temporal como primeiro runtime. Consulte ADR 0010 e ADR-0016 antes de propor mudanças.
