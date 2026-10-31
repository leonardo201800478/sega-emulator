# Arquitetura de emulação multi-sistema

## Objetivo

Construir uma base de emulação determinística, testável e evolutiva, consolidando conjuntamente SG-1000 e Master System. Componentes de hardware devem ser reutilizados entre consoles quando o hardware e os contratos forem realmente compatíveis. Reutilização não deve apagar diferenças entre revisões, mapas de memória, periféricos ou temporização.

## Política de implementação (decisão adotada)

Rust é a linguagem principal e definitiva para a lógica de hardware do emulador. Novos componentes críticos de CPU, barramento, memória, temporização, VDP, PSG, interrupções e scheduler devem ser implementados diretamente em Rust quando seus contratos e critérios de validação estiverem claros. Não é obrigatório criar primeiro uma versão Python nem portar cada componente duas vezes.

Python ativo na aplicação é restrito ao frontend Qt, orquestração de aplicação, carregamento de ROMs/perfis, configuração/logging e à fachada fina PyO3. A arquitetura-alvo reserva a Rust os backends que executam hardware, processam áudio/input ou controlam o pipeline de vídeo; o estado da migração deve ser verificado por componente, pois propostas abertas não contam como integração. Qt pode consultar e apresentar capacidades do host, mas não substitui esses runtimes. Ferramentas e testes podem ser escritos em Python, mas devem exercitar o runtime Rust e não importar implementações de hardware arquivadas. As antigas implementações Python de CPU, barramento/memória, VDP, PSG e máquina ficam em `legacy/python_reference/` apenas para consulta, fora do pacote executável.

Novos componentes de hardware devem nascer em Rust quando os contratos e critérios de validação estiverem claros. Não criar novos protótipos de hardware Python nem reativar código de `legacy/`; validar Rust com testes sintéticos, vetores independentes, regressões e evidência técnica.

A migração é incremental: preservar contratos públicos e testes úteis, mas o único runtime de hardware ativo é Rust. O código arquivado não é backend de referência ativo nem parte da estratégia de validação. Não adicionar JIT nem paralelismo de hardware por antecipação; usar profiling e benchmarks reproduzíveis. A ordem temporal de CPU, VDP, PSG e barramento deve continuar determinística, sem paralelismo inseguro entre componentes sincronizados.

Consulte [ADR 0007 — Política de desenvolvimento Rust-first](adr/0007-rust-first-development-policy.md) e [AGENTS.md](../AGENTS.md) para o fluxo de implementação, testes e workflows.

## Princípios

1. **Fidelidade antes de atalhos:** implementar as instruções documentadas de cada CPU, com flags, efeitos colaterais, interrupções e temporização sustentados por documentação e testes.
2. **Separar CPU de plataforma:** um núcleo de CPU depende de contratos de barramento e sinais, não diretamente de um console concreto.
3. **Separar estado lógico de temporização:** uma instrução que retorna um número nominal de ciclos não equivale a executar cada ciclo de barramento.
4. **Dispositivos com identidade própria:** RAM, ROM, VDP, PSG, controladores, chips de som e demais componentes têm estados e regras de acesso explícitos.
5. **Sistemas compostos por componentes:** cada console conecta CPUs, barramentos, memória e periféricos por meio de uma configuração de plataforma.
6. **Determinismo e teste:** mesma configuração, estado inicial e sequência de eventos devem produzir o mesmo resultado.
7. **Sem abstração prematura:** interfaces devem surgir de necessidades concretas e testes; não criar uma camada genérica para todo hardware futuro sem evidência.
8. **Precisão declarada por evidência:** documentar separadamente o que está implementado, o que foi validado e o que ainda é aproximação.

## Camadas previstas

### 1. Núcleos de processadores

Cada arquitetura de CPU terá seu próprio núcleo, testes e documentação. O núcleo Z80 será reutilizado nos sistemas que realmente contêm um Z80 compatível, respeitando configurações e sinais específicos da plataforma. Não compartilhar o código de CPU entre arquiteturas distintas.

A meta é cobrir o conjunto de instruções completo e documentado de cada processador usado pelos consoles suportados, incluindo prefixos, modos de endereçamento, flags, instruções privilegiadas ou especiais quando aplicáveis, interrupções e casos de borda. Comportamentos não documentados serão identificados como tais e implementados somente com evidências experimentais ou referências confiáveis.

