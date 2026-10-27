> **Atualização de estado (2026-10-27):** o hardware SG-1000 ativo é Rust-only. A implementação Python antiga e ferramentas/testes que dependiam dela estão em `legacy/python_reference/`; não fazem parte do runtime e os comandos históricos abaixo não são caminhos de execução ativos.

# SG-1000 — memória, portas e integração inicial

## Escopo

Este documento descreve a arquitetura que está efetivamente presente na primeira
integração executável do SG-1000. Não deve ser confundido com a proposta
alternativa de barramentos modulares da PR #19: esta integração usa a composição
direta de `SG1000Machine`, com o VDP mantendo sua própria VRAM.

## Componentes presentes

- `SG1000Ram` (`src/sega_emulator/bus/memory.py`): RAM interna física de 1 KiB
  espelhada em `C000h–FFFFh` por padrão, ou 8 KiB para perfis Type-B
  conhecidos de cartucho SG-1000 II com adaptador de expansão. `The Castle
  (Japan)` mantém 1 KiB interno e recebe 8 KiB separados no mapa do cartucho. A RAM é
  inicializada com bytes zerados por convenção determinística do software; isso
  não afirma que a RAM real ligue zerada.
- `cartridge_profiles.py`: reconhece por CRC-32 os cinco dumps catalogados
  pelo Genesis Plus GX como dependentes da expansão Type-B de 8 KiB, além do
  SHA-256 da variante Bomberman English Logo. Reconhece também `The Castle
  (Japan)` (CRC-32 `092F29D6`) como cartucho com 8 KiB em `8000h–BFFFh`. Dumps
  desconhecidos continuam usando 1 KiB; não inferimos hardware apenas pelo
  tamanho ou pelo fato de uma ROM ser não licenciada.
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
- Interface ativa: Qt/PySide6 apresenta o framebuffer e converte entrada de host em snapshots para o runtime Rust. O frontend Pygame e o runner headless da CLI foram arquivados.

## Mapa de memória atual

| Endereços da CPU | Tratamento implementado | Limitações |
|---|---|---|
| `0000h–7FFFh` | ROM de cartucho por meio do adaptador de espelhamento | A imagem precisa ter entre 1 byte e 48 KiB; não há banking |
| `8000h–BFFFh` | ROM espelhada normalmente; para `The Castle (Japan)`, RAM de cartucho de 8 KiB espelhada duas vezes | Seleção explícita por CRC-32 `092F29D6`; outros cartuchos permanecem somente leitura |
| `C000h–FFFFh` | RAM interna padrão de 1 KiB espelhada; perfis Type-B conhecidos usam 8 KiB espelhados na janela de 16 KiB | Perfis selecionados por hashes conhecidos; outras expansões e mapeadores não são inferidos |

O barramento base `SG1000MemoryBus` sozinho não espelha automaticamente uma
ROM curta; essa política fica no adaptador privado usado por `SG1000Machine`.
A seleção Type-B é baseada em hashes explícitos, não no tamanho da ROM. O
perfil inclui Bomberman Special (Taiwan), Magical Kid Wiz (Taiwan), The Castle
[MSX] (Taiwan), Rally-X (Taiwan) e Road Fighter (Taiwan), conforme o catálogo
de hardware do Genesis Plus GX. O CRC-32 do Bomberman Special é também
registrado no catálogo de software do MAME. Para contemplar a variante com
logotipo em inglês usada na validação local, o perfil inclui seu SHA-256
`3eff3d6f1f74041f7b732455799d0978ab49724552ff2985f34b76478cd91721`. Essa
lista é uma base de compatibilidade observada, não um detector universal de
mapeadores.

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
- Os testes sintéticos do núcleo Rust verificam o caminho de execução, mas
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
