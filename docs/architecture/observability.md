# Arquitetura de logging e observabilidade

## Objetivos

O logging é transversal aos crates compartilhados e não participa do controle temporal da máquina. CPU, VDP, PSG, Scheduler, barramentos e hosts publicam eventos estruturados; a aplicação nativa instala o subscriber e mantém o ciclo de vida do escritor.

**Coleta desativada por padrão (opt-in).** A aplicação não inicializa o subscriber, não cria o diretório de logs, não instala o panic hook e não grava snapshots enquanto o usuário não ativar explicitamente `Debugger > Coleta de logs`. A opção não é persistida: cada inicialização começa desligada.

A autorização é representada por um único `DiagnosticsConsent`, criado somente no callback de ação afirmativa do usuário e passado ao logger central e ao worker SMS. O worker não expõe setter booleano público para habilitar diagnósticos. O host Slint consulta `LoggingHandle::is_enabled()` para condicionar seus eventos e timers; a cópia do token mantida em memória serve apenas para autorizar workers substitutos durante a mesma sessão.

Ao revogar a coleta durante a execução, o gate compartilhado é fechado, o filtro global é recarregado para `off`, eventos pendentes são descartados, o ring buffer é limpo e o worker antigo é encerrado com o sink fechado. Uma nova autorização inicia uma sessão de coleta limpa. O trace do worker SMS também é desabilitado, seu collector é descartado e os buffers de métricas do host/runtime são limpos.

O caminho de hardware não executa operações de disco. Quando ativada, a coleta usa uma fila limitada e uma thread de escrita independente. Quando desativada, não há coleta de eventos de diagnóstico.

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
- `sega-emulator-diagnostics.json`: não é escrito automaticamente no encerramento; eventual exportação exige ação explícita do usuário.
- `sega-emulator-crash.log`: nome legado reservado à política de falha crítica de bootstrap; não deve ser usado por um panic hook global.

### Política de falha crítica de bootstrap — substitui a abordagem de panic hook

A política aprovada **não instala hook global de panic**. Panics de runtime pós-boot seguem o mecanismo padrão do Rust e não geram arquivos diagnósticos automáticos.

A única gravação automática permitida é um relatório mínimo quando um erro classificado de bootstrap impede a inicialização do emulador. O relatório é independente do logger e do consentimento, é criado somente no ramo de falha e contém categoria estática e timestamp. Não captura payload, backtrace, ROM/BIOS, memória, caminhos absolutos, diretórios pessoais ou conteúdo de sessão. Erros comuns de ROM, firmware, jogo e áudio após o boot não são falhas críticas de bootstrap.

**Estado de implementação:** a infraestrutura ainda contém o hook privado legado descrito no ADR 0012. Sua remoção e a implementação de `record_bootstrap_failure` são pendências P0; DP-0 não pode ser declarado concluído antes dessa correção e de seus testes.

`SEGA_LOG_DIR` permite substituir o diretório e `SEGA_LOG_FILTER` define o filtro inicial. O filtro pode ser recarregado em runtime por `LoggingHandle::set_filter`.

O `WorkerGuard` só é criado após o opt-in e permanece vivo durante a execução. No encerramento normal, a aplicação pode registrar o evento final se a coleta estiver ativa e então encerra o logger; não grava snapshot JSON automaticamente. Qualquer exportação de snapshot deve partir de uma ação explícita do usuário.

## Níveis e filtros

Níveis suportados: ERROR, WARN, INFO, DEBUG e TRACE.

Com a coleta ativada e sem `SEGA_LOG_FILTER` definido, o filtro inicial é `info,sega::audio::pacing=trace`. Enquanto desativada, o filtro é `off`. Eventos de alta frequência, como despacho individual do Scheduler, escrita de registradores PSG, submissão de buffers e consumo de amostras, usam TRACE.

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
| `sega::audio::output` | CPAL stream, formato/taxa negociados, FIFO, underruns, descartes e falhas de dispositivo |
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

## Política de panic e crash

A saída padrão de panic do Rust deve ser preservada e chamada exatamente uma vez, mesmo quando a coleta privada estiver desativada. A captura para `sega-emulator-crash.log` é independente da saída padrão: só pode ocorrer com consentimento ativo no instante do panic. Não capturar payload arbitrário nem backtrace bruto por padrão; não suprimir panic nem usar `catch_unwind` para mascarar falhas. Ver [ADR 0012](../adr/0012-panic-crash-diagnostics-policy.md).

## Testes e validação

O crate contém testes para rotação por tamanho, retenção de backups e ring buffer. A suíte com feature `runtime` deve ser executada explicitamente:

`cargo test --manifest-path native/observability/Cargo.toml --features runtime --locked`

A fatia DP0-F5 acrescenta testes do formatter puro, ausência de sentinelas, limite do arquivo, retenção pré-hook, falha de I/O e delegação em subprocessos. A aprovação exige Windows x64 Debug e Release. CI aprovado não substitui validação física de saída de console em todos os ambientes.

## Entregas seguintes da sprint

Esta fundação disponibiliza inicialização sob demanda, opt-in pela GUI, persistência, rotação, filtro recarregável, ring buffer, contador de descarte e snapshot JSON. As próximas entregas devem concluir:

1. Painel Diagnostics com filtros por target, níveis, eventos recentes e contadores.
2. Exportação de pacote ZIP contendo logs, snapshot e configuração sanitizada.
3. Métricas de áudio/vídeo/Scheduler com distinção entre valores instantâneos, acumulados e estimados.
4. Benchmarks reproduzíveis de Debug, Debug otimizado e Release.

## Referências oficiais

- [tracing 0.1.44](https://docs.rs/tracing/0.1.44/tracing/)
- [tracing-subscriber 0.3.23 — EnvFilter e reload](https://docs.rs/tracing-subscriber/0.3.23/tracing_subscriber/)
- [tracing-appender 0.2.5 — non-blocking writer](https://docs.rs/tracing-appender/0.2.5/tracing_appender/non_blocking/)
