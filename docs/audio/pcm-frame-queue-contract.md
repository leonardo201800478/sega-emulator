# Contrato OUT-A — FIFO PCM em frames

**Escopo:** primeiro incremento executável do transporte de áudio host.
**Unidade:** um frame = duas amostras PCM16 intercaladas [left, right].
**Concorrência:** um produtor e um consumidor; cada endpoint pertence a uma única thread.
**Clock:** a fila não possui relógio e não controla o MachineScheduler.

## Contrato implementado

- Capacidade fixa definida em frames estéreo, não em quantidade de blocos variáveis.
- Publicação parcial: cada frame é aceito enquanto houver espaço.
- Overflow: política drop-newest; frames já enfileirados nunca são sobrescritos.
- Leitura parcial: o consumidor solicita qualquer quantidade de frames, sem exigir alinhamento com o lote do produtor.
- Underflow: os frames disponíveis são preservados; somente a região faltante é preenchida com [0, 0].
- Telemetria exata: frames aceitos, consumidos, descartados e silenciados; eventos de underrun; high-water mark.
- Operações de push/read não esperam e não alocam memória depois da construção da fila.
- O encerramento de um endpoint pode ser detectado pelo outro.

A implementação utiliza rtrb 0.3, que já é dependência do crate. Não há atualização de dependência nesta etapa.

## Fora do escopo desta etapa

- A fila ainda não substitui os transports Vec<u8> usados pelos backends WASAPI/XAudio2.
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