### 2. Contratos de acesso

O contrato atual `MemoryBus` expõe `read(address)` e `write(address, value)` para acessos lógicos a bytes. É adequado para os testes e a fase inicial, mas não deve ser confundido com um modelo de barramento temporal.

Quando necessário, evoluir separadamente para contratos explícitos de:
- acesso a memória e portas de I/O;
- ciclos de leitura/escrita, estados de espera e contenção;
- interrupções, reset, HALT e linhas de controle;
- avanço temporal e sincronização de dispositivos.

Esses contratos devem ser introduzidos em etapas pequenas, com testes de compatibilidade. Não adicionar todos de uma vez antes de o hardware exigir isso.

### 3. Componentes de hardware

Memórias e periféricos devem ser módulos independentes sempre que isso representar hardware real e facilitar testes. Um componente pode ser reutilizado se suas características elétricas/lógicas e temporais forem compatíveis; caso contrário, compartilhar apenas código comprovadamente comum e manter configurações ou implementações específicas.

### 4. Plataformas

Uma plataforma instancia e conecta os componentes concretos, define mapas de memória e I/O, clocks, reset, interrupções e regras de arbitragem. A CPU não deve conhecer o modelo específico de console.

## Alvos e responsabilidades por sistema

| Sistema | CPUs principais/auxiliares relevantes | Consequência arquitetural |
|---|---|---|
| SG-1000 | Z80 | Runtime Rust integrado; preservar CPU, RAM, barramento, I/O, TMS9918A, PSG e entrada como baseline funcional. |
| Master System (SMS) | Z80 | Runtime Rust integrado; consolidar gameplay, CPU, mapa, VDP Mode 4, PSG, I/O, mappers e diferenças próprias. |
| Game Gear | Z80 | Marco futuro após SG-1000/SMS; reutilizar somente mecanismos compatíveis e especializar LCD, I/O, estéreo e periféricos. |
| Mega Drive / Genesis | Motorola 68000 principal e Z80 auxiliar | Reutilizar o núcleo Z80 quando compatível; criar núcleo 68000 separado e modelar a comunicação, RAM e arbitragem entre processadores. |
| Sega CD | Base Mega Drive, 68000 adicional e subsistemas específicos | Compor com o Mega Drive; adicionar CPU, memória, CD e demais componentes específicos em marcos próprios. |
| 32X | Base Mega Drive e dois SH-2 | Compor com a plataforma-base e implementar os SH-2 e os subsistemas 32X separadamente. |

Esta tabela é um direcionamento de arquitetura, não uma afirmação de que esses sistemas já estejam implementados ou que todos os detalhes de compatibilidade estejam especificados.

## Plano incremental

A ordem atual de execução está no [Roadmap de consolidação SG-1000 + SMS](roadmaps/sg1000-sms-consolidation-roadmap.md). Ele prioriza baseline reproduzível, motor Z80 compartilhado, semântica Z80, barramentos/I/O, VDPs independentes, áudio, integração funcional, compatibilidade, scheduler e Qt/PyO3. Game Gear e plataformas posteriores aguardam o fechamento desse marco.

## Critérios mínimos de conclusão de uma CPU

- Cobertura documentada de todos os opcodes oficiais e prefixos aplicáveis ao modelo escolhido.
- Testes de resultados, flags documentadas e não documentadas quando conhecidas, PC/SP, wraparound e efeitos colaterais.
- Ciclos corretos para caminhos tomados/não tomados e modos de endereçamento.
- Interrupções, reset, HALT e I/O validados conforme a arquitetura.
- Testes de diagnóstico de referência e regressão automatizada.
- Incertezas de hardware identificadas; nenhuma alegação de precisão temporal além do que os testes comprovam.

## Estado atual — fotografia de arquitetura

O estado a seguir resume a arquitetura integrada conhecida no baseline documental de 2026-10-31. Ele não substitui a auditoria de código e workflows no SHA de cada mudança.

