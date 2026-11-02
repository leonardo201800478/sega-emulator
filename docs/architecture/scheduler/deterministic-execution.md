# Execução determinística e invariantes do Scheduler

## 1. Definição operacional

Para a mesma configuração, estado inicial, sequência de entradas e perfil, a máquina deve produzir a mesma sequência observável de eventos, estados digitais e saídas digitais, independentemente de:
- ordem de criação de objetos;
- fragmentação dos intervalos de avanço;
- velocidade/latência do host;
- VSync e pacing;
- carga de CPU;
- ordem de execução de threads não temporais;
- serialização/restauração intermediária.

## 2. Fontes de não determinismo proibidas

- `f32/f64` para timestamps, razão, fase, contagem ou ordenação;
- `HashMap`/hash aleatório para ordem observável;
- relógio de parede ou timer do SO dentro do domínio;
- callbacks assíncronos de Python;
- threads por dispositivo que avancem estado sem coordenação;
- payloads de evento com closures, ponteiros ou referências ao host;
- overflow com wrap ou saturação silenciosa;
- ignorar eventos vencidos ou normalizar estado inválido;
- processamento em lote que atravesse um evento externo.

## 3. Invariantes

1. Tempo monotônico.
2. Razões válidas e fase dentro do denominador.
3. Avanço chunk-invariant.
4. Domínios avançados em ordem canônica.
5. Eventos ordenados por chave completa.
6. Nenhum evento antes do watermark.
7. Delta budget nunca descarta evento.
8. Falha de aritmética não deixa avanço parcial.
9. Restore válido equivale à continuação sem restore.
10. Codec rejeita truncamento, dados extras, versão desconhecida e checksum inválido.
11. Mesma entrada produz mesmo trace e estado final.
12. A saída digital não depende do host de áudio/vídeo.

## 4. Avanço em lote

Avanço agregado é permitido somente até o próximo limite observável e deve demonstrar equivalência com um modelo de referência mais granular. A prova precisa cobrir:
- contagem e fase;
- eventos internos gerados;
- estados intermediários observáveis;
- interações com outros dispositivos;
- overflow e limites;
- save/restore no meio do span.

Não basta comparar somente o estado final se eventos intermediários puderem alterar outro componente.

## 5. Replay e diagnóstico

Trace deve registrar no mínimo timestamp, fase, prioridade, sequência, tipo/payload estável, domínio e estado/avanço associado quando aplicável. O comparador deve reportar a primeira divergência, não apenas um hash final.

Replays devem fixar seed, configuração, ROM/homebrew de teste permitido, entradas, versão do perfil, SHA e versão dos codecs. Não versionar ROMs comerciais nem dumps protegidos.

## 6. Fronteira do host

O host pode atrasar a apresentação, mas não pode atrasar ou acelerar a timeline lógica. Pacer consome métricas da emulação e solicita trabalho; não injeta ciclos em dispositivos. Áudio/vídeo são consumidores de buffers produzidos no tempo virtual.

Python é plano de controle e telemetria, nunca participante da execução temporal.
