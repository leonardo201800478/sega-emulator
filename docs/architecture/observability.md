# Arquitetura de logging e observabilidade

## Objetivos

O logging é transversal aos crates compartilhados e não participa do controle temporal da máquina. CPU, VDP, PSG, Scheduler, barramentos e hosts publicam eventos estruturados; a aplicação nativa instala o subscriber e mantém o ciclo de vida do escritor.

O caminho de hardware não executa operações de disco. Eventos são enviados a uma fila limitada e processados por uma thread de escrita independente.

## Bibliotecas e versões

| Biblioteca | Versão | Responsabilidade |
|---|---:|---|
| `tracing` | 0.1.44 | API estruturada de eventos e spans nos crates |
| `tracing-subscriber` | 0.3.23 | Formatação JSON, `EnvFilter` e reload de filtros |
| `tracing-appender` | 0.2.5 | Fila não bloqueante e worker de escrita |
| `serde` / `serde_json` | 1.0.229 / 1.0.154 | Snapshot de diagnóstico JSON |

As dependências de subscriber e appender são opcionais e habilitadas pela feature `runtime` do crate `sega-observability`. Os crates de hardware usam somente a fachada de `tracing`; não carregam o subscriber nem o worker de arquivo.

## Pipeline

```text
Z80 / Scheduler / VDP / PSG / Audio / D3D12 / ROM / GUI
                         |
                 tracing events
                         |
                EnvFilter (reload)
                         |
                bounded NonBlocking
                lossy queue + counter
                         |
                 writer thread
                         |
          rotating JSON-lines log file
                         |
             recent-event ring buffer
                         |
              Diagnostics / export
```

A fila tem capacidade padrão de 16.384 linhas e usa política `lossy`: quando saturada, o produtor não aguarda disco; o evento é descartado e o contador `dropped_lines` é incrementado. O contador deve ser exibido no painel Diagnostics. A fila e o ring buffer são limitados.

O ring buffer mantém os 256 eventos mais recentes para consulta pela GUI. Ele é atualizado pela thread de escrita, não pelo thread de CPU ou Scheduler.

## Arquivos e rotação

No Windows, o diretório padrão é:

`%LOCALAPPDATA%\\Sega Emulator\\logs\\`

Arquivos:

- `sega-emulator.log`: JSON Lines, rotação por tamanho (10 MiB por arquivo, cinco backups).
- `sega-emulator-diagnostics.json`: snapshot da sessão escrito no encerramento ou na exportação.
- `sega-emulator-crash.log`: panic hook, com timestamp, payload e backtrace.

`SEGA_LOG_DIR` permite substituir o diretório e `SEGA_LOG_FILTER` define o filtro inicial. O filtro pode ser recarregado em runtime por `LoggingHandle::set_filter`.

O `WorkerGuard` permanece vivo durante a execução. No encerramento normal, a aplicação registra o evento final, descarta o guard para aguardar o worker e então grava o snapshot JSON.

## Níveis e filtros

Níveis suportados: ERROR, WARN, INFO, DEBUG e TRACE.

O padrão é `info`. Eventos de alta frequência, como despacho individual do Scheduler, escrita de registradores PSG, submissão de buffers e consumo de amostras, usam TRACE e não são emitidos no perfil normal.

Exemplos de filtros:

```text
info
info,sega::z80=debug,sega::scheduler=trace
warn,sega::audio::backend=trace
debug,sega::vdp=trace,sega::mapper=trace
```

Não registrar cada instrução Z80 nem cada T-state por padrão. A instrumentação detalhada deve ser ativada somente para o subsistema investigado.

## Taxonomia de targets

| Target | Conteúdo |
|---|---|
| `sega::app` | inicialização, encerramento e metadados da sessão |
| `sega::z80` | reset, interrupções e exceções de execução |
| `sega::scheduler` | domínios de clock, eventos causais e violações temporais |
| `sega::vdp` | configuração, modo, VBlank e Line IRQ |
| `sega::psg` | configuração, registradores e fila de amostras |
| `sega::audio::dsp` | resampling, volume e lotes de amostras |
| `sega::audio::backend` | WASAPI/XAudio2, buffers, underruns, descartes e reconexões |
| `sega::video::d3d12` | adaptador, device, swapchain, HWND e falhas de Present |
| `sega::rom` | formato e tamanho da imagem |
| `sega::mapper` | mapper, BIOS, memória e trocas de banco |
| `sega::machine` | perfil e clocks da máquina |
| `sega::io` | operações de I/O selecionadas |

## Privacidade e determinismo

- Não registrar bytes da ROM, BIOS, memória do jogo ou conteúdo protegido.
- Não registrar caminhos absolutos de ROMs ou diretórios pessoais.
- A identificação de ROM deve usar metadados não sensíveis (formato, tamanho e, quando integrado, hash).
- A fila é limitada e lossy para impedir espera de I/O no caminho de hardware.
- O logging não agenda eventos, não avança clocks e não altera WAIT states.
- O timestamp é gerado no subscriber; não é usado como fonte temporal do emulador.
- A política de descarte fica observável pelo contador de eventos perdidos.

## Testes e validação

O crate contém testes para rotação por tamanho, retenção de backups e ring buffer. A suíte com feature `runtime` deve ser executada explicitamente:

`cargo test --manifest-path native/observability/Cargo.toml --features runtime --locked`

Além dos testes, a aplicação deve ser validada em Windows para confirmar os caminhos de escrita, rotação, encerramento e captura de panic.

## Entregas seguintes da sprint

Esta fundação disponibiliza inicialização, persistência, rotação, filtro recarregável, ring buffer, contador de descarte e snapshot JSON. As próximas entregas devem concluir:

1. Instrumentação de GUI e alterações de configuração.
2. Painel Diagnostics com filtros por target, níveis, eventos recentes e contadores.
3. Exportação de pacote ZIP contendo logs, snapshot e configuração sanitizada.
4. Métricas de áudio/vídeo/Scheduler com distinção entre valores instantâneos, acumulados e estimados.
5. Benchmarks reproduzíveis de Debug, Debug otimizado e Release.

## Referências oficiais

- [tracing 0.1.44](https://docs.rs/tracing/0.1.44/tracing/)
- [tracing-subscriber 0.3.23 — EnvFilter e reload](https://docs.rs/tracing-subscriber/0.3.23/tracing_subscriber/)
- [tracing-appender 0.2.5 — non-blocking writer](https://docs.rs/tracing-appender/0.2.5/tracing_appender/non_blocking/)
