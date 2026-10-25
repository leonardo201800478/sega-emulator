# ADR 0005 — Núcleo de referência Python, hardware em Rust e JIT LLVM opcional

- **Status:** aceito como direção arquitetural; execução incremental sujeita aos gates deste ADR.
- **Data:** 2026-10-25

## Contexto

O projeto SG-1000 começou com Python para manter o comportamento legível e testável. Já existem trabalhos separados para Z80 Rust, TMS9918A Rust e infraestrutura/tradutor LLVM ORC. Converter tudo de uma vez aumentaria o risco de divergências simultâneas e dificultaria localizar regressões.

LLVM é infraestrutura de compilação, não uma linguagem de implementação de todos os subsistemas. Traduzir blocos Z80 para código nativo não resolve, por si só, barramento, memória, vídeo, áudio, entrada ou sincronização.

## Decisão

1. Manter Python como implementação de referência, oráculo diferencial, frontend e fallback inicial.
2. Migrar seletivamente componentes críticos e contratos de hardware para Rust, começando por validar o Z80 interpretado e avançando por barramento/memória, VDP, PSG e scheduler.
3. Manter LLVM ORC como backend JIT separado para tradução Z80→LLVM IR→código nativo. Não portar barramento, RAM/VRAM, VDP, PSG, input ou frontend para LLVM.
4. Manter Python, Rust interpretado e LLVM JIT selecionáveis explicitamente. Não trocar o backend padrão antes de cumprir testes diferenciais, CI e benchmark.
5. Definir contratos nativos de granularidade grossa; evitar chamadas Python por instrução. O ABI entre Rust e JIT deve ser estreito, documentado e versionável.
6. Preservar determinismo: CPU e dispositivos avançam sob scheduler/event model explícito. Apresentação e saída de áudio podem ser isoladas, mas não alteram clocks lógicos.
7. Tratar precisão como propriedade comprovada. T-states nominais por instrução não são precisão de barramento ciclo a ciclo.
8. Separar modo de fidelidade e modo de desempenho. Aproximações de modo rápido devem ser opt-in, declaradas e testadas. HLE é independente.
9. Preparar multithreading por meio de ownership explícito, filas e pontos de sincronização, mas adiar paralelismo do estado lógico até o scheduler determinístico e o profiling justificarem a divisão. Threads periféricas para apresentação e saída podem ser usadas se não alterarem os clocks emulados.
10. Para Mega Drive, Sega CD e 32X, representar múltiplas CPUs e recursos compartilhados por um modelo temporal explícito. Não assumir que cada CPU precisa de uma thread; a equivalência observável com a execução serial é requisito.

## Consequências

- A migração incremental permite localizar divergências por componente, embora exija mais etapas que uma reescrita integral.
- Python continua útil para testar a semântica de Rust/JIT sem depender da mesma implementação.
- Rust pode reduzir overhead, mas ganhos devem ser medidos; custo PyO3/Maturin e fronteiras também entram no benchmark.
- LLVM aumenta custo de build/distribuição e pode não compensar em blocos curtos; compilação e steady-state devem ser reportados separadamente.
- JIT exige limites de bloco, side exits, invalidação de código modificável e respeito a eventos de barramento/interrupção.
- Multithreading acrescenta custos de sincronização e risco de data races; aplicar apenas a fronteiras medidas e sem alterar a ordem lógica dos eventos.
- Nenhum sistema futuro (68000, Sega CD, 32X) será implementado antecipadamente neste marco; primeiro validar SG-1000 e componentes realmente reutilizáveis.

## Plano de validação

- Testes unitários e diferenciais Python/Rust/JIT para o escopo comum.
- Testes de barramento, RAM, portas, interrupções, HALT e eventos observáveis.
- Testes de determinismo com estado inicial e entradas idênticos.
- Regressões de framebuffer/áudio e comparação com referências externas versionadas quando disponíveis.
- CI Windows x64 e benchmark reproduzível.
- Resultados de CI não observados devem permanecer como pendentes.

## Referências de trabalho

- [Roteiro do núcleo nativo](../architecture/native-core-roadmap.md)
- [Issue #47 — migração Z80 Rust](https://github.com/leonardo201800478/sega-emulator/issues/47)
- [Issue #54 — investigação JIT](https://github.com/leonardo201800478/sega-emulator/issues/54)
- [Issue #68 — tradutor LLVM ORC](https://github.com/leonardo201800478/sega-emulator/issues/68)
- [Issue #69 — benchmark comparável](https://github.com/leonardo201800478/sega-emulator/issues/69)
