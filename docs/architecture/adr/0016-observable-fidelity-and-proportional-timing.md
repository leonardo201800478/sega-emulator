# ADR-0016 — fidelidade observável e precisão proporcional ao console

- **Estado:** Aceito.
- **Data:** 2026-11-05.
- **Substitui:** ADR-0011 como diretriz global de precisão cycle-accurate.
- **Escopo:** todos os consoles Sega suportados pelo projeto.
- **Prioridade:** comportamento correto, temporização necessária, compatibilidade e desempenho.

## Contexto

O projeto acumulou propostas que tratavam a reprodução integral de M-cycles, T-states, fases de clock, sinais físicos, pinos e pipelines internos como requisitos universais. Essa abordagem amplia custo, estado, eventos, serialização e trabalho de validação mesmo quando o console ou os jogos não observam tais detalhes.

O objetivo do Sega Emulator é emular consoles com fidelidade comportamental e temporal próxima ao hardware original. Não é construir uma réplica elétrica, analógica ou transistor a transistor dos componentes.

## Decisão

A fidelidade será definida pelo comportamento observável do console e pelos requisitos reais de seus jogos, periféricos e dispositivos.

1. **Semântica funcional:** implementar corretamente as instruções e operações que o software do console pode executar. Particularidades não documentadas só serão adicionadas quando houver evidência de impacto em software ou hardware suportado.
2. **Temporização suficiente:** preservar clocks, relações de frequência, ordem causal e sincronização entre componentes na granularidade necessária para reproduzir o comportamento observável.
3. **Precisão seletiva:** M-cycles, T-states, WAIT, fases de borda, arbitragem, IRQs e detalhes de barramento serão modelados quando uma característica concreta do console depender deles. Não são requisitos universais por si só.
4. **Sinais físicos:** não é necessário representar todos os pinos do encapsulamento nem atualizar um vetor elétrico completo a cada clock. Uma interface de sinais pode existir quando um periférico ou barramento realmente consumir esses sinais.
5. **Sem simulação elétrica:** capacitância, tensão, ruído, carga, propagação analógica e nível de transistor estão fora do escopo, salvo se uma futura funcionalidade de produto justificar explicitamente um modelo específico.
6. **Granularidade eficiente:** dispositivos podem avançar em blocos, scanlines, amostras ou eventos, desde que não atravessem uma interação observável e preservem o resultado esperado. Não criar eventos por tick/pixel/pino sem necessidade comprovada.
7. **Estado e save states:** serializar o estado necessário para retomar a execução de forma determinística. Não persistir microestados ou sinais que não participem do modelo ativo.
8. **Desempenho:** estruturas compactas, baixo overhead e ausência de alocações/eventos desnecessários no caminho crítico. Otimização deve ser medida e não pode alterar o comportamento emulado.
9. **Evidência proporcional:** cada detalhe adicional deve ter uma justificativa ligada a documentação, medição, teste independente, jogo/periférico afetado ou divergência reproduzível.
10. **Sem falsa alegação:** declarar com precisão o que foi implementado e validado. Não usar “cycle-accurate”, “pin-accurate” ou equivalência elétrica como rótulo global sem demonstração específica e necessária.

## Aplicação ao Z80

O Z80 compartilhado deve executar corretamente o conjunto de instruções necessário aos consoles suportados e preservar flags, registradores, interrupções e efeitos arquiteturais relevantes.

- A cobertura de instruções não deve ser confundida com a obrigação de reproduzir todas as fases internas de cada opcode.
- M-cycles/T-states detalhados serão mantidos onde necessários à interação com o barramento, VDP, áudio, interrupções ou periféricos do console.
- WAIT será implementado quando a plataforma efetivamente o utilizar. Por exemplo, a ligação READY → /WAIT documentada em determinadas placas SG-1000 é uma dependência real a considerar.
- BUSREQ/BUSACK, daisy chain, sinais M1/RFSH e demais linhas só entram no runtime quando um console ou periférico suportado depender deles.
- A observação de pinos, quando útil para diagnóstico, será derivada do estado real do core e não constituirá uma segunda implementação de CPU.

## Critério para adicionar detalhe

Antes de introduzir uma nova camada de precisão, responder:

1. Qual console, revisão, jogo ou periférico depende desse comportamento?
2. Qual é o efeito observável se ele não for modelado?
3. Qual fonte ou teste demonstra a dependência?
4. Qual é a menor granularidade que reproduz o comportamento?
5. Qual o custo em CPU, memória, eventos, snapshots e manutenção?
6. Como será validado sem transformar uma hipótese em requisito universal?

Se não houver dependência observável ou evidência suficiente, o detalhe não será implementado como requisito. A incerteza pode ser registrada sem criar um bloqueio de arquitetura.

## Arquitetura preservada

- Runtime de hardware, barramentos, CPU, áudio, vídeo e scheduler permanecem em Rust.
- Python/PySide6 permanece restrito à aplicação, interface, configuração, diagnóstico e tooling.
- PyO3 continua coarse-grained, sem callbacks por instrução.
- O Scheduler mantém a autoridade temporal necessária à máquina composta, sem impor um evento para cada tick de todos os componentes.
- Componentes compartilhados continuam únicos quando representam o mesmo hardware; diferenças reais permanecem nos perfis/adaptadores de plataforma.
- JIT, frame skipping, hacks temporais e otimizações especulativas continuam proibidos.

## Consequências

- A arquitetura deixa de exigir um modelo universal cycle-accurate ou pin-level.
- Propostas de timeline racional, fases sub-T-state, traces completos e pipelines internos deixam de ser gates globais; só serão retomadas se uma dependência concreta justificar o custo.
- Testes funcionais, regressões, compatibilidade de jogos e sincronização relevante passam a orientar a prioridade.
- Documentos antigos de precisão integral passam a ser históricos ou serão removidos quando duplicados/inaplicáveis.
- A documentação específica de hardware continua válida como fonte de comportamento; não autoriza automaticamente implementar todos os detalhes descritos.

## Referências

- [Escopo do projeto](../../project-scope.md)
- [Roadmap mestre](../../project-roadmap.md)
- [Estado dos componentes](../../component-status.md)
- [SMS VDP — estado e comportamento implementado](../sms-vdp.md)
- [Perfis de hardware Z80 — referência](../../hardware/z80-silicon-platform-profiles.md)
