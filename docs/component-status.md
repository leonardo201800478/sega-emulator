# Estado atual dos componentes — Sega Emulator

**Baseline consultada:** main em f4e428ad31c0d2e214e8ed15aab8fa705b1c795e (2026-11-05).  
**Política de fidelidade:** [ADR-0016](architecture/adr/0016-observable-fidelity-and-proportional-timing.md).  
**Prioridade:** Master System, mantendo SG-1000 funcional e sem regressões.

Este documento é a fotografia resumida do estado conhecido na baseline acima. Antes de implementar, atualizar a auditoria para o SHA corrente e verificar código, testes, PRs e workflows.

## 1. Plataformas

| Plataforma | Estado | Diretriz |
|---|---|---|
| SG-1000 | Runtime Rust integrado | Preservar funcionalidades e jogos já validados |
| Master System | Runtime Rust integrado | Prioridade atual; ampliar compatibilidade funcional e corrigir divergências observáveis |
| Game Gear | Não concluído como plataforma independente | Fase futura, reutilizar SMS quando compatível |
| Mega Drive / Genesis | Não concluído | Fase futura, CPU 68000 e Z80 auxiliar |
| Sega CD / 32X | Não concluídos | Dependem das plataformas anteriores |

## 2. Componentes

| Componente | Estado conhecido | Prioridade / limite |
|---|---|---|
| Z80 compartilhado | Implementado em Rust e consumido pelos perfis SG-1000/SMS | Corrigir semântica e regressões; não duplicar o core |
| Z80 E5 / E6 | Consta como integrado na main nas auditorias recentes | Confirmar testes no SHA candidato antes de novas alterações |
| Z80 E7 | Parcial; E7.3/HALT T4 consta como integrada | Revisar somente comportamentos relevantes aos consoles suportados |
| Z80 M-cycle/T-state | Há contratos e fatias temporais no runtime | Não exigir microsequenciamento integral de todas as instruções |
| WAIT / arbitragem | Dependem do circuito e dos consumidores de cada plataforma | Implementar somente quando houver ligação e efeito demonstrados |
| Memória / mappers SMS | Implementações Rust presentes | Corrigir compatibilidade de cartuchos e mapas reais |
| VDP SMS | Implementação Rust com Mode 4 e modos legados | Refinar raster/registradores quando jogos demonstrarem divergências |
| VDP TMS9918A | Runtime SG-1000 em Rust | Preservar compatibilidade e particularidades próprias |
| PSG / áudio | Runtime de áudio Rust integrado | Validar comportamento audível e temporização necessária |
| Machine Scheduler | Infraestrutura Rust presente | Coordenar eventos relevantes sem impor eventos por pino/tick |
| PyO3 / Qt | Extensão nativa e frontend ativos | Manter interface coarse-grained; sem lógica de hardware em Python |

## 3. Modelagem temporal em pausa

Na consulta de 2026-11-05, as PRs #512, #514 e #526 estavam abertas/Draft e não integradas na main. A PR #527 também estava aberta/Draft e não integrada.

Essas propostas não são trabalho ativo. Não avançar em timeline racional, microsequenciamento universal, WAIT/TW genérico, BUSREQ/BUSACK ou representação de pinos sem primeiro identificar uma dependência observável de SMS ou SG-1000 e demonstrar que a menor solução funcional exige esse detalhe.

## 4. Validação

- Executar os gates Rust e Python no SHA candidato.
- Testar a funcionalidade afetada e executar regressões SMS/SG-1000.
- Usar ROMs de diagnóstico e referências independentes quando agregarem evidência.
- Medir desempenho quando houver alteração no caminho crítico.
- Classificar cada suíte como PASS, FAIL, BLOCKED ou NOT RUN; SKIPPED não é PASS.

Nenhuma validação global de cycle-accuracy ou pin-accuracy é requisito universal.

## 5. Fontes de autoridade

- [Roadmap mestre](project-roadmap.md)
- [Escopo do projeto](project-scope.md)
- [ADR-0016 — fidelidade observável](architecture/adr/0016-observable-fidelity-and-proportional-timing.md)
- [Scorecard funcional do Z80](z80-completion-scorecard.md)
- [Matriz semântica DD/FD](z80-dd-fd-semantic-matrix.md)

Auditorias datadas preservam evidências históricas; não devem ser usadas como status corrente sem reconciliação com a main.
