# T5a — índices M-cycle/T-state do microsequenciador Z80

**Estado:** registro de implementação de uma fatia específica; não é requisito de expansão universal.  
**Escopo original:** índices M-cycle/T-state serializáveis para as operações já expostas pelo microsequenciador.

## Interpretação vigente

Os índices implementados podem ser úteis para diagnóstico, suspensão e retomada das transações que realmente utilizam esse contrato. Sua existência não obriga a:
- ampliar o trace a todas as instruções;
- representar todas as bordas elétricas ou subfases de clock;
- modelar todos os pinos do Z80;
- adicionar eventos por T-state em consoles que não observam essas diferenças;
- implementar BUSREQ/BUSACK ou arbitragem sem consumidor de plataforma.

A extensão desses índices só deve ocorrer quando um teste de integração demonstrar que o console precisa da informação para produzir o resultado correto.

Consulte [ADR-0016 — fidelidade observável](adr/0016-observable-fidelity-and-proportional-timing.md) e [scorecard funcional do Z80](../z80-completion-scorecard.md).

## Registro histórico

O trabalho original documentou ownership da continuação no Z80MicroSequencer, identificação das transações e preservação dos índices no snapshot para o subconjunto suportado. Os critérios e resultados registrados abaixo desta nota devem ser lidos como evidência daquela fatia, não como uma meta de cobertura temporal integral.
