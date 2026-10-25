# Roteiro arquitetural: núcleo nativo com fidelidade verificável

- **Status:** direção arquitetural aprovada; execução incremental.
- **Escopo inicial:** SG-1000, CPU Z80, barramento, RAM/ROM, TMS9918A, PSG SN76489AN e entrada.
- **Princípio:** Python permanece como referência funcional e fallback; Rust interpretado é o primeiro backend nativo; LLVM ORC é um backend JIT opcional para o Z80, não uma substituição geral da máquina.

## Objetivo

Reduzir o custo de execução e preparar a arquitetura para sistemas Sega posteriores sem perder determinismo, semântica observável nem testes isolados. A migração será por componentes e contratos, não uma transcrição integral do projeto Python para Rust ou LLVM.

## Decisões de alto nível

1. **Python de referência:** preservar a implementação Python legível para frontend, ferramentas, diagnósticos, testes-oráculo e fallback. O runtime padrão não muda até que a equivalência esteja demonstrada.
2. **Rust para caminho crítico e hardware:** migrar seletivamente CPU, contratos de barramento/memória, scheduler e componentes para Rust, em fatias pequenas.
3. **LLVM apenas como compilador JIT:** LLVM ORC traduz blocos Z80 para código nativo. Não é a linguagem de implementação dos barramentos, RAM, VDP, PSG, input ou frontend.
4. **Três backends independentes:** Python interpretado (referência), Rust interpretado (nativo) e LLVM ORC JIT (experimental). A seleção é explícita; nenhum fallback silencioso nem troca automática de precisão.
5. **Precisão como contrato:** otimizações não podem remover ou reordenar leituras e escritas observáveis, I/O, interrupções, HALT, estados de espera ou sincronização. T-states nominais por instrução não equivalem a precisão ciclo a ciclo.
6. **Núcleo determinístico:** CPU, barramento e dispositivos avançam sob scheduler/modelo de eventos explícito. Não separar CPU, VDP e PSG em threads independentes sem sincronização formal. Frontend, apresentação e saída de áudio podem usar buffers isolados sem alterar clocks lógicos.
7. **ABI de fronteira grossa:** evitar chamadas Python↔Rust ou Rust↔JIT por opcode. Definir contrato nativo estreito e versionável para estado, execução de bloco e callbacks necessários.
8. **Medição antes da promoção:** comparar cargas idênticas e equivalência; separar custos de compilação, inicialização, warm-up e execução steady-state.
9. **Preparar multithreading sem paralelismo prematuro:** definir ownership de estado, filas e pontos de sincronização desde já, mas manter CPU, barramento, VDP e PSG no mesmo domínio lógico determinístico até que profiling e o scheduler provem que uma divisão é segura. Threads são uma opção, não uma exigência.

## Camadas-alvo

| Camada | Responsabilidade | Estratégia |
|---|---|---|
| Frontend Python/Qt | UI, configuração, ROM local, diagnóstico e apresentação | Permanece em Python inicialmente |
| Máquina/plataforma | Mapa de memória e portas, clocks, reset, IRQ e ligações | Python de referência; migrar contratos estáveis seletivamente |
| CPU Z80 | Instruções, flags, registradores, interrupções e contadores | Python oráculo + Rust interpretado + JIT opcional |
| Barramento e memória | ROM/RAM, portas, wait states e efeitos observáveis | Modelo explícito; Rust após contratos/testes |
| Scheduler temporal | Ordenação determinística de CPU, VDP, PSG e interrupções | Projetar antes de alegar precisão ciclo a ciclo |
| TMS9918A VDP | Registradores, VRAM, portas, raster, sprites e framebuffer | Python oráculo + Rust selecionável; ampliar precisão por evidência |
| PSG SN76489AN | Registradores, divisores, tons, ruído, mistura e amostragem | Python oráculo + Rust após testes de áudio |
| Entrada | Estado de joystick/gamepad e eventos | Fronteira de eventos; input não altera clocks internos |
| LLVM ORC | Tradução Z80→LLVM IR→código nativo e cache de blocos | Backend independente, opcional e desativado por padrão |
| Saída audiovisual | Buffer de áudio, escala/apresentação e sincronização com host | Isolar do relógio lógico; medir latência e buffering |

## Sequência de marcos

### M0 — Baseline e contratos
- Confirmar branch/base e CI das PRs nativas existentes.
- Fixar workloads sintéticos determinísticos e ROM homebrew legal quando disponível.
- Capturar estado de CPU, RAM, portas, T-states nominais, hashes de framebuffer e áudio.
- Registrar limitações temporais e construir matriz de equivalência.

**Gate:** baseline reproduzível e suíte de referência executável; nenhuma promoção de backend.

### M1 — Z80 Rust interpretado
- Completar cobertura por família/prefixo a partir da matriz Z80.
- Validar flags, PC/SP, interrupções, HALT, memória/I/O e T-states nominais contra Python.
- Manter API selecionável e Python como padrão.
- Evitar callback Python por instrução.

**Gate:** testes diferenciais e workflow Windows x64 aprovados; divergências conhecidas documentadas.

### M2 — Contratos nativos da máquina
- Especificar interfaces de memória, portas, interrupções e avanço temporal.
- Migrar ROM/RAM e barramento após definir semântica e testes de efeitos colaterais.
- Definir entrega de eventos; T-states agregados não bastam automaticamente para sincronização exata.

**Gate:** testes de barramento, mapa de memória, I/O e determinismo sem regressão Python.

