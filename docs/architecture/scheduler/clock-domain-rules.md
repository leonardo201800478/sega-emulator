# Regras de domínio de clock e tempo mestre

## 1. Unidade temporal

`MachineTime` é timestamp absoluto inteiro `u128`. Sua unidade é definida pelo perfil da máquina; não existe frequência universal embutida no tipo. O perfil deve documentar como um tick mestre se relaciona com os clocks físicos considerados.

Não confundir:
- tick mestre;
- ciclo local de CPU;
- T-state;
- M-cycle;
- ciclo de VDP;
- pixel/raster position;
- período de amostragem de áudio;
- tempo de parede do host.

## 2. Razão racional

`ClockRatio` representa:

`local_cycles_per_master_tick = numerator / denominator`

Numerador e denominador são positivos (`u64`). Razões inválidas são rejeitadas. Não converter para float.

Para intervalo mestre `dt`, razão `n/d` e fase anterior `p`:

`scaled = dt * n + p`  
`elapsed_cycles = floor(scaled / d)`  
`new_phase = scaled mod d`

A fase é resíduo racional e deve satisfazer `0 <= phase < denominator`. Aritmética é checked; overflow é erro, nunca wrap.

## 3. Chunk invariance

Avançar de A até C em uma chamada deve produzir o mesmo total de ciclos e fase que avançar A→B e B→C, desde que nenhum evento/fronteira relevante seja ignorado. O resíduo é parte do estado, não arredondamento descartável.

## 4. Registro

Cada domínio tem:
- ID estável;
- razão;
- instante de registro;
- fase inicial;
- instante atual;
- ciclos locais acumulados;
- fase residual atual.

Registro no Scheduler atual ocorre no timestamp corrente. IDs duplicados falham sem mutação. Domínios são mantidos em `BTreeMap` e avançados/exportados por ID crescente.

## 5. ClockEdgeTime

`ClockEdgeTime` calcula posição exata de borda como parte inteira mais fração racional reduzida. Comparações devem evitar float e preservar exatidão.

**Limite importante:** a fila de eventos usa `MachineTime` inteiro. `ClockEdgeTime` não significa que eventos possam ser despachados entre ticks inteiros. Não converter a fração em `HardwarePhase`.

## 6. Frequências de hardware

Frequência nominal/divisor/fase inicial não devem ser copiadas de uma tabela genérica sem identificar:
- sistema e região;
- revisão de placa/ASIC;
- fonte primária ou medição;
- tolerância e arredondamento;
- relação de fase entre fontes;
- convenção escolhida para o tick mestre.

Quando os dados não forem suficientes, registrar `UNKNOWN` e bloquear integração dependente dessa relação.

## 7. Múltiplos domínios

CPU, VDP, PSG, FM, DMA e controladores podem ter domínios distintos. Mesma frequência nominal não prova mesma fase. Um componente pode possuir vários domínios se o hardware justificar.

Nenhum componente mantém seu próprio relógio mestre. Contadores locais podem existir como estado derivado, mas devem avançar exclusivamente a partir dos deadlines concedidos pela timeline global.

## 8. Reset, pause e gating

A razão atual exige valor positivo. Pause/gating não é representado por ratio zero. O perfil deve definir explicitamente:
- quais domínios param;
- quais continuam;
- se a fase é preservada ou reinicializada;
- o que ocorre com eventos pendentes;
- como reset parcial/global afeta a fila e watermark.

Até haver decisão por hardware, não implementar semântica genérica presumida.
