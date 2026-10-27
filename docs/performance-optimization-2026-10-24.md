> **Atualização de estado (2026-10-27):** o hardware SG-1000 ativo é Rust-only. A implementação Python antiga e ferramentas/testes que dependiam dela estão em `legacy/python_reference/`; não fazem parte do runtime e os comandos históricos abaixo não são caminhos de execução ativos.

# Registro de otimização de desempenho — 24/10/2026

## Objetivo

Registrar as otimizações feitas no núcleo SG-1000, os métodos de medição, os resultados observados, os experimentos descartados e o trabalho ainda necessário para reduzir o uso de CPU.

## Resumo

O perfil do jogo mostrou que o custo dominante está na execução do Z80 em Python. Foram reduzidas chamadas e validações no caminho quente da CPU, agrupados os caminhos de despacho mais frequentes e acelerada a escrita de pixels do TMS9918A.

Na medição do Wonder Boy com a ROM carregada corretamente, a mediana observada passou de **41,32 FPS para 53,68 FPS** nesta rodada, cerca de **30% mais FPS** e aproximadamente **23% menos tempo de emulação por quadro**. Ainda não atinge os 59,92 FPS nominais: 53,68 FPS correspondem a cerca de 89,6% dessa taxa.

O resultado não representa uma melhoria de 10×. A implementação continua em Python e o Z80 permanece como o maior custo. Emuladores SG-1000 maduros, como Gearsystem, usam C++ para o núcleo Z80/VDP. A documentação do MAME descreve a interpretação por opcode e, como alternativa mais complexa, recompilar blocos de código para reutilizar o trabalho de decodificação.

## Alterações no código

### CPU Z80 — `src/sega_emulator/cpu/z80.py`

- O fetch de opcode lê a memória diretamente e atualiza `_pc`, sem chamar `_fetch_byte()` como etapa intermediária. O fetch de operandos também atualiza `_pc` diretamente. A interface pública de `pc` continua validada; esses caminhos internos mantêm o contador dentro de 16 bits.
- As operações internas de pilha atualizam `_sp` diretamente e evitam passar pelos getters/setters públicos em cada byte de `PUSH` e `POP`.
- O acompanhamento do latch Q usa `_step_depth` como estado ativo/inativo. Chamadas recursivas usadas por prefixos passam diretamente ao decodificador, sem incrementar/decrementar a profundidade em cada chamada. O `finally` do passo externo restaura o estado se houver exceção.
- As condições Z80 são consultadas por uma tabela pré-calculada, evitando criar uma tupla de condições em cada avaliação.
- As famílias `LD r,r'` e `ALU A,r` foram agrupadas no despacho para evitar testar cada faixa de opcode em sequência.
- Foram adicionados caminhos diretos para `JR NZ,e` (`20h`) e `LD HL,nn` (`21h`), que são muito frequentes no Wonder Boy analisado. Eles preservam ciclos, PC, WZ, latch Q e atraso de EI.

### Máquina SG-1000 — `src/sega_emulator/machine/sg1000.py`

- `run_frame()` mantém referências locais para os métodos e dispositivos usados no laço CPU/VDP.
- No caminho normal, o laço executa o decodificador Z80 diretamente, atualizando Q antes de cada instrução. Isso elimina o wrapper `cpu.step()` do ciclo repetido. Se `step()` estiver sobrescrito ou instrumentado, o código usa o caminho compatível com `step()`.
- A verificação da linha de interrupção lê diretamente o bit VBlank do status do VDP e o bit de habilitação do registrador 1, evitando uma chamada de propriedade por instrução. `SG1000Machine.step()` continua disponível para execução individual.

### VDP TMS9918A — otimizações já presentes no commit `f8d1569`

- `advance()` tem um caminho curto quando os T-states da instrução não alcançam o fim da scanline; a lógica completa de limite permanece para instruções que cruzam a linha.
- As linhas RGB de fundo e os trechos de quatro pixels de uma cor são pré-calculados e reutilizados.
- Um cache limitado de spans RGB converte padrão e cores de tile em blocos de bytes. Graphics I, Graphics II, texto e multicolor escrevem blocos por fatia de framebuffer, reduzindo chamadas a `_put_pixel()`.
- A rasterização por scanline e `render_frame()` usam os mesmos spans para os modos correspondentes.

### ROM de cartucho — otimizações já presentes no commit `f8d1569`

- A janela de ROM espelhada de `0000h–BFFFh` é expandida uma vez na criação da máquina. Leituras não precisam calcular `address % rom_size` nem atravessar novamente o barramento base.
- A CPU usa um caminho interno de fetch para endereços já limitados a 16 bits; CPUs com barramentos genéricos continuam usando o método normal.

## Perfil do jogo e escolha das instruções quentes

Foi feita uma passagem instrumentada de 120 quadros após 30 quadros de aquecimento. A contagem dos bytes de opcode mais comuns foi:

