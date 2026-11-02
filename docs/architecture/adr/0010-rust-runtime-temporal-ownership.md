# ADR-0010 — Runtime Rust autônomo e propriedade temporal da máquina

- **Status:** Accepted — contrato arquitetural; implementação incremental.
- **Escopo inicial:** SG-1000 e Master System, com prioridade imediata no SMS.
- **Decisões relacionadas:** arquitetura Rust-first; scheduler temporal; apresentação nativa Rust; perfis de hardware SMS1/SMS2.

## Contexto

O estado atual do SMS ainda é instruction-stepped: a fronteira PyO3 expõe operações que avançam instruções/frames, enquanto SmsMachine::step() executa uma instrução Z80 e só depois avança VDP/PSG. O crate temporal fornece MachineTime, domínios racionais de clock e fila determinística de eventos, mas ainda não constitui um scheduler de máquina integrado.

Essa estrutura é transitória e não satisfaz o contrato final de autonomia Rust nem de autoridade temporal única.

## Decisão 1 — Rust é o runtime completo

O processo Rust deverá iniciar, configurar, executar, apresentar, pausar, retomar e encerrar uma sessão de emulação sem Python instalado ou em execução.

O runtime Rust é proprietário de:

- thread/event loop de emulação e scheduler mestre;
- CPUs, barramentos, memórias, mappers, VDPs, PSG e demais dispositivos;
- clocks, contadores, timestamps, wait states, interrupções e ordenação de eventos;
- leitura e timestamp de entradas físicas/lógicas;
- produção e lifecycle dos buffers de áudio/vídeo;
- pacing de execução e integração com os backends nativos de host;
- save states, snapshots e telemetria de hardware.

A CLI e a integração PyO3 devem ser fachadas sobre a mesma API de aplicação/runtime. Não pode haver um caminho de execução alternativo com semântica diferente.

## Decisão 2 — Python é plano de controle opcional

Python/Qt não executa nem agenda hardware. Pode consultar snapshots imutáveis e telemetria read-only e enviar comandos de alto nível validados.

### Permitido

- consultar perfil ativo, capacidades, configuração efetiva e versão do runtime;
- ler snapshots de registradores, raster, clocks acumulados, contadores, IRQ e métricas;
- solicitar load, reset, pause, resume, stop e alterações de configuração;
- enviar eventos de input lógico (botões, teclado, controles), sem timestamp de hardware escolhido pelo Python;
- selecionar modelo de console, revisão de VDP, região, periféricos e opções suportadas;
- apresentar na GUI imagens/snapshots e métricas já produzidos pelo runtime Rust.

### Proibido

- chamar step(), step_instruction(), run_frame() ou qualquer API que avance ciclos ou instruções;
- ler ou escrever diretamente o estado mutável interno dos dispositivos;
- controlar clocks, scheduler, prioridades, timestamps de hardware, H/VBlank ou interrupções;
- gerar, editar, misturar, drenar ou agendar PCM/framebuffer como parte do caminho de emulação;
- escolher quando um dispositivo deve avançar ou quando um frame lógico está concluído;
- executar callbacks Python por instrução, ciclo, scanline ou pixel.

Os comandos recebidos são mensagens para a thread Rust. O runtime valida, serializa e aplica mudanças em fronteiras seguras definidas por ele. O timestamp de input é atribuído pelo próprio runtime na sua timeline; o cliente não pode retroagir eventos para alterar causalidade já processada.

A GUI pode exibir conteúdo imutável que Rust publicou, mas não passa a ser proprietária do output ou do seu relógio.

## Decisão 3 — Uma API de sessão, múltiplos hosts

Contrato conceitual da camada de aplicação (nomes ilustrativos; não são APIs já implementadas):

