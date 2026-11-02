# Snapshot, restore e codec do Scheduler

## 1. Escopo

O Scheduler oferece snapshot do estado temporal que ele próprio possui. Isso não equivale a um save state completo da máquina.

## 2. Estado preservado

O DTO `SchedulerState<T>` atual inclui:
- versão do formato;
- `now`;
- domínios em ordem crescente de ID;
- estado da fila de eventos;
- limite delta;
- timestamp e quantidade do último despacho.

Cada `ClockDomainState` preserva ID, numerador/denominador, tempo atual, ciclos totais, fase, origem de registro e fase inicial. A fila preserva eventos pendentes em ordem canônica, próximo sequence e watermark completo.

## 3. Restore validado e atômico

Restore deve validar:
- versão suportada;
- limite delta não zero;
- ordem estrita de IDs;
- timestamps consistentes com `now`;
- histórico de razão/fase/ciclos;
- eventos não anteriores a `now`;
- watermark não posterior a `now`;
- consistência entre timestamp/count de dispatch e watermark;
- ordem canônica, sequência única e allocator válido.

Reconstruir domínios e fila em valores temporários; só publicar Scheduler após validação completa. Nunca deixar objeto parcialmente restaurado.

## 4. Codec binário atual

Frame:
`magic[8] | state_version[u16 BE] | payload_version[u16 BE] | body_length[u64 BE] | body | crc32[u32 BE]`

- magic: `SEGSCHD\0`;
- timestamps, ciclos, fases e demais campos temporais `u128`: 16 bytes unsigned big-endian;
- contagens de domínios/eventos e comprimentos de payload: `u32` big-endian;
- sequence, delta-event limit, dispatch count e body length: `u64` big-endian;
- versão do DTO e versão do codec de payload independentes;
- payload de evento codificado por `EventPayloadCodec<T>`;
- CRC-32/ISO-HDLC é calculado somente sobre `body` (não cobre magic, versões ou `body_length`); detecta corrupção acidental no corpo, mas não é autenticação criptográfica.

O codec de payload deve ser determinístico, versionado e rejeitar bytes inválidos. Não persistir tipos de dispositivo por meio de suposições genéricas.

## 5. Compatibilidade

Mudança incompatível no DTO exige incrementar versão do estado e política explícita de migração/rejeição. Mudança independente no payload exige atualizar sua versão. Nunca reinterpretar bytes antigos silenciosamente.

## 6. Save state da máquina

A máquina precisa salvar também:
- estado arquitetural e microsequencer de cada CPU;
- transações pendentes e fase de WAIT;
- estado completo de VDP/PSG/FM e pipelines;
- latches, IRQs, DMA e arbitragem;
- memória e mappers;
- entradas sincronizadas;
- eventos produzidos mas ainda não consumidos;
- estado dos codecs e versão do perfil.

A restauração da máquina deve ser transacional e validar referências cruzadas. Uma transação pendente deve ser aplicada exatamente uma vez.

## 7. Prova de equivalência

Para cada checkpoint:
1. executar até checkpoint;
2. exportar estado;
3. continuar sem interrupção e capturar trace/estado;
4. restaurar em instância independente;
5. continuar com as mesmas entradas;
6. comparar trace, sequência, ciclos/fases e estado final.

Cobrir checkpoint em evento pendente, delta budget esgotado, fase fracionária, transação de barramento pendente e CPU suspensa assim que essas integrações existirem.
