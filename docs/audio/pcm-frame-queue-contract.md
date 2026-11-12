# Contrato OUT-A — FIFO PCM em frames

**Escopo:** primeiro incremento executável do transporte de áudio host.
**Unidade:** um frame = duas amostras PCM16 intercaladas [left, right].
**Concorrência:** um produtor e um consumidor; cada endpoint pertence a uma única thread.
**Status:** contrato inicial OUT-A; extensões posteriores de flush/epoch estão registradas ao final deste documento.
**Clock:** a fila não possui relógio e não controla o MachineScheduler.

## Contrato implementado

- Capacidade fixa definida em frames estéreo, não em quantidade de blocos variáveis.
- Publicação parcial: cada frame é aceito enquanto houver espaço.
- Overflow: política drop-newest; frames já enfileirados nunca são sobrescritos.
- Leitura parcial: o consumidor solicita qualquer quantidade de frames, sem exigir alinhamento com o lote do produtor.
- Underflow: os frames disponíveis são preservados; somente a região faltante é preenchida com [0, 0].
- Telemetria exata: frames aceitos, consumidos, descartados e silenciados; eventos de underrun; high-water mark.
- Extensão posterior: frames descartados deliberadamente em flush de epoch são contados separadamente em `flushed_frames`.
- Operações de push/read não esperam e não alocam memória depois da construção da fila.
- O encerramento de um endpoint pode ser detectado pelo outro.

A implementação utiliza rtrb 0.3, que já é dependência do crate. Não há atualização de dependência nesta etapa.

## Fora do escopo desta etapa

- A fila frame-based é o transporte usado pelo adaptador CPAL; os transports Vec<u8> específicos de WASAPI/XAudio2 foram removidos da implementação candidata.
- Não há flush concorrente nem comando de mute nesta etapa.
- Não há prebuffer ou política de retomada.
- Não há mudança no worker SMS, na sessão DSP, nos chips ou no Scheduler.

Essas operações exigem um protocolo de controle/epoch para que limpar áudio pendente não introduza uma segunda thread produtora ou viole a propriedade SPSC. Serão tratadas na integração subsequente, com transições testadas separadamente.

## Testes adicionados

1. capacidade zero rejeitada;
2. preservação de ordem com tamanhos distintos de escrita/leitura;
3. overflow drop-newest com contagem exata;
4. underflow preenchendo somente os frames faltantes;
5. leitura vazia e leitura de comprimento zero;
6. encerramento do produtor após drenar os frames restantes;
7. detecção de consumidor desconectado.

## Validação pendente

Os testes foram adicionados ao código, mas ainda dependem da execução do CI no commit desta branch. Não declarar PASS até o workflow concluir.
## API pública

A construção é feita por `pcm_frame_queue(capacity_frames)`, que retorna os endpoints exclusivos `PcmFrameProducer` e `PcmFrameConsumer`. A capacidade zero retorna `PcmFrameQueueError::ZeroCapacity`; após a construção, o produtor publica com `push_frames` e o consumidor preenche buffers com `read_frames`.

## Extensão posterior — flush por epoch

O contrato de OUT-A foi estendido sem violar a propriedade SPSC:

- `PcmFrameConsumer::discard_queued_frames()` é chamado exclusivamente pela thread consumidora durante uma barreira de flush confirmada.
- O número de frames descartados é acumulado em `flushed_frames`; não é somado a `consumed_frames` nem a `dropped_frames`.
- O produtor não limpa nem reposiciona o cursor da FIFO.
- O worker SMS atualiza o snapshot de métricas após Start/Pause/Stop/Reset/Load ROM, depois de a operação do backend retornar.
- A telemetria distingue flush da FIFO, overflow e underrun; não há métricas específicas de endpoint XAudio2 no contrato genérico.

A extensão é coberta por testes unitários no crate de saída. A validação em dispositivo físico continua separada.
