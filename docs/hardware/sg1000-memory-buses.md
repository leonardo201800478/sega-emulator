# SG-1000 — memória, portas e integração inicial

## Escopo

Este documento descreve a arquitetura que está efetivamente presente na primeira
integração executável do SG-1000. Não deve ser confundido com a proposta
alternativa de barramentos modulares da PR #19: esta integração usa a composição
direta de `SG1000Machine`, com o VDP mantendo sua própria VRAM.

## Componentes presentes

- `SG1000Ram` (`src/sega_emulator/bus/memory.py`): 1 KiB de RAM física,
  espelhada em `C000h–FFFFh`. A RAM é inicializada com bytes zerados por
  convenção determinística do software; isso não afirma que a RAM real ligue
  zerada.
- `SG1000MemoryBus` (`src/sega_emulator/bus/memory_bus.py`): mapeia ROM
  linear e RAM. Rejeita escrita em ROM e endereços não mapeados.
- `_CartridgeMirrorBus` (`src/sega_emulator/machine/sg1000.py`): adaptador
  privado que espelha a ROM fornecida ao longo da região de cartucho por módulo
  do tamanho da imagem. É uma simplificação para cartuchos pequenos, não um
  mapper universal; banking, SRAM e hardware especial de cartucho não são
  implementados.
- `SG1000Machine`: conecta CPU Z80, barramento de memória, VDP e entradas
  iniciais de joystick. Avança o VDP após cada instrução usando os T-states
  nominais retornados pela CPU.
- `TMS9918A` (`src/sega_emulator/video/tms9918.py`): mantém 16 KiB de VRAM,
  registradores, portas de dados/controle, status, framebuffer e renderização
  por software dos modos implementados nesta etapa.
- Frontend Pygame opcional: apresenta o framebuffer e converte teclado em bits
  de entrada; o modo headless permite executar quadros sem abrir janela.

## Mapa de memória atual

| Endereços da CPU | Tratamento implementado | Limitações |
|---|---|---|
| `0000h–BFFFh` | ROM de cartucho por meio do adaptador de espelhamento | A imagem precisa ter entre 1 byte e 48 KiB; não há banking nem SRAM de cartucho |
| `C000h–FFFFh` | RAM de 1 KiB, com espelhamento | Não há perfil de expansão de RAM nesta integração |

O barramento base `SG1000MemoryBus` sozinho não espelha automaticamente uma
ROM curta; essa política fica no adaptador privado usado por `SG1000Machine`.
Essa diferença deve permanecer explícita nos testes para não generalizar o
comportamento do adaptador para todo tipo de cartucho físico.

## Portas de I/O atuais

A CPU tem espaço de I/O separado do espaço de memória. A máquina encaminha as
portas usando o byte inferior do endereço de I/O:

| Porta | Direção | Dispositivo | Estado |
|---|---|---|---|
| `BEh` | leitura/escrita | Porta de dados do VDP | Implementada |
| `BFh` | leitura/escrita | Porta de controle/status do VDP | Implementada |
| `DCh` | leitura | Joystick 1 | Implementação inicial, ativa em nível baixo |
| `DDh` | leitura | Joystick 2 | Implementação inicial, ativa em nível baixo |
| PSG | escrita | SN76489A | Não conectado; sem áudio |

A decodificação é deliberadamente inicial. O tratamento de endereços de 16 bits
de I/O usa o byte inferior e não representa por si só a decodificação elétrica
completa, aliases, todos os sinais de controle ou periféricos de variantes
regionais. Não reutilizar automaticamente o mapa de portas do Master System.

## Temporização e sincronização

- A CPU executa uma instrução por chamada a `step()` e retorna um custo nominal
  em T-states.
- Depois da instrução, `SG1000Machine` avança o VDP por esse número de
  T-states e atualiza a solicitação de INT conforme o estado de interrupção do
  VDP.
- Uma instrução pode atravessar uma fronteira de scanline ou quadro. Portanto,
  eventos do VDP não estão alinhados a cada T-state nem a cada acesso ao
  barramento.
- O modo headless e os testes sintéticos verificam o caminho de execução, mas
  não provam compatibilidade com jogos comerciais nem precisão de ciclo.
- O PSG não está integrado; não há sincronização audiovisual nem saída de som.

## Testes existentes e lacunas

Os testes em `tests/unit/test_sg1000_machine.py` verificam avanço de quadro,
espelhamento de uma ROM pequena, entradas iniciais de joystick, escrita nos
registradores do VDP e uma ROM de diagnóstico gerada localmente.
Os testes em `tests/unit/test_tms9918.py` cobrem operações de VRAM, registradores,
status/VBlank e o framebuffer de diagnóstico.

Antes de declarar compatibilidade ampliada, ainda é necessário adicionar ou
aprofundar:

1. testes diferenciais do TMS9918A para todos os modos, sprites, prioridade,
   colisão, overflow e interrupções;
2. validação do mapa de I/O contra esquemas/manuais e comportamento observado;
3. testes para limites e variantes de cartucho, sem assumir que o espelhamento
   modular funciona para todas as imagens;
4. execução reproduzível de pelo menos uma ROM real legalmente obtida;
5. sincronização temporal por eventos e, futuramente, PSG com testes de frequência
   e regressão de amostras.

A infraestrutura modular `SG1000IOBus`/`TMS9918VRAM` proposta em outro ramo não
está conectada a esta máquina. Se ela for reaproveitada futuramente, a integração
deverá ser feita explicitamente, preservando os contratos e testes atuais; não
devemos manter duas implementações concorrentes do mesmo dispositivo sem uma
decisão arquitetural documentada.