- **SG-1000:** runtime Rust integrado à aplicação por PyO3. A máquina inclui Z80, memória, barramento, I/O, VDP TMS9918A, PSG SN76489AN, entrada e execução de frames. A compatibilidade funcional observada em jogos específicos está registrada em documentos de hardware e não implica suporte universal.
- **Master System:** runtime Rust integrado e ativado no frontend Qt. A rota de carregamento/execução SMS foi integrada na PR #256. A auditoria SMS de 2026-10-29 registra o estado técnico e limitações; deve ser atualizada contra o baseline corrente antes de orientar remoções/refatorações.
- **Z80:** SG-1000 e SMS mantêm executores duplicados no baseline histórico auditado, com 3.257 linhas executáveis normalizadas idênticas reportadas pela auditoria comparativa. O SMS possui suíte de testes mais madura. A consolidação em um único crate compartilhado é a prioridade técnica inicial; confirme novamente os dados no SHA candidato antes de remover cópias.
- **VDP:** TMS9918A do SG-1000 e VDP Mode 4 do SMS são dispositivos distintos, com estados, registradores, raster, sprites e contratos próprios.
- **Áudio:** geração digital e runtime nativo pertencem a Rust. WASAPI Shared/event-driven e XAudio2 são caminhos de saída documentados; a migração e a fidelidade por chip continuam sujeitas a validação específica.
- **Qt/PyO3:** o frontend existente suporta os dois sistemas integrados. Os documentos de modelo funcional e arquitetura-alvo Qt são especificações de evolução e não afirmam que todas as decisões já estejam implementadas.
- **Tempo:** o modelo atual de execução por instrução/T-states nominais não equivale a precisão ciclo a ciclo. A autoridade temporal exclusiva Rust e o Master Scheduler são decisões arquiteturais; o nível de implementação e validação deve ser conferido na auditoria temporal atual.

A fotografia e os limites por sistema estão documentados em [auditoria SMS de 2026-10-29](audits/sms-current-technical-status-2026-10-29.md), [auditoria comparativa Z80](architecture/migrations/sms-z80-maturity-audit-2026-10-31.md), [marco SG-1000](milestones/sg1000-audio-video-milestone-2026-10-26.md) e [roadmap SG-1000 + SMS](roadmaps/sg1000-sms-consolidation-roadmap.md). Auditorias datadas são evidências históricas, não garantias de que o código atual permaneça idêntico.



## Fidelidade de áudio e reutilização do PSG

O modelo SN76489AN do SG-1000 tem um roadmap dedicado que separa lógica digital, reamostragem anti-aliasing e resposta analógica, além de registrar as diferenças necessárias para futuras variantes Master System, Game Gear e Mega Drive: [Roadmap de fidelidade do PSG e reutilização de hardware Sega](architecture/psg-fidelity-roadmap.md).


## Catálogo de ROM, metadados e OSD SMS

O modelo de identificação regional, catálogo técnico de hardware, metadados de save/FM/periféricos e OSD do Master System está especificado em [SMS ROM metadata, compatibilidade de hardware e OSD](architecture/sms-rom-metadata-and-osd.md). O documento está em estado DESIGN / NOT IMPLEMENTED e separa identidade da ROM, perfil de console, padrão de vídeo e estado de compatibilidade.

## Fluxo permanente de implementação e validação

O procedimento obrigatório para cada componente está definido em [Política permanente de desenvolvimento, fidelidade e entrega](development-and-validation-policy.md). Resumo:

1. Inventariar código, contratos, testes, documentação e SHA atual; não duplicar trabalho concluído.
2. Implementar as lacunas reais em Rust, em fatias pequenas e compiláveis, com fidelidade máxima sustentada por evidências.
3. Executar testes rápidos por fatia, validação funcional abrangente curta e depois testes longos, corrigindo divergências entre as fases.
4. Só então avançar ao próximo componente.
5. Preparar desde o início contratos de tempo e eventos de barramento, mas reservar a campanha completa de temporização ciclo a ciclo para a integração da máquina SMS completa e funcional.

A implementação, a validação funcional e a validação temporal são estados separados. Uma CPU que executa todos os opcodes ou retorna T-states nominais não é, por isso, semanticamente exaustiva nem ciclo a ciclo. O projeto usa no máximo três agentes de IA simultâneos; CI continua sendo validação automatizada independente. Todo runtime de hardware ativo é Rust; Python serve ao frontend Qt/PySide6 e à infraestrutura de testes/ferramentas.

## SMS cartridge mappers and game-specific hardware

The current SMS cartridge implementation and the staged plan for ROM-only cartridges, Sega SRAM, Codemasters, Korean/MSX/Nemesis/Janggun mappers, multicarts, FM audio and peripherals are documented in:

