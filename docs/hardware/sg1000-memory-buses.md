# SG-1000 — memória e barramentos

## Escopo desta etapa

Esta etapa fornece uma base testável para a RAM principal, a VRAM separada,
o barramento de memória, o roteador de I/O e a interface CPU–VDP. Ela não
equivale a uma máquina SG-1000 completa e não implementa renderização do VDP.

## Componentes

- `SG1000Ram`: 1 KiB de RAM física com espelhamento na região `C000h–FFFFh`,
  conforme o mapa modelado atualmente.
- `SG1000MemoryBus`: ROM linear opcional no início do espaço de 64 KiB e
  RAM espelhada; lacunas não mapeadas geram erro explícito.
- `TMS9918VRAM`: 16 KiB independentes, endereçamento físico de `0000h–3FFFh`.
- `SG1000IOBus`: recebe endereço de I/O Z80 de 16 bits e decodifica o byte
  inferior. Decodificações/espelhamentos adicionais devem ser adicionados com
  evidência do hardware específico.
- `TMS9918`: portas de dados e controle `BEh/BFh`, sequência de dois bytes
  para endereço e registradores, buffer de leitura, incremento de endereço e
  bit de VBlank configurável para testes.
- `create_memory_system()`: conecta os componentes e mapeia as portas do VDP.

## Limitações conhecidas

1. O barramento de memória ainda não modela todos os ciclos, estados de espera,
   contenção ou arbitragem física.
2. O modelo TMS9918 ainda não gera pixels nem implementa modos gráficos,
   sprites, prioridade, colisões, interrupções completas ou temporização de
   linha/quadro. O bit de VBlank é definido explicitamente; ainda não existe
   escalonador de hardware que o gere automaticamente.
3. O modelo do VDP e o mapa de portas precisam ser verificados contra a revisão
   concreta do SG-1000 alvo e testes de referência antes de alegar precisão
   ciclo a ciclo.
4. A ROM é linear e não implementa mappers de cartucho.
5. A RAM permanece em 1 KiB para o SG-1000; a configuração de 2 KiB do
   SC-3000 deve ser uma configuração de plataforma separada.

## Testes

Os testes unitários cobrem capacidade e limites da VRAM, roteamento de I/O,
acesso bufferizado ao VDP, escrita de registradores, limpeza do bit de VBlank
ao ler status e wrap de endereço. Os testes devem ser executados com pytest
no ambiente Python configurado no `pyproject.toml`.
