# Diretiva obrigatória — autoridade temporal exclusiva em Rust

**Status:** decisão arquitetural normativa do projeto.  
**Escopo:** todos os sistemas, máquinas, CPUs, barramentos, dispositivos, periféricos e subsistemas atuais e futuros do Sega Emulator.  
**Runtime de hardware:** exclusivamente Rust.

## 1. Regra central

Todo controle do tempo emulado pertence exclusivamente ao runtime Rust. Isso inclui decidir quando a máquina avança, quantos ticks/ciclos são processados, a ordem dos eventos e quando cada componente observa ou produz efeitos.

**Python nunca pode ser autoridade temporal, de ciclo, de frame ou de sincronização da emulação.** A interface Qt/PySide6 pode solicitar comandos coarse-grained ao runtime Rust, mas não pode executar a lógica temporal nem determinar a progressão da máquina.

Esta regra é obrigatória para SG-1000, Master System, Game Gear, Mega Drive/Genesis, Sega CD, 32X e qualquer plataforma que venha a ser adicionada.

## 2. Responsabilidades exclusivas do runtime Rust

Rust é responsável por:

- relógio virtual da máquina, ticks, T-states, ciclos e domínios de clock;
- execução e avanço de CPU, micro-operações, barramento, memória, I/O, waits, contenção e arbitragem;
- scheduler único por instância de máquina, fila de eventos, prioridades, desempates e causalidade;
- sincronização de CPU(s), VDP(s), PSG/FM, áudio PCM/CDDA/PWM, DMA, interrupções e periféricos;
- raster, scanlines, dots/pixels, HBlank, VBlank, beam position e eventos temporais de vídeo;
- protocolos e transições temporizadas de periféricos, incluindo Pause/NMI, lightguns e correlação do disparo com o raster;
- timestamping no domínio emulado, conversão de eventos externos para a timeline e replay determinístico;
- produção de quadros e blocos de áudio associados ao tempo emulado;
- pausa, retomada, step, reset, load, stop e aplicação de comandos em safe points definidos pelo runtime;
- preservação do estado temporal em save states e restauração determinística.

A máquina deve ter um único proprietário do avanço temporal. Componentes podem possuir domínios de clock diferentes, mas não schedulers independentes que avancem hardware por conta própria.

## 3. Responsabilidades permitidas à interface Python/Qt

Python/PySide6 permanece restrito a:

- janela, menus, diálogos, apresentação e interação visual;
- configuração e persistência de preferências;
- seleção/carregamento de mídia e configuração de perfis;
- exibição de logs, métricas, diagnósticos e snapshots fornecidos pelo runtime;
- envio de comandos coarse-grained à API Rust, como criar/carregar máquina, pause, resume, reset, stop e solicitar uma operação de step;
- ferramentas de desenvolvimento, auditorias, automação e testes que exercitem o runtime Rust.

A interface pode solicitar uma operação; somente Rust decide como executá-la, em que ponto temporal, com quais efeitos e como avançar a máquina.

A fronteira PyO3/Maturin deve permanecer coarse-grained. Não introduzir callbacks Python por instrução, T-state, acesso ao barramento, pixel, evento de chip ou amostra de áudio.

## 4. Proibições explícitas em Python

É proibido usar Python, Qt ou bibliotecas Python para:

- avançar a emulação por chamadas periódicas a `run_frame()`, `step()` ou equivalentes;
- determinar a duração de ciclos, instruções, frames ou eventos;
- implementar ou arbitrar clocks, T-states, contadores temporais ou filas de eventos de hardware;
- sincronizar CPU, vídeo, áudio, DMA, interrupções ou periféricos;
- fazer pacing temporal da máquina por `QTimer`, `time.sleep()`, loops de FPS ou callbacks de frame;
- decidir quando o VDP gera HBlank/VBlank, quando o feixe alcança uma lightgun ou quando um periférico muda de estado;
- corrigir deriva do relógio emulado com base no relógio de parede ou no relógio do dispositivo de áudio;
- manter um scheduler, relógio ou fallback de hardware paralelo em Python.