- [SMS cartridge mapper and game-compatibility roadmap](architecture/sms-cartridge-mapper-roadmap.md)
- [Special SMS mappers and known game families — research inventory](hardware/sms-special-mappers-research.md)

The research inventory is not an implementation specification by itself. Use the roadmap's source hierarchy, explicit uncertainty labels, deterministic synthetic tests and license boundary before changing the Rust runtime.


## Runtime de áudio nativo multi-sistema

O runtime de áudio é uma infraestrutura Rust compartilhada, independente das APIs do host e dos consoles. WASAPI Shared/event-driven é o backend padrão Windows e tenta IAudioClient3 para negociar o período suportado, com fallback para Shared convencional; XAudio2 é a alternativa. Python/PySide6 é apenas frontend de configuração, controle e diagnóstico: PCM não atravessa PyO3 durante gameplay. O escopo de hardware contempla PSG, FM, PCM/DAC, CDDA e PWM ao longo de SG-1000, SMS/GG, Mega Drive, Sega CD e 32X; QSound permanece extensão DSP opcional. A arquitetura, os critérios e as fases de migração estão em [Runtime de áudio nativo](architecture/native-audio-runtime.md).


## Scheduler mestre Rust e sincronização A/V/input

A arquitetura-alvo para sincronizar CPU, barramento, VDP, PSG, entrada e saídas de áudio/vídeo em uma única timeline Rust está especificada em [Master Scheduler Rust](architecture/native-master-scheduler.md), com etapas e gates em [Plano de migração do Master Scheduler](architecture/migrations/native-master-scheduler.md). Essa etapa depende da integração dos ports nativos de vídeo e input; a documentação não declara implementação nem validação temporal concluídas.


## Reorganização do runtime e núcleo Z80 compartilhado

A decisão arquitetural vigente para consolidar o runtime Rust, unificar o motor Z80 de SG-1000/SMS e preparar Game Gear/Genesis, e coordenar a transição temporal está registrada em [Reorganização arquitetural — runtime Rust, Z80 compartilhado e sincronização temporal](architecture/migrations/shared-z80-runtime-reorganization.md). O documento separa o estado observado em `main` do desenho-alvo e das propostas ainda abertas; não declara a migração concluída.


## ADRs da reorganização Rust-first

As decisões para o motor Z80 compartilhado, scheduler temporal e fronteiras do runtime host estão indexadas em [ADRs de reorganização](architecture/adr/README.md). A evidência de baseline e reconciliação das PRs está em [F0 — inventário remoto](architecture/migrations/f0-remote-inventory-2026-10-31.md).


### Auditoria de maturidade Z80 SMS × SG-1000

A comparação confirmou que o SMS é o baseline semântico e de validação mais maduro, embora o código executável atual dos dois cores seja idêntico. Consulte a [auditoria comparativa Z80](architecture/migrations/sms-z80-maturity-audit-2026-10-31.md) e a [ADR-0001 atualizada](architecture/adr/0001-shared-z80-engine.md).


## Diretrizes normativas consolidadas

- [Escopo do projeto](project-scope.md) define plataformas, prioridades, limites e critérios globais.
- [Roadmap SG-1000 + SMS](roadmaps/sg1000-sms-consolidation-roadmap.md) define a ordem de execução atual e os critérios de saída do marco prioritário.
- [Diretriz de autoridade temporal Rust](architecture/rust-temporal-authority.md) determina que Rust é o único proprietário do avanço temporal emulado.
- [Diretriz Rust-only de áudio](architecture/audio-rust-only-directive.md) determina que chips, geração, processamento e pipeline digital de áudio pertencem ao runtime Rust.
- [Escopo de periféricos e entrada](architecture/input-peripherals-scope.md) registra decisões de periféricos e limitações dos backends host.
- [Modelo funcional Qt](architecture/qt-frontend-functional-model.md) e [arquitetura-alvo Qt](architecture/qt-frontend-target-architecture.md) descrevem produto e fronteiras da aplicação; são especificações, não prova de implementação.
- [Coprocessadores especializados](architecture/specialized-coprocessors.md) é uma diretriz futura, condicionada ao marco Mega Drive.

Em caso de divergência, a precedência é: contratos e comportamento efetivamente implementados e validados no código; decisões arquiteturais normativas deste repositório; documentos especializados; inventários/auditorias datados (fotografias históricas); propostas de implementação. Nenhuma proposta ou PR aberta prevalece sobre código integrado sem revisão e merge.

