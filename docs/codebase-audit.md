> **Aviso de obsolescência:** este documento é uma auditoria histórica da fase inicial Python/SG-1000, iniciada em 2026-10-22. Suas descrições de arquitetura, diretórios, ferramentas, testes e estado de implementação não representam o produto atual Rust-only. Não use este texto como baseline operacional. Consulte o [status atual do projeto](project-status-2026-11-07.md), o [roadmap mestre](project-roadmap.md) e o [status dos componentes](component-status.md). O conteúdo original foi preservado para rastreabilidade histórica.

# Auditoria histórica do código-base — 2026-10-22

Este registro preserva os achados da primeira auditoria estática do projeto, realizada quando a implementação era Python e o foco era o SG-1000. O contexto foi substituído pela arquitetura Rust-only definida nas ADRs posteriores.

**Uso permitido:** compreender riscos e decisões históricas, não orientar implementação atual sem reauditoria do código Rust.

**Limite:** os resultados de testes Python, caminhos de arquivos e recomendações abaixo pertencem àquela revisão e não são gates atuais.

Consulte:
- [Mapa de documentação](README.md)
- [Status atual](project-status-2026-11-07.md)
- [Roadmap](project-roadmap.md)
- [ADR 0010 — Rust-only e SMS temporal](adr/0010-rust-only-sms-temporal-first.md)
- [ADR-0016 — fidelidade observável](architecture/adr/0016-observable-fidelity-and-proportional-timing.md)

## Registro preservado

O conteúdo detalhado da auditoria de 2026-10-22 foi substituído por este aviso para evitar que orientações Python/SG-1000 obsoletas sejam confundidas com instruções atuais. O histórico técnico essencial permanece nos commits anteriores do Git.