- Runtime::new(config) — cria sessão e dispositivos;
- Runtime::load(content, system_profile) — solicita carregamento validado;
- Runtime::submit_command(command) — envia comando de alto nível;
- Runtime::submit_input(input) — entrega input para timestamp pelo runtime;
- Runtime::snapshot() — obtém snapshot imutável e versionado;
- Runtime::telemetry() — obtém métricas read-only;
- Runtime::request_shutdown() — solicita encerramento sem avançar a máquina pelo chamador.

A thread/event loop interna do runtime executa a máquina. A API pública não expõe step, tick, advance_cycles ou run_frame para consumidores externos.

A CLI sega-emu e a fachada PyO3 devem instanciar a mesma sessão e os mesmos serviços. A CLI headless utiliza os mesmos componentes digitais e portas de host nulas, sem janela, GPU ou dispositivo de áudio físico. A execução com janela utiliza backends nativos Rust. A apresentação física pode operar assincronamente, sem bloquear nem retroagir a timeline lógica.

## Decisão 4 — Autoridade temporal e determinismo

O scheduler mestre Rust é a única autoridade temporal. Cada máquina declara uma unidade de tempo mestre documentada e relações racionais entre essa unidade e os domínios de clock dos dispositivos.

- Não usar f32/f64 para acumular fase ou converter clocks.
- Preservar restos fracionários ao converter domínios de clock.
- Rejeitar avanço temporal regressivo.
- Ordenar eventos pelo timestamp, prioridade de hardware documentada e sequência estável.
- Definir explicitamente a ordem de eventos simultâneos; não atribuir prioridades por conveniência.
- Acesso de CPU ao barramento deve ocorrer no instante correto do ciclo, e não ser aplicado retrospectivamente após uma instrução agregada.
- Wait states e contenção, quando sustentados por evidência, consomem tempo na mesma timeline.
- O tempo de parede, VSync, latência de áudio e duração de renderização não alteram a quantidade de tempo emulado.

A existência de MachineTime ou EventQueue isoladamente não constitui scheduler completo nem prova cycle accuracy. A precisão só pode ser declarada após integração de CPU, bus e dispositivos, traces reproduzíveis e oráculos independentes.

## Decisão 5 — Perfil de hardware SMS

O perfil separa dimensões que não são equivalentes:

- SmsConsoleModel: SMS1 ou SMS2;
- SmsVdpRevision: Sega 315-5124 ou Sega 315-5246;
- Territory: Japan, Export ou Unknown;
- VideoStandard: NTSC, PAL-M, PAL ou Unknown;
- presença de FM e opções de compatibilidade.

Padrões de configuração: SMS2 + Sega 315-5246. O usuário pode selecionar SMS1 e escolher a revisão de VDP independentemente. O modelo de console não pode sobrescrever silenciosamente a revisão ASIC selecionada.

Os tipos de perfil são a primeira etapa de modelagem. Ainda não significam que todas as diferenças entre ASICs estejam implementadas no renderer/VDP. Cada diferença (máscara de endereço, modos 224/240, paleta, limites/zoom de sprite, latches, counters e timing) deverá ser ligada a um comportamento específico com teste e fonte independente. Não usar condicionais espalhadas: concentrar diferenças em capacidades/regras explícitas do perfil VDP.

## Decisão 6 — Outputs e apresentação

O VDP e o áudio produzem dados digitais dentro do runtime Rust. Os adaptadores Rust de host consomem esses dados e controlam dispositivos físicos.

- Python não gera nem controla PCM ou framebuffer.
- O host de vídeo consome snapshots imutáveis; resize, VSync, GPU busy e device loss não alteram a timeline.
- O host de áudio consome PCM produzido pelo runtime; backpressure não pode fazer o PSG avançar ou recuar.
- O host nulo permite testes determinísticos sem hardware físico.
- A mesma ROM, configuração, estado inicial e sequência de inputs deve produzir os mesmos eventos e resultados digitais em CLI headless e execução com GUI, independentemente da API gráfica.

## Decisão 7 — CLI independente

