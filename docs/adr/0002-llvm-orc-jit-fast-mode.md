# ADR 0002 — LLVM ORC JIT e modo rápido

- **Status:** Aprovada para prototipagem; integração com a máquina pendente.
- **Data:** 2026-10-24

## Contexto

O perfil de desempenho compartilhado anteriormente apontou custo relevante no caminho da CPU em Python. A meta de longo prazo inclui o 32X, cuja carga de CPU torna importante reduzir overhead de interpretação. O projeto aceita oferecer um perfil mais rápido mesmo que ele abra mão de parte da precisão temporal.

## Terminologia

- **JIT:** traduz blocos de instruções da CPU em código nativo do host durante a execução.
- **HLE:** substitui comportamentos específicos de hardware/firmware por implementações de nível superior.
- **LLVM ORC:** infraestrutura de compilação e ligação JIT; não é, por si só, HLE.

Essas técnicas devem permanecer separadas, embora possam coexistir num perfil de compatibilidade rápida.

## Decisão

1. Prototipar o JIT com LLVM ORC começando pelo Z80 do SG-1000, onde existe uma referência Python para comparação.
2. Traduzir blocos básicos para LLVM IR e código nativo, começando com um subconjunto de instruções.
3. Usar fallback para o interpretador quando o tradutor encontrar instruções, efeitos de barramento ou estados que ainda não suporta.
4. Manter dois perfis explícitos: `accurate` (interpretador e temporização atual) e `fast` (JIT habilitado e aproximações temporais documentadas).
5. Em ambos os perfis, preservar determinismo, efeitos observáveis de I/O, interrupções e ordenação de eventos. O perfil rápido poderá reduzir granularidade de despacho/sincronização apenas onde testes demonstrarem segurança.
6. Tratar HLE de serviços ou subsistemas como camada opt-in separada, nunca como substituição genérica do Z80.
7. Não tornar o JIT o padrão até que testes diferenciais, benchmark de ROM e critérios de regressão sejam satisfeitos.

## Limites de bloco e invalidade

- Encerrar blocos em desvios, instruções de controle de fluxo, acesso a I/O, interrupções e pontos de sincronização.
- Não compilar através de efeitos de barramento que ainda não sejam representados explicitamente.
- Invalidar blocos quando uma escrita puder modificar bytes de código traduzidos; se a região executável não puder ser classificada com segurança, usar o caminho interpretado.
- A cache JIT deve ter chaves estáveis por endereço e contexto de mapeamento de memória, e invalidade testada.

## Critérios de aceitação

- Build e smoke test de LLVM ORC no Windows x64.
- Primeiros blocos Z80 comparados diferencialmente com a referência Python.
- Comparação de registradores, flags, PC, T-states nominais e efeitos observáveis.
- Fallback correto para instruções não suportadas e código auto-modificável.
- Benchmark sintético e ROM local, com hash do workload e resultados reproduzíveis.
- Toda aproximação temporal do modo rápido documentada e coberta por testes.

## Fora de escopo inicial

- JIT de SH-2 ou 68000.
- HLE universal de firmware.
- Multithreading da execução lógica de CPU/VDP/PSG.
- Alegação de precisão ciclo a ciclo de barramento sem um modelo e testes que a comprovem.

## Referências

- LLVM ORC Design and Implementation: https://llvm.org/docs/ORCv2.html
- Building a JIT with LLVM: https://llvm.org/docs/tutorial/BuildingAJIT1.html