| Opcode | Instrução | Ocorrências |
|---|---|---:|
| `20h` | `JR NZ,e` | 142.683 |
| `21h` | `LD HL,nn` | 101.295 |
| `7Eh` | `LD A,(HL)` | 100.201 |
| `B7h` | `OR A` | 98.161 |
| `B0h` | `OR B` | 62.439 |
| `D3h` | `OUT (n),A` | 46.960 |
| `79h` | `LD A,C` | 42.005 |
| `0Bh` | `DEC BC` | 42.004 |
| `1Ah` | `LD A,(DE)` | 28.062 |
| `13h` | `INC DE` | 28.016 |

Essa contagem serviu para escolher os primeiros fast paths; ela não é uma medição de tempo por instrução, pois o contador adiciona overhead.

## Medições

### ROM real: Wonder Boy (Japan) (Rev 1)

O arquivo `.zip` foi carregado pelo `load_rom()` do projeto, que seleciona o membro `.sg` antes de criar a máquina. Cada rodada usou 30 quadros de aquecimento e 120 quadros medidos; a tabela registra três repetições.

| Versão medida | Tempos para 120 quadros | Mediana | FPS mediano | Estado final |
|---|---|---:|---:|---|
| Antes desta rodada de mudanças no Z80/laço da máquina | 2,9041 s; 2,9479 s; 2,8328 s | 2,9041 s | 41,32 | vídeo ativo, modo 4 |
| Depois das mudanças | 2,2354 s; 2,2748 s; 2,2339 s | 2,2354 s | 53,68 | vídeo ativo, modo 4 |

As duas medições executaram em média cerca de **7.057,8 instruções** e **59.736 T-states** por quadro. O tempo por quadro caiu de aproximadamente 24,2 ms para 18,6 ms. Essa medição é da máquina em linha de comando, não uma nova medição do tempo `EMU` mostrado pelo frontend.

### Perfil sintético do VDP

O script `tools/benchmarks/profile_emulation.py` usa uma ROM de NOPs e uma cena Graphics I sintética; não é um benchmark de jogo.

| Métrica | Antes das otimizações iniciais | Depois das otimizações iniciais |
|---|---:|---:|
| CPU isolada por quadro | 14,263 ms | 10,102 ms |
| VDP isolado por quadro | 24,657 ms | 6,361 ms |
| Integrado | 24,34 FPS | 50,81 FPS |

Esse perfil mostrou um ganho grande na renderização sintética, mas não deve ser usado para projetar o ganho de um jogo real.

### Diagnóstico histórico inválido para o jogo

`logs/runtime-diagnostic-wonder-boy.json` registra o caminho de um arquivo `.zip`, mas o script de diagnóstico leu todos os bytes do ZIP e os passou diretamente a `SG1000Machine`, sem extrair a ROM interna. O resultado histórico — 32,48 FPS e vídeo desativado — **não é uma linha de base válida do Wonder Boy**. As medições acima usam o carregador correto e confirmam vídeo ativo.

O screenshot fornecido pelo usuário mostrava 32,7 FPS, com `EMU 27,6 ms`, `QImage 0,05 ms`, escala 0,63 ms e UI 0,09 ms. Isso apontava o núcleo de emulação como o custo principal naquela captura; não foi usado como comparação controlada antes/depois.

## Experimento descartado

Foi experimentada a execução da CPU em blocos até o próximo limite de scanline, com sincronização antes de acessos às portas do VDP. A mediana caiu de aproximadamente 41,3 para 39,9 FPS. O controle adicional em Python custou mais do que as chamadas ao VDP removidas. A alteração foi revertida; a sincronização atual continua após cada instrução.

## Pesquisa técnica

- [Gearsystem](https://github.com/piepacker/gearsystem) é um emulador C++ de Master System, Game Gear e SG-1000, com núcleo Z80 e VDP precisos. Serve como referência de implementação nativa para a mesma família de hardware.
- O [guia de recompilação dinâmica do MAME](https://wiki.mamedev.org/index.php?title=Dynamic_Recompiler_Author%27s_Guide) descreve a interpretação opcode a opcode e a tradução de blocos para um cache de código.
- [Core Concepts do MAME](https://wiki.mamedev.org/index.php/Core_Concepts) ressalta a relação entre desempenho e complexidade de um recompilador dinâmico e observa que CPUs clássicas como o Z80 também podem ser atendidas por um interpretador bem otimizado.

O código deste projeto continua em Python. Na inspeção desta sessão, Cython e compiladores C/C++/Rust não estavam disponíveis no ambiente. Portanto, não foi criado nem medido um backend nativo. Para reduzir mais o tempo por instrução, a próxima investigação de maior impacto é um núcleo Z80 compilado que preserve o núcleo Python como referência e sincronize o VDP nas mesmas fronteiras observáveis.

## Verificação registrada

- Foram executados benchmarks do Wonder Boy, perfil sintético do VDP, perfil de chamadas e contagem instrumentada de opcodes.
- `git diff --check` passou.
- A suíte de testes não foi executada durante as alterações de desempenho; os números acima demonstram desempenho medido, não equivalência funcional abrangente.
