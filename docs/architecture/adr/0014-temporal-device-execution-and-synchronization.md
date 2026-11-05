# ADR-0014 — contratos de execução e sincronização temporal de dispositivos

**Estado:** Proposta de referência, subordinada à [ADR-0016 — fidelidade observável](0016-observable-fidelity-and-proportional-timing.md).  
**Data original:** 2026-11-02.

Esta ADR descreve conceitos para coordenar dispositivos temporizados em máquinas compostas. Não impõe execução por tick, subfase, evento por pino ou microsequenciamento universal. Sua aplicação depende da necessidade observável de cada plataforma.

## Escopo útil preservado

- Um único proprietário temporal por instância de máquina.
- Domínios de clock quando componentes realmente utilizam fontes/divisores distintos.
- Execução determinística e causal entre CPU, barramento, vídeo, áudio e periféricos.
- Avanço limitado quando uma operação depende de outro dispositivo.
- Snapshot/restore do estado necessário à retomada.
- Separação entre tempo emulado e relógio/pacing do host.

## Limites

- Não modelar clocks, fases, estados ou eventos que não alterem o comportamento observável.
- Não criar um evento por T-state, pixel ou pino como regra geral.
- Não adicionar dispositivos futuros ao runtime antes da fase correspondente do roadmap.
- Não tratar o exemplo YM2413 ou qualquer outro componente futuro como autorização para antecipar sua implementação.
- Não exigir simulação elétrica/analógica.

## Regra de aplicação

Para cada plataforma, documentar o componente consumidor, a interação observável, a menor granularidade suficiente e os testes que demonstram a necessidade. Se uma execução em blocos preservar o resultado, preferir a solução em blocos.

A implementação e a validação devem ser verificadas no código e no SHA atual. Esta ADR é uma proposta arquitetural, não uma declaração de implementação concluída.
