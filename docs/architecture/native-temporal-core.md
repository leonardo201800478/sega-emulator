# Rust Temporal Core — relógio mestre, domínios de clock e eventos

**Nome adotado:** Rust Temporal Core.  
**Estado desta primeira fatia:** infraestrutura temporal genérica implementada em crate isolado; integração com máquinas e dispositivos pendente.  
**Precisão ciclo a ciclo:** não declarada.

## Decisão arquitetural

O emulador terá um único proprietário temporal em Rust por instância de máquina. Esse proprietário coordena todos os processadores, barramentos e dispositivos digitais da plataforma em uma timeline determinística.

A infraestrutura compartilhada chama-se Rust Temporal Core e será composta progressivamente por:

1. MachineTime: timestamp inteiro no domínio de tempo escolhido pelo perfil da máquina.
2. ClockRatio / ClockDomain: conversão racional entre o tempo mestre e os ciclos de cada fonte de clock, preservando fase.
3. EventPriority / EventQueue: ordenação estável por timestamp, prioridade específica da plataforma e sequência de inserção.
4. MachineScheduler (fase posterior): proprietário do avanço, despacho, safe points e integração com os dispositivos.
5. Adaptadores temporais: CPU/micro-operações, barramento, VDP, PSG/FM, DMA, interrupções e demais dispositivos.

O crate compartilhado não define frequências, divisores, prioridades ou comportamento elétrico de consoles. Esses fatos pertencem aos perfis e componentes de hardware e precisam de fontes e testes próprios.

## Uma infraestrutura, múltiplos domínios

Não haverá um scheduler independente para CPU, VDP ou PSG, nem um scheduler separado por chip. Cada instância de SG-1000, SMS/GG, Mega Drive, Sega CD ou 32X terá um scheduler de máquina. Esse scheduler poderá administrar diversos ClockDomains simultâneos.

Um domínio representa uma fonte de clock ou divisor logicamente relevante, não necessariamente cada componente físico. Componentes que compartilham a mesma fonte e fase podem compartilhar um domínio quando a documentação de hardware justificar isso. Relações de frequência e fase diferentes exigem domínios distintos.

MachineTime é uma unidade inteira definida pelo perfil da máquina; não é uma frequência universal em Hz. O perfil escolhe uma base que permita representar exatamente as relações temporais que precisa modelar. A escolha da base deve ser documentada e validada antes da integração de cada plataforma.

## Conversão racional e fase

ClockRatio representa ciclos locais por tick mestre como uma fração positiva. ClockDomain mantém:

- razão numerador/denominador;
- timestamp até o qual avançou;
- total de ciclos locais;
- resto/fase fracionária da conversão.

A conversão usa aritmética inteira verificada. Avançar um domínio em muitos blocos deve produzir o mesmo total de ciclos e fase que avançá-lo em um único bloco. Overflow e avanço retroativo são erros explícitos e não podem alterar parcialmente o estado.

Não arredondar cada intervalo isoladamente nem usar ponto flutuante como fonte de verdade temporal. Divisores e fases reais de hardware precisam ser obtidos de documentação ou evidência, nunca inferidos a partir de valores convenientes.

## Ordenação de eventos

A fila inicial ordena eventos pela chave:

    (MachineTime, EventPriority, insertion_sequence)

- Timestamp menor é despachado primeiro.
- Em timestamps iguais, a prioridade numérica menor é despachada primeiro.
- A sequência estável desempata eventos da mesma classe.
- A prioridade é fornecida pelo perfil da plataforma; o crate não presume que CPU sempre precede VDP, ou vice-versa.
- Eventos passados e regressões de prioridade no timestamp já despachado são rejeitados.

Um evento causal criado durante o despacho no mesmo timestamp só pode ter prioridade igual ou posterior à já despachada. Eventos simultâneos de prioridade anterior devem ter sido inseridos antes de o despacho alcançar aquele timestamp. O futuro MachineScheduler deverá tornar explícitos os limites de fase e a política de eventos delta para evitar causalidade retroativa.

## Contratos temporais dos dispositivos

A integração deverá evoluir sem exigir que cada componente execute código em todos os ticks:

- Dispositivos avançam até um timestamp de sincronização ou seu próximo evento interno.
- Componentes podem anunciar seu próximo evento temporal quando isso for determinável.
- O scheduler avança os dispositivos dependentes até o instante necessário antes de aplicar um acesso ou evento.
- Leituras/escritas de barramento têm instante, duração, tipo e efeitos colaterais explícitos.
- Esperas, contenção, DMA e arbitragem são modelados pela plataforma e podem suspender ou deslocar operações.
- CPU evolui de T-states agregados para operações internas/micro-operações e ciclos de barramento observáveis.
- VDP evolui de avanço agregado por instrução/scanline para eventos raster/IRQ/registradores nos instantes necessários.
- PSG/FM avançam seus divisores e estados digitais até o mesmo instante lógico; amostragem e saída analógica permanecem camadas distintas.

