# Arquitetura de emulação multi-sistema

## Objetivo

Construir uma base de emulação determinística, testável e evolutiva, começando pelo SG-1000. Componentes de hardware devem ser reutilizados entre consoles quando o hardware e os contratos forem realmente compatíveis. Reutilização não deve apagar diferenças entre revisões, mapas de memória, periféricos ou temporização.

A implementação de referência começa em Python e permanece como oráculo diferencial e fallback. Rust será adotado seletivamente para componentes críticos após testes de equivalência; LLVM ORC será apenas um backend JIT opcional de CPU, nunca a linguagem dos dispositivos. Consulte o [roteiro do núcleo nativo](architecture/native-core-roadmap.md) e o [ADR 0005](adr/0005-python-rust-llvm-architecture.md).

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
