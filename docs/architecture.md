# Arquitetura de emulação multi-sistema

## Objetivo

Construir uma base de emulação determinística, testável e evolutiva, começando pelo SG-1000. Componentes de hardware devem ser reutilizados entre consoles quando o hardware e os contratos forem realmente compatíveis. Reutilização não deve apagar diferenças entre revisões, mapas de memória, periféricos ou temporização.

## Política de implementação (decisão adotada)

Rust é a linguagem principal e definitiva para a lógica de hardware do emulador. Novos componentes críticos de CPU, barramento, memória, temporização, VDP, PSG, interrupções e scheduler devem ser implementados diretamente em Rust quando seus contratos e critérios de validação estiverem claros. Não é obrigatório criar primeiro uma versão Python nem portar cada componente duas vezes.

Python ativo na aplicação é restrito ao frontend Qt e seus backends de host, carregamento de ROMs/perfis, configuração/logging e à fachada fina PyO3. Ferramentas e testes podem ser escritos em Python, mas devem exercitar o runtime Rust e não importar implementações de hardware arquivadas. As antigas implementações Python de CPU, barramento/memória, VDP, PSG e máquina ficam em `legacy/python_reference/` apenas para consulta, fora do pacote executável.

Novos componentes de hardware devem nascer em Rust quando os contratos e critérios de validação estiverem claros. Não criar novos protótipos de hardware Python nem reativar código de `legacy/`; validar Rust com testes sintéticos, vetores independentes, regressões e evidência técnica.

A migração é incremental: preservar contratos públicos e testes úteis, mas o único runtime de hardware ativo é Rust. O código arquivado não é backend de referência ativo nem parte da estratégia de validação. Não adicionar JIT nem paralelismo de hardware por antecipação; usar profiling e benchmarks reproduzíveis. A ordem temporal de CPU, VDP, PSG e barramento deve continuar determinística, sem paralelismo inseguro entre componentes sincronizados.

Consulte [ADR 0007 — Política de desenvolvimento Rust-first](adr/0007-rust-first-development-policy.md) e [AGENTS.md](../AGENTS.md) para o fluxo de implementação, testes e workflows.

## Princípios

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
| SG-1000 | Z80 | Primeiro alvo para validar CPU, RAM, barramento, VDP, PSG e entrada. |
| Master System / Game Gear | Z80 | Reutilizar o núcleo Z80; implementar mapas, VDP, áudio, I/O e diferenças da plataforma separadamente. |
| Mega Drive / Genesis | Motorola 68000 principal e Z80 auxiliar | Reutilizar o núcleo Z80 quando compatível; criar núcleo 68000 separado e modelar a comunicação, RAM e arbitragem entre processadores. |
| Sega CD | Base Mega Drive, 68000 adicional e subsistemas específicos | Compor com o Mega Drive; adicionar CPU, memória, CD e demais componentes específicos em marcos próprios. |
| 32X | Base Mega Drive e dois SH-2 | Compor com a plataforma-base e implementar os SH-2 e os subsistemas 32X separadamente. |

Esta tabela é um direcionamento de arquitetura, não uma afirmação de que esses sistemas já estejam implementados ou que todos os detalhes de compatibilidade estejam especificados.

## Plano incremental

1. **SG-1000 — base e Z80:** completar e validar a CPU Z80 progressivamente, manter o contrato atual funcionando e só ampliá-lo quando necessário.
2. **Z80 — cobertura:** construir uma matriz de opcodes por família/prefixo, testes de flags e ciclos, testes de limites e comparação com testes de diagnóstico confiáveis.
3. **Temporização:** definir o modelo de avanço temporal e acessos ao barramento antes de afirmar precisão ciclo a ciclo.
4. **SG-1000 — integração:** adicionar I/O, VDP, PSG, entrada e sincronização, testando cada componente isoladamente e em conjunto.
5. **Master System / Game Gear:** reutilizar os componentes compatíveis e documentar diferenças específicas.
6. **Mega Drive:** desenvolver e validar o núcleo 68000 e a interação entre CPUs antes de ampliar para Sega CD ou 32X.
7. **Sega CD e 32X:** adicionar cada conjunto de hardware como expansão incremental da plataforma-base.

## Critérios mínimos de conclusão de uma CPU

- Cobertura documentada de todos os opcodes oficiais e prefixos aplicáveis ao modelo escolhido.
- Testes de resultados, flags documentadas e não documentadas quando conhecidas, PC/SP, wraparound e efeitos colaterais.
- Ciclos corretos para caminhos tomados/não tomados e modos de endereçamento.
- Interrupções, reset, HALT e I/O validados conforme a arquitetura.
- Testes de diagnóstico de referência e regressão automatizada.
- Incertezas de hardware identificadas; nenhuma alegação de precisão temporal além do que os testes comprovam.

## Estado atual

O repositório contém caminhos de execução para as famílias base, CB, ED, DD/FD e DD/FD-CB da CPU Z80 e um barramento inicial de memória SG-1000 com ROM e RAM. O protocolo de memória modela leituras e escritas lógicas de bytes; não modela cada ciclo de barramento. Existe um contrato inicial separado para I/O, mas o mapa de portas da plataforma ainda não está integrado. A validação local reportada em 22/10/2026 aprovou 262 testes, além das verificações Ruff; isso não prova compatibilidade geral nem precisão ciclo a ciclo. A máquina SG-1000 completa permanece não integrada.

- [Registro de desenvolvimento e ponto de retomada](development-handoff-2026-10-22.md): HEAD confirmado, testes locais, limitações e próximos passos.
- [Auditoria geral do código](codebase-audit.md): riscos e estado revisado.
- [Auditoria de cobertura da CPU Z80](z80-coverage.md): inventário de instruções e prefixos.

Próxima tarefa técnica: validação diferencial da CPU Z80 contra uma suíte de referência confiável, com foco inicial em flags, operações ED de bloco, IX/IY, interrupções e temporização nominal. Corrigir primeiro as divergências reproduzíveis e documentar o que não puder ser validado.


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


## Escopo consolidado do projeto

O escopo transversal vigente de plataformas, identificação de mídia, perfis regionais NTSC/PAL/PAL-M, firmware/BIOS, VDPs, raster, sprites, composição, scheduler mestre, áudio, input e critérios de validação está consolidado em [Escopo do projeto — Sega Emulator](project-scope.md). Este documento é o índice normativo de escopo; as páginas de arquitetura e hardware detalham cada componente. Uma decisão em PR relacionada não é considerada integrada até seu merge.