Não é requisito executar uma chamada Rust por T-state. O objetivo é preservar a posição temporal exata dos efeitos observáveis com batching seguro entre fronteiras de evento.

## CPU multicore e expansão multi-sistema

O modelo é um scheduler por máquina, não por CPU:

- SG-1000: Z80, TMS9918 e SN76489 conforme perfil documentado.
- SMS/GG: Z80, VDP e fontes de áudio específicas da variante.
- Mega Drive: 68000, Z80 auxiliar, VDP, PSG/YM2612 e arbitragem compartilhada.
- Sega CD: componentes da máquina-base mais CPU e subsistemas adicionais.
- 32X: componentes Mega Drive mais dois SH-2 e regras de acesso/arbitragem específicas.

CPUs podem ter domínios de clock distintos e disputar barramentos/recursos. O scheduler deve representar essas relações sem criar timelines independentes que avancem hardware fora de ordem. O crate compartilhado não declara clocks exatos nem que os sistemas futuros já estejam implementados.

## Fronteira com o host

Rust controla o tempo emulado. Python/PySide6 não chama o avanço por frame nem agenda ciclos. O host pode:

- enviar comandos coarse-grained (load/reset/pause/resume/stop);
- capturar input e publicar eventos normalizados por uma fronteira nativa;
- consumir áudio e apresentar vídeo em seus próprios relógios físicos;
- receber snapshots e telemetria.

Relógios de áudio e vídeo do host nunca avançam CPU, VDP ou PSG. Input físico recebe uma política explícita de conversão/amostragem para a timeline; o instante de chegada do evento ao worker não se torna automaticamente um timestamp de hardware.

## Fases e critérios de aceitação

### T0 — contrato e infraestrutura genérica

- [x] Criar crate Rust independente de plataforma.
- [x] Representar tempo mestre com inteiro de 128 bits.
- [x] Converter clock domains por razão racional e fase preservada.
- [x] Rejeitar avanço retroativo e overflow sem mutação parcial.
- [x] Ordenar eventos por timestamp, prioridade e sequência estável.
- [x] Testes unitários escritos para invariância por chunks, fase, monotonicidade, overflow e ordenação.
- [ ] Executar fmt, Clippy e testes no SHA candidato.
- [ ] Integrar o crate a qualquer runtime de máquina.

### T1 — MachineScheduler headless

- [ ] Criar scheduler com tempo atual, fila, despacho, safe points e política de delta-cycle.
- [ ] Definir perfil temporal SG-1000 a partir de fontes primárias.
- [ ] Provar ordenação, simultaneidade, eventos causais, pausa/reset e replay em testes sintéticos.
- [ ] Manter execução independente de wall-clock, Qt, GPU e áudio.

### T2 — integração SG-1000/SMS

- [ ] Integrar primeiro sem alterar resultados funcionais existentes.
- [ ] Remover conversões locais duplicadas de T-states/PSG somente após equivalência demonstrada.
- [ ] Sincronizar Z80, barramento, VDP, PSG e interrupções em pontos explícitos.
- [ ] Adicionar traces e oráculos independentes para acessos temporais.
- [ ] Preservar regressões SG-1000 e SMS.

### T3 — micro-operações e precisão temporal

- [ ] Evoluir Z80 sem descartar a implementação de instruções já validada.
- [ ] Cobrir fetch, refresh, leitura/escrita, I/O, ciclos internos, acknowledge, waits e interrupções.
- [ ] Alinhar eventos raster/VDP e divisores de áudio com evidência de hardware.
- [ ] Declarar TEMPORAL_PASS apenas após traces por T-state/evento e regressões independentes.

### T4 — expansão de plataformas

- [ ] Adicionar perfis e adaptadores específicos de SMS/GG, Mega Drive, Sega CD e 32X quando cada plataforma entrar no escopo.
- [ ] Modelar domínios múltiplos e arbitragem de barramento para máquinas com várias CPUs.
- [ ] Não inventar clocks nem promover plataformas futuras antes da validação do marco anterior.

## Dependências de vídeo e input

As PRs #300 (input nativo) e #301 (vídeo nativo) são necessárias para a integração completa das fronteiras de host, mas não bloqueiam o desenvolvimento nem os testes headless de T0/T1. O Temporal Core não depende de SDL, XInput, Qt, Vulkan, WASAPI ou PyO3.

A integração final deve reutilizar os ports existentes sem permitir que captura, áudio ou apresentação do host se tornem autoridades temporais da máquina.

## Estados de validação

A infraestrutura T0 pode ser declarada IMPLEMENTED somente após o código estar presente; QUICK_PASS e SHORT_PASS dependem de execução comprovada dos testes correspondentes. Nenhum estado LONG_PASS ou TEMPORAL_PASS é reivindicado por esta etapa. Clock conversion e fila de eventos isoladas não demonstram precisão de hardware.