O binário standalone será uma interface de usuário do runtime, não um segundo emulador. Comandos esperados, sujeitos à API real e ao inventário dos crates:

    sega-emu run jogo.sms --model sms2
    sega-emu run jogo.sms --model sms1 --vdp 315-5124
    sega-emu run jogo.sms --model sms1 --vdp 315-5246
    sega-emu run jogo.sms --headless
    sega-emu inspect jogo.sms
    sega-emu test --system sms --models all

O formato exato dos argumentos será definido após inventário de CLI/configuração existente. Não criar um binário paralelo antes de verificar os crates e ferramentas atuais.

## Modelo de componentes e dependências

    CLI Rust ───────────────┐
                           ├── Runtime / Session API ── Emulation Thread
    PyO3 control facade ───┘             │
                                         ├── Machine Scheduler
                                         │    ├── CPU + Bus + Memory + Mapper
                                         │    ├── VDP + IRQ
                                         │    ├── PSG / FM
                                         │    └── Input event queue
                                         │
                                         ├── Immutable snapshots ── Rust video host
                                         ├── PCM stream ─────────── Rust audio host
                                         └── read-only telemetry

    Qt/Python: commands + snapshots + telemetry only

## Plano de modelagem e implementação

1. **M0 — Perfil explícito:** tipos SMS1/SMS2 e ASIC 315-5124/315-5246; dimensões independentes e defaults testados.
2. **M1 — Contrato de sessão:** definir comandos, estados de sessão, snapshots e telemetria sem API pública de avanço.
3. **M2 — Scheduler de máquina:** compor primitives temporais existentes; definir perfil de clock SMS, prioridades e causalidade com base em documentação.
4. **M3 — Integração de dispositivos:** migrar execução Z80, bus, VDP e PSG para eventos/ciclos no scheduler; remover avanço agregado por instrução como autoridade.
5. **M4 — Hosts Rust:** áudio, vídeo, input e storage nativos; headless determinístico.
6. **M5 — CLI standalone:** executar e testar sem Python.
7. **M6 — Fachada PyO3:** comandos/snapshots/telemetria apenas; remover step/run_frame da API pública.
8. **M7 — Qt launcher:** seletor de perfil e painel de inspeção, sem controle temporal ou ownership de outputs.
9. **M8 — Aceitação:** replay cruzado CLI/headless/GUI, testes de regressão SG-1000 e SMS, SHORT/LONG/TEMPORAL gates conforme evidência.

## Critérios de aceitação

- O processo standalone inicia e executa SMS sem Python.
- Não há API externa que avance instrução/ciclo/frame.
- Somente a thread/runtime Rust altera estado mutável de hardware.
- Python pode consultar snapshots e telemetria sem referências mutáveis.
- Configuração/input são comandos validados e aplicados pelo runtime.
- Áudio/vídeo são produzidos e entregues por componentes Rust.
- Mesma sequência de inputs/configuração produz o mesmo resultado digital em CLI e GUI.
- Scheduler é integrado a CPU, bus e dispositivos; nenhum TEMPORAL_PASS antes de trace e oráculo independentes.
- SG-1000 continua coberto pelos gates de regressão.
- Todos os gates Rust/Python passam sem warnings no mesmo SHA candidato.

## Estado inicial

- Contrato arquitetural: **DOCUMENTED**.
- Tipos SMS1/SMS2 e VDP ASIC: **MODELED — initial type layer only**.
- VDP ASIC behavior differences: **NOT IMPLEMENTED**.
- Machine scheduler: **NOT IMPLEMENTED**.
- Runtime standalone/CLI: **NOT IMPLEMENTED**.
- PyO3 control-only facade: **NOT IMPLEMENTED**.
- Host áudio/vídeo totalmente Rust-owned: **PARTIAL / INVENTORY REQUIRED**.
- TEMPORAL_PASS: **NOT CLAIMED**.
