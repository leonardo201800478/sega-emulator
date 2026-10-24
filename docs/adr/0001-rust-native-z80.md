# ADR 0001 — Núcleo nativo Z80 em Rust

- **Status:** Aceita para migração incremental; integração com a máquina ainda pendente.
- **Data:** 2026-10-24

## Contexto

O núcleo Python oferece legibilidade e uma referência executável, mas a medição de desempenho compartilhada anteriormente indica custo relevante no caminho da CPU. A meta de longo prazo inclui plataformas mais complexas, culminando em Mega Drive, Sega CD e 32X. Precisamos reduzir overhead sem comprometer determinismo, cobertura de instruções ou contratos de hardware.

## Decisão

1. Rust será a linguagem prioritária para o primeiro núcleo nativo.
2. A implementação Python será mantida como referência e fallback até que a equivalência seja demonstrada.
3. PyO3 e Maturin serão usados para integração com Python, com chamadas em lotes/blocos em vez de uma chamada por opcode.
4. A migração será incremental, guiada por testes diferenciais, benchmarks reproduzíveis e revisão das semânticas de flags, registradores, memória, interrupções e T-states.
5. LLVM JIT/ORC, HLE e multithreading não fazem parte do primeiro marco. Serão reavaliados com base em perfil medido e requisitos de precisão.
6. Não será alegada precisão de ciclo de barramento enquanto o modelo e os testes só validarem T-states nominais por instrução.

## Alternativas consideradas

- **Manter somente Python:** simples para desenvolvimento, mas não resolve o overhead do interpretador no caminho crítico.
- **C++:** também pode oferecer desempenho nativo comparável; não foi escolhido para o primeiro núcleo porque Rust oferece segurança de memória por padrão e uma integração Python viável.
- **JIT imediato:** potencialmente mais rápido em workloads específicos, mas acrescenta compilação dinâmica, invalidação de blocos e complexidade temporal antes de termos um núcleo nativo de referência.
- **Reescrever tudo de uma vez:** rejeitado pelo risco de regressões e pela dificuldade de localizar divergências.

## Consequências

- O repositório terá uma crate Rust isolada enquanto a implementação cresce.
- O primeiro recorte de opcodes serve apenas para validar toolchain, bindings e testes; não substitui a CPU Python.
- O caminho de integração à máquina SG-1000 só será habilitado após cobertura e equivalência suficientes.
- Ganhos de desempenho serão reportados apenas com medições comparáveis em workloads sintéticos e ROMs locais legalmente obtidas.