Timers de UI podem ser usados exclusivamente para animações, atualização visual ou telemetria não autoritativa. Eles nunca podem disparar ou regular o avanço do hardware emulado.

## 5. Fronteira com relógios e eventos do host

O sistema operacional e os backends nativos podem fornecer timestamps, eventos de teclado/controle, sensores, áudio de captura, VSync ou notificações de dispositivos. Esses dados são entradas externas, não o relógio da máquina.

- A captura pode ocorrer em workers nativos com ownership, filas limitadas e sincronização explícitos.
- Eventos de host devem ser normalizados e entregues ao runtime Rust.
- Rust define a política determinística de conversão, ordenação, quantização e aplicação desses eventos na timeline emulada.
- Um timestamp de parede não pode avançar, retroceder ou reescalar o relógio virtual.
- VSync/VRR, apresentação física, WASAPI/XAudio2 e consumo de áudio podem regular a saída do host, mas não se tornam autoridades temporais do hardware.
- Workers de captura, áudio e apresentação não podem mutar diretamente registradores nem avançar a máquina.

Quando a evidência do hardware não justificar precisão sub-frame para uma entrada, a política de amostragem deve ser explícita, documentada e implementada em Rust. Não se deve inventar precisão temporal a partir de timestamps do host.

## 6. API de controle e execução

A interface deve operar por comandos e resultados de alto nível. Exemplos conceituais:

- `load_machine(...)`
- `pause()` / `resume()`
- `reset()` / `stop()`
- `step_instruction()` — solicitação de depuração; a execução e a contabilização temporal continuam em Rust.
- `run_until(...)` — quando exposto, o limite e o avanço são interpretados e aplicados pelo scheduler Rust.
- consulta de estado, métricas e snapshots imutáveis.

Esses nomes são exemplos de contrato, não uma exigência de API pública imediata. Não implementar wrappers que devolvam a autoridade temporal à interface.

## 7. Critérios obrigatórios de aceitação

- [ ] O scheduler Rust é o único proprietário do avanço temporal de cada máquina.
- [ ] Python não chama frames/instruções periodicamente para manter a emulação em execução.
- [ ] Nenhum `QTimer`, sleep, loop de FPS ou callback Python controla ciclos, frames ou sincronização do hardware.
- [ ] CPU, barramento, vídeo, áudio, interrupções e periféricos são coordenados pela timeline Rust.
- [ ] Eventos de host são normalizados na fronteira e aplicados pelo runtime segundo política temporal documentada.
- [ ] Workers de host não avançam a máquina nem alteram diretamente estado de hardware.
- [ ] Pausa, step, reset, load e stop são comandos aplicados por Rust em safe points definidos.
- [ ] Execução headless é possível sem Qt, janela, GPU, dispositivo de áudio ou relógio de parede.
- [ ] Replay com estado inicial e sequência de eventos normalizados reproduz os mesmos resultados digitais.
- [ ] Testes demonstram determinismo, ordenação de eventos, invariância por tamanho de lote e comportamento em limites.
- [ ] Métricas distinguem tempo emulado, tempo de execução, apresentação física e latência medida.

## 8. Evidência e estados

A existência de uma API ou crate temporal não comprova integração. Registrar separadamente `IMPLEMENTED`, `QUICK_PASS`, `SHORT_PASS`, `LONG_PASS`, `INTEGRATION_PENDING` e `TEMPORAL_PASS`.

`TEMPORAL_PASS` exige evidência de ordenação e comportamento temporal nos componentes relevantes, com traces/testes adequados ao nível de precisão declarado. T-states nominais por instrução, contagem de frames ou sincronização em fronteiras de frame não comprovam precisão ciclo a ciclo.

Qualquer exceção a esta diretiva exige decisão arquitetural explícita, justificada por evidência técnica e registrada no repositório. Conveniência da interface ou facilidade de implementação não são justificativas suficientes.

## 9. Relação com outros documentos

- [Arquitetura multi-sistema](../architecture.md)
- [Master Scheduler Rust](native-master-scheduler.md)
- [Política de desenvolvimento e validação](../development-and-validation-policy.md)