### M3 — TMS9918A Rust
- Validar portas, registradores, buffer de leitura, modos de vídeo, sprites e framebuffer contra Python.
- Comparar snapshots no mesmo estado e ROM/revisão fixadas; usar Ares como referência auxiliar com versão/commit registrado.
- Evoluir raster/fetch por dot e contenção somente com fontes e testes adequados.

**Gate:** paridade nos casos cobertos, regressões reproduzíveis e limitações restantes listadas.

### M4 — PSG Rust
- Separar contrato digital da saída de áudio do host.
- Testar divisores/frequências, volumes, formas de onda, ruído, sequências de registradores, amostras e determinismo.
- Comparar assinaturas de áudio em cargas sintéticas e documentar limitações analógicas.

**Gate:** equivalência digital nos casos cobertos e medição de latência/custo.

### M5 — Scheduler, integração e fronteiras de concorrência
- Definir ordenação de eventos CPU–VDP–PSG, interrupções, I/O e estados de espera.
- Declarar ownership do estado mutável e os eventos que podem atravessar fronteiras entre threads.
- Manter CPU, barramento, VDP e PSG serializados inicialmente; não usar uma thread por dispositivo como padrão.
- Permitir threads para apresentação, saída de áudio e carregamento quando não alterarem diretamente o estado lógico emulado.
- Definir filas/buffers, backpressure, encerramento e sincronização de frames sem deixar os clocks do host controlar clocks de hardware.
- Testar execução por frame e por instrução, determinismo e regressões visuais/sonoras.

**Gate:** estado final e assinaturas audiovisuais estáveis nos workloads aceitos.

### M6 — LLVM ORC JIT Z80
- Expandir tradução em subconjuntos com limites de bloco explícitos.
- Implementar side exits para I/O, interrupções, HALT, acessos mapeados e instruções não suportadas.
- Implementar cache e invalidação para código modificável, com estado de PC/flags/interrupções correto e fallback seguro.
- Testar diferencialmente cada subconjunto contra Python e Rust.
- Validar build/distribuição Windows x64 e custo do runtime LLVM.

**Gate:** paridade do subconjunto, fallback e invalidação testados; JIT segue opt-in até benchmark e compatibilidade aprovados.

### M7 — Benchmark e decisão de promoção
- Executar os três backends na mesma carga e sob as mesmas semânticas.
- Separar build, inicialização, compilação JIT, warm-up e steady-state.
- Publicar mediana, p95, instruções/s, T-states/s, throughput por frame, custos de fronteira e hashes como artefatos de CI.
- Medir workload sintético e ROM homebrew; não extrapolar um microbenchmark para todos os jogos.

**Gate:** decidir por workload e publicar trade-offs. Nenhum backend se torna padrão com base em um único microbenchmark.

## Política de precisão e fallback

- **Referência:** Python, com comportamento e limitações documentados.
- **Rust interpretado:** backend selecionável; equivalência exigida para o escopo anunciado.
- **LLVM JIT:** experimental, separado e não padrão. Não pode ignorar eventos nem aproximar temporização silenciosamente.
- **Modo rápido:** aproximações somente opt-in, enumeradas e testadas; não confundir com HLE.
- **Fallback:** bloco não suportado deve sair antes de alterar estado ou usar side exits cuja semântica seja testada. Nunca reexecutar do PC original depois de efeitos parciais observáveis.
- **Incerteza:** registrar hipóteses e pendências em vez de assumir comportamento não comprovado.

## Estratégia multithread para sistemas posteriores

Preparar paralelismo seguro agora não significa executá-lo em todos os componentes. No SG-1000, manter o estado lógico no scheduler determinístico e usar threads periféricas apenas quando isoladas e justificadas por profiling. No Mega Drive, Sega CD e 32X, modelar explicitamente a relação temporal entre CPUs e recursos compartilhados. Processadores emulados podem avançar em fatias independentes entre pontos de sincronização, mas acessos compartilhados, arbitragem e interrupções devem respeitar a ordem definida pelo modelo de hardware. Se a execução paralela não reproduzir os resultados observáveis da execução serial de referência, usar o caminho serial.

A estratégia não autoriza paralelizar CPU/VDP/PSG antes de o scheduler e os benchmarks justificarem a mudança.

## Critérios globais de aceitação

1. Mudanças pequenas, isoladas e reversíveis; sem remover a referência Python.
2. Testes unitários, integração, diferenciais, determinismo e regressão aplicáveis.
3. CI Windows x64 e ferramentas relevantes aprovadas antes de promover backend.
4. Nenhuma alegação de precisão ciclo a ciclo sem modelo e evidência ciclo a ciclo.
5. Benchmarks com commit, ambiente, workload e hashes registrados.
6. Sem ROM comercial, BIOS protegida ou artefatos locais sensíveis no repositório/CI.
7. Cada etapa atualiza README, documentação de hardware, matriz de cobertura e ADR quando necessário.

## Trabalho já rastreado

- PR #51: infraestrutura LLVM ORC; validar e integrar conforme sua CI.
- PR #53: núcleo Z80 Rust selecionável; não assumir aprovado sem CI e revisão.
- PR #70: TMS9918A Rust selecionável; não assumir aprovado sem CI e revisão.
- PR #71: tradutor Z80 LLVM inicial, empilhado sobre a infraestrutura ORC; protótipo sem integração ou benchmark.
- Issue #47: migração incremental do Z80 para Rust.
- Issue #54 / #68: avaliação e tradutor JIT Z80.
- Issue #69: benchmark comparável Python/Rust/LLVM.

Este roteiro não mescla nem substitui PRs existentes. Dependências e resultados de CI devem ser confirmados antes de avançar.
