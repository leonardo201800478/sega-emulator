# YM2413 / OPLL — Especificação arquitetural e modelo FM

**Projeto:** Sega Emulator — Emulação de Alta Precisão  
**Escopo:** núcleo compartilhado Rust e integração futura no Master System japonês  
**Estado:** especificação arquitetural inicial (M0); **não autoriza declarar o chip implementado**  
**Branch de documentação:** `fix/sn76489-psg-fidelity-20261107`  
**PR relacionada:** [#615](https://github.com/leonardo201800478/sega-emulator/pull/615)

> Convenção de evidência usada neste documento:
> - **DOCUMENTADO:** descrito no manual Yamaha ou documentação técnica primária.
> - **MEDIDO:** resultado de engenharia reversa com observações em hardware real.
> - **INFERIDO:** hipótese consistente com evidências, ainda não comprovada diretamente no YM2413.
> - **PENDENTE:** exige medição, comparação diferencial ou decisão arquitetural.
>
> Não converter INFERIDO/PENDENTE em comportamento normativo do core.

---

## 1. Objetivo e limites

Modelar o Yamaha YM2413 (OPLL) como um dispositivo digital determinístico, independente de console e host, com estado integral, avanço explícito de clock e integração temporal pelo `MachineScheduler`.

O núcleo deve reproduzir:
- síntese FM de 9 canais melódicos;
- dois operadores por canal, com modulator e carrier;
- 15 instrumentos predefinidos e um instrumento programável;
- geradores de fase, envelope, AM e PM;
- feedback do modulator;
- modo Rhythm com cinco vozes de percussão;
- registradores, latch, temporização e pipeline interno;
- saídas digitais que representem os pinos de áudio do chip.

Fora do escopo do core:
- decodificação de portas específicas do Master System;
- presença física do FM em cada console;
- política de seleção PSG/FM;
- mixer, resampler e dispositivo de áudio do host.

Essas responsabilidades pertencem aos adaptadores e à configuração da máquina.

## 2. Parâmetros fundamentais

| Parâmetro | Valor / regra | Evidência |
|---|---:|---|
| Clock de entrada | 3.579.545 Hz | DOCUMENTADO |
| Slots por período de saída | 18 | DOCUMENTADO |
| Clocks por slot | 4 | DOCUMENTADO |
| Clocks por período de saída | 72 | DOCUMENTADO |
| Taxa interna de saída | 3.579.545 / 72 ≈ 49.716,04 Hz | Derivada |
| Canais melódicos | 9 | DOCUMENTADO |
| Operadores lógicos | 18 (2 por canal) | DOCUMENTADO |
| Células físicas do operador | 1, reutilizada sequencialmente | DOCUMENTADO |
| Resolução do DAC | 9 bits | DOCUMENTADO |
| Instrumentos fixos | 15 + patch de usuário | DOCUMENTADO |
| Saídas | MO e RO | DOCUMENTADO |

A taxa de aproximadamente 49,716 kHz é a taxa interna do gerador, **não** a taxa final do dispositivo de áudio. A conversão para 44,1/48/96 kHz deve preservar fase e histórico do resampler entre blocos.

### 2.1 Ordem dos slots

| Slot físico | Canal | Operador |
|---:|---:|---|
| 1 | 1 | Modulator |
| 2 | 2 | Modulator |
| 3 | 3 | Modulator |
| 4 | 1 | Carrier |
| 5 | 2 | Carrier |
| 6 | 3 | Carrier |
| 7 | 4 | Modulator |
| 8 | 5 | Modulator |
| 9 | 6 | Modulator |
| 10 | 4 | Carrier |
| 11 | 5 | Carrier |
| 12 | 6 | Carrier |
| 13 | 7 | Modulator |
| 14 | 8 | Modulator |
| 15 | 9 | Modulator |
| 16 | 7 | Carrier |
| 17 | 8 | Carrier |
| 18 | 9 | Carrier |

Cada slot ocupa quatro clocks de entrada. A visita de todos os slots ocupa 72 clocks. O modelo deve manter os 18 estados lógicos, mas executar as operações na ordem sequencial observada. Não implementar 18 engines independentes em paralelo.

O processamento em lote é permitido somente quando preserva a sequência de slots, os pontos de escrita de registradores, os eventos de envelope e os limites de saída. Uma escrita no instante T exige avançar o chip até T, aplicar a escrita na fase correta e continuar a partir dali.

## 3. Modelo de síntese

### 3.1 Caminho de sinal por operador

Cada operador possui, conceitualmente:
1. Phase Generator (PG);
2. forma de onda log-sine e conversão exponencial;
3. Envelope Generator (EG);
4. controle de amplitude e sinal.

O canal usa modulator seguido de carrier. O modulator pode receber feedback de sua própria saída histórica. A saída do modulator altera a fase do carrier.

Representação matemática conceitual:

\[
y(t)=A_c(t)\sin(\phi_c(t)+I(t)\sin(\phi_m(t)))
\]

Essa equação descreve a topologia funcional, não a aritmética interna exata. Larguras, deslocamentos, truncamentos e atrasos devem ser determinados pelas medições e pelo manual.

### 3.2 Phase Generator

**DOCUMENTADO/MEDIDO**
- O contador de fase identificado nas notas de engenharia reversa tem 19 bits, com representação fixa 10.9.
- Cada operador lógico mantém seu próprio estado de fase.
- FNUM, BLOCK e MUL controlam a frequência.
- PM/VIB pode alterar o incremento de fase.

Tabela de multiplicadores do campo MUL:

| MUL | Multiplicador |
|---:|---:|
| 0 | 0,5 |
| 1–9 | 1–9 |
| A–B | 10 |
| C–D | 12 |
| E–F | 15 |

Fórmula conceitual para o incremento base:

\[
\Delta P \propto FNUM\times MUL\times 2^{BLOCK}
\]

**PENDENTE:** escala exata, posição dos bits fracionários, truncamento, tratamento de overflow, fase inicial, cadência de atualização e aplicação precisa de PM. Não codificar a expressão simplificada como fórmula final sem confronto de vetores.

### 3.3 Log-Sine / EXP

O caminho de amplitude deve utilizar representação logarítmica e conversão exponencial por tabelas inteiras. O loop de síntese não deve chamar funções trigonométricas ou logarítmicas de ponto flutuante.

As notas de engenharia reversa documentam uma tabela de seno em domínio logarítmico e uma tabela EXP de 256 entradas. Uma implementação de referência aberta, `emu2413`, contém tabelas concretas e está licenciada sob MIT; deve ser usada como referência diferencial, não como prova isolada do comportamento físico.

**PENDENTE:** estabelecer os vetores oficiais do projeto para cada entrada, convenção de arredondamento, saturação, simetria de quadrantes, meia onda e conversão final para o DAC. Tabelas copiadas de implementações não devem ser aceitas sem comparação.

### 3.4 Envelope Generator

Não usar um ADSR genérico contínuo. O EG é uma máquina de estados discreta e temporizada por hardware.

Fases a modelar:
- DAMP;
- ATTACK;
- DECAY;
- SUSTAIN;
- RELEASE;
- variantes de release e transições dependentes de KEY-ON, KEY-OFF e SUS.

Parâmetros relevantes por operador:
- AR, DR, RR, SL;
- EG-TYP;
- KSR;
- KSL;
- AM enable.

Parâmetros por canal:
- KEY-ON;
- SUS;
- BLOCK/FNUM.

A relação de taxa documentada é:

\[
RATE=4R+R_{ks}
\]

com taxa nula quando R=0. A implementação ainda precisa reconstruir o gerador de taxas, seus contadores e padrões de incremento. Não aceitar uma tabela inventada ou uma curva exponencial aproximada como substituta.

**DIVERGÊNCIA A RESOLVER:** o manual e as medições/implementações de referência não apresentam a mesma granularidade de nível/atenuação em todas as descrições. As notas de andete descrevem 128 níveis com passo de 0,375 dB; o manual contém uma especificação de atenuação que precisa ser reconciliada com o comportamento medido. Não escolher um valor sem documentar a evidência e o teste que o confirma.

### 3.5 KSR e KSL

KSR afeta a taxa do envelope por meio da escala de nota derivada de BLOCK e FNUM. KSL aplica atenuação dependente da altura da nota.

A expressão simplificada `(BLOCK << 1) | (FNUM >> 8)` pode servir como índice inicial de análise, mas não é por si só a especificação completa. A tabela, o arredondamento e a aplicação de KSR/KSL devem ser confrontados com o manual e as notas de medição.

### 3.6 Feedback

O feedback é aplicado ao modulator. As medições indicam dependência das duas saídas anteriores do modulator, não de uma saída instantânea ideal.

O manual associa os valores FB a índices de modulação:

| FB | Índice documentado |
|---:|---:|
| 0 | 0 |
| 1 | π/16 |
| 2 | π/8 |
| 3 | π/4 |
| 4 | π/2 |
| 5 | π |
| 6 | 2π |
| 7 | 4π |

O estado precisa preservar pelo menos o histórico temporal exigido pelo modelo confirmado. A média das duas últimas amostras aparece nas implementações de referência e nas análises de engenharia reversa.

**PENDENTE:** largura do sinal histórico, ponto exato de truncamento, deslocamento de feedback por FB, sinal, saturação e pipeline por slot. Não fixar `9 - FB` ou qualquer shift simplificado sem vetores diferenciais.

### 3.7 LFO AM e PM

O chip possui modulação de amplitude e de fase. A documentação de aplicação indica modulações lentas; as notas de reverse engineering investigam suas tabelas e sequências.

O estado deverá incluir:
- contador/posição de AM;
- contador/posição de PM;
- fase e divisor de cada LFO;
- valores efetivos aplicados ao EG e PG.

**PENDENTE:** período exato em clocks, forma da tabela, fase após reset, sequência inicial e ponto de aplicação em cada slot. A frequência aproximada divulgada em documentos não substitui a sequência discreta.

## 4. Instrumentos e registradores

### 4.1 Instrumentos

- Instrument ID 0: patch de usuário programável nos registradores $00–$07.
- Instrument IDs 1–15: presets internos do YM2413.
- O instrumento selecionado é aplicado aos operadores do canal.
- O patch de usuário é global/compartilhado: todos os canais que selecionam ID 0 observam o mesmo conjunto de registradores.

**PENDENTE:** validar byte a byte a ROM dos 15 instrumentos no YM2413 original. Não importar ROM de VRC7, YMF281B ou derivativos FHB013 como se fosse automaticamente a ROM do YM2413. As notas de reverse engineering alertam que algumas imagens de die analisadas são de um derivativo FHB013, não de um YM2413 original.

### 4.2 Mapa funcional de registradores

| Endereço | Função |
|---|---|
| $00–$07 | Patch de usuário (parâmetros dos dois operadores) |
| $0E | Ativação e controles do Rhythm Mode |
| $0F | Registrador de teste; comportamento não destinado ao uso normal |
| $10–$18 | FNUM LSB dos canais 1–9 |
| $20–$28 | FNUM MSB, BLOCK, KEY-ON e SUS dos canais 1–9 |
| $30–$38 | Instrument ID e volume dos canais 1–9 |

Os campos reservados e os efeitos de escrita nos registradores de teste devem ser tratados como PENDENTES até haver evidência. Não normalizar escritas inválidas para um comportamento conveniente.

## 5. Rhythm Mode

O Rhythm Mode altera a utilização dos canais 7–9 e de seus seis operadores para formar cinco vozes de percussão:

| Voz | Slots envolvidos |
|---|---|
| Bass Drum | 13 e 16 |
| Hi-Hat | 14 |
| Tom-Tom | 15 |
| Snare Drum | 17 |
| Top Cymbal | 18 |

Ao ativar Rhythm Mode, os canais 7–9 deixam de ser três canais melódicos normais. Os registradores de frequência desses canais continuam relevantes para algumas vozes; o controle de volume e disparo das percussões passa a utilizar os campos específicos do registrador de ritmo.

Não presumir que desligar individualmente uma voz de percussão libere seu operador para recuperar um canal melódico.

**PENDENTE:** equações de fase metálica, combinações de ruído e fase, taps e estado inicial do gerador pseudoaleatório, sincronização do LFSR, envelopes e saída duplicada de percussão. O estado de ruído deverá ser serializado integralmente. Não assumir um polinômio ou seed como confirmado sem comparação com hardware.

## 6. Interface de barramento do chip

O core deve expor operações explícitas equivalentes a:
- reset;
- escrita de endereço/latch;
- escrita de dados;
- avanço de clocks;
- consulta/retirada de amostras já produzidas;
- exportação e restauração de estado.

O manual especifica:
- aguardar 12 clocks mestres entre escrita de endereço e escrita de dados;
- aguardar 84 clocks mestres depois da escrita de dados antes da próxima escrita de endereço ou dados.

Se os intervalos não forem respeitados, o manual classifica os dados como incertos. Não converter automaticamente esses intervalos em WAIT states de Z80. A camada de barramento deve registrar o timestamp da transação e o core deve modelar o comportamento de escrita no instante correto.

## 7. Integração Master System

### 7.1 Separação de responsabilidades

| Camada | Responsabilidade |
|---|---|
| `sega-audio` / core YM2413 | Síntese e estado digital do chip |
| Adaptador temporal | Relógio e sincronização com o Scheduler |
| Perfil físico SMS | Modelo, revisão, região e presença do chip |
| Barramento SMS | Portas e timestamps de I/O |
| Controle de áudio SMS | Seleção de saída PSG/FM conforme hardware |
| AudioMixer | Soma das fontes |
| Resampler | Conversão para a taxa de saída |
| Host | Dispositivo físico; sem controle de ciclos do chip |

O núcleo YM2413 não deve depender de tipos do Master System.

### 7.2 Portas SMS

- $F0: latch/endereço do YM2413.
- $F1: dados do YM2413.
- $F2: detecção/controle de áudio e leitura de bits de contador no hardware correspondente.

A decodificação deve ocorrer no evento de conclusão da transação de I/O, preservando o timestamp. A máscara e os espelhos de portas devem ser confirmados no decodificador real do SMS, não inferidos apenas do byte baixo.

### 7.3 Perfis de hardware e controle $F2

Separar três conceitos:
1. capacidade física: o YM2413 existe nesta placa/revisão;
2. seleção/roteamento de saída: PSG e FM estão audíveis ou silenciados;
3. atividade de síntese: o chip continua avançando seu estado interno.

Tabela de saída documentada para o Japanese Master System:

| Valor escrito em $F2 | PSG | FM |
|---:|---|---|
| $00 | habilitado | silenciado |
| $01 | silenciado | habilitado |
| $02 | silenciado | silenciado |
| $03 | habilitado | habilitado |

No Japanese Master System, PSG e FM podem operar simultaneamente. No Mark III com FM Unit, o bit de controle do PSG não tem o mesmo efeito: a seleção é mutuamente exclusiva e o bit correspondente é forçado a zero. Não aplicar esse comportamento do Mark III ao SMS japonês.

A presença de FM não deve ser inferida apenas de `Territory::Japan`. O perfil deve representar explicitamente modelo/revisão e capacidade física. SMS exportação e variantes Tec Toy não devem receber YM2413 por suposição.

Na leitura de $F2, bits 7, 6 e 5 refletem bits 11, 7 e 3 de um contador de 12 bits associado a C-Sync. Os demais bits têm comportamento definido pela documentação de Audio Control Port. A ausência de FM possui comportamento de leitura diferente conforme o modelo; o perfil precisa tratar isso explicitamente.

### 7.4 PSG e FM simultâneos

O PSG e o YM2413:
- têm clocks e estados internos independentes;
- não devem ser pausados ou substituídos apenas porque a outra fonte está selecionada;
- devem gerar amostras de forma determinística;
- devem entrar no mixer comum com ganho/headroom documentados;
- precisam preservar a fase de reamostragem entre blocos.

A implementação atual do mixer suporta múltiplas fontes, mas a sessão de áudio deve ser auditada e ampliada para entregar PSG + FM no mesmo intervalo de saída. Não duplicar mixers por console.

## 8. Scheduler e invariância temporal

O `MachineScheduler` existente continua sendo o único dono da linha do tempo.

A integração FM requer:
- um domínio de clock registrado e validado;
- razão de clock explícita para o perfil de hardware;
- prioridade/fase de eventos documentada;
- avanço do YM2413 até cada timestamp de escrita;
- preservação de restos de clock, slot e subfase;
- processamento em lote que não ultrapasse eventos pendentes.

O ID do domínio FM ainda não está reservado nesta especificação. Não codificar um ID arbitrário sem verificar a política central de IDs e a validação do snapshot do coordenador SMS. A restauração atual do coordenador valida exatamente três domínios (Z80, PSG, VDP); adicionar FM exigirá evolução explícita do esquema e versão do estado.

## 9. Save State

Serializar, no mínimo:
- banco completo de registradores e latch;
- contador de clock e fase residual;
- slot corrente e subfase;
- estado dos 9 canais;
- estado dos 18 operadores;
- PG, EG, AM, PM;
- dois históricos de feedback por operador/canal conforme o modelo final;
- estado do Rhythm Mode e do gerador pseudoaleatório;
- pipeline de saída e amostras pendentes necessárias à equivalência;
- estado do adaptador e domínio de clock;
- controle $F2, perfil físico e roteamento SMS;
- estado do resampler e mixer que afete a continuidade.

Restaurar deve validar coerência entre timestamp do Scheduler, clock acumulado do chip, eventos pendentes, estado do barramento e fase do resampler. Não basta serializar registradores visíveis.

## 10. Estratégia de implementação incremental

1. Fechar especificação de PG, EG, AM/PM, feedback e Rhythm com evidências.
2. Fixar vetores dourados e resultados diferenciais para cada bloco.
3. Implementar núcleo puro em Rust, sem integração SMS.
4. Validar reset, registradores, clock, invariância de blocos e snapshots.
5. Implementar presets e patch de usuário.
6. Validar síntese melódica e envelopes.
7. Validar Rhythm Mode.
8. Implementar adaptador temporal e domínio no Scheduler.
9. Integrar portas $F0–$F2 por perfil físico.
10. Integrar fontes PSG + FM no mixer e no resampler.
11. Executar regressão SMS e SG-1000 e todos os gates do workspace.

## 11. Matriz de aceitação

| ID | Teste | Estado inicial |
|---|---|---|
| YM-001 | Clock de entrada e 18 slots / 72 clocks | NOT RUN |
| YM-002 | Ordem dos slots e correspondência canal/operador | NOT RUN |
| YM-003 | Escrita de endereço com latência de 12 clocks | NOT RUN |
| YM-004 | Escrita de dados com latência de 84 clocks | NOT RUN |
| YM-005 | Tabela MUL e sweep FNUM/BLOCK | NOT RUN |
| YM-006 | Vetores Log-Sine/EXP | NOT RUN |
| YM-007 | Attack/Decay/Sustain/Release e DAMP | NOT RUN |
| YM-008 | KSR/KSL e SUS | NOT RUN |
| YM-009 | AM/PM e fase inicial | NOT RUN |
| YM-010 | Feedback com histórico e atrasos | NOT RUN |
| YM-011 | Presets originais e patch de usuário | NOT RUN |
| YM-012 | Cinco vozes de Rhythm Mode | NOT RUN |
| YM-013 | Continuidade de execução contínua vs blocos | NOT RUN |
| YM-014 | Snapshot/restore bit-exato e continuidade de áudio | NOT RUN |
| YM-015 | $F2 Japanese SMS: PSG/FM simultâneos | NOT RUN |
| YM-016 | $F2 Mark III FM Unit: seleção exclusiva | NOT RUN |
| YM-017 | SMS sem FM e perfis desconhecidos | NOT RUN |
| YM-018 | Mixagem PSG + FM e resampling | NOT RUN |
| YM-019 | Regressão SG-1000 e SMS | NOT RUN |
| YM-020 | Gates Rust do workspace | NOT RUN |

Nenhum teste é PASS por ter sido ignorado ou não executado. Registrar PASS, FAIL, BLOCKED ou NOT RUN com evidência e SHA.

## 12. Referências técnicas

### Documentação primária / técnica
- Yamaha YM2413 Application Manual, espelhado em SMS Power: https://www.smspower.org/maxim/Documents/YM2413ApplicationManual
- SMS Power — YM2413: https://www.smspower.org/Development/YM2413
- SMS Power — Audio Control Port: https://www.smspower.org/Development/AudioControlPort
- SMS Power — índice das notas de reverse engineering: https://www.smspower.org/Development/YM2413ReverseEngineeringNotes
- Andete — repositório de medições e notas: https://github.com/andete/ym2413

### Implementações de referência (uso diferencial, não autoridade única)
- emu2413 (MIT): https://github.com/digital-sound-antiques/emu2413
- Nuked-OPLL: https://github.com/nukeykt/Nuked-OPLL
- ymfm: https://github.com/aaronsgiles/ymfm
- ares: https://github.com/ares-emulator/ares

Antes de reutilizar código, tabelas ou dados, verificar licença, versão/commit e proveniência. O die analisado por andete é de um derivativo FHB013; não tratá-lo como uma fotografia do silício YM2413 original.

---

## 13. Decisões congeladas nesta revisão

- Núcleo YM2413 compartilhado e independente do SMS.
- Backend de hardware 100% Rust.
- Scheduler único, sem callbacks por instrução.
- Operador processado sequencialmente em 18 slots.
- PSG + FM simultâneos no perfil Japanese SMS.
- Mark III FM Unit com comportamento exclusivo próprio.
- Não presumir FM em SMS exportação ou Tec Toy sem evidência.
- Não iniciar síntese de produção antes de fechar os itens PENDENTES.
- Nenhuma mudança de hardware é considerada concluída sem testes diferenciais, temporais, de snapshot e regressão.


---

## 14. Reconciliação de evidências M1 — PG, PM, EG e feedback (2026-11-06)

Esta revisão transforma parte dos itens pendentes de M0 em modelos candidatos com evidência publicada. Ela **não** declara o core implementado e não substitui vetores do projeto nem validação independente no YM2413 original.

### 14.1 Phase Generator — incremento base

As medições de andete sustentam um acumulador de fase de 19 bits em formato 10.9. Os 10 bits superiores selecionam a posição da tabela seno; os 9 bits inferiores preservam a fração. A ausência de interpolação é uma inferência consistente com as medições, não uma especificação elétrica independente.

Tabela de multiplicadores usada no cálculo:

| MUL | ML |
|---:|---:|
| 0 | 1 |
| 1 | 2 |
| 2 | 4 |
| 3 | 6 |
| 4 | 8 |
| 5 | 10 |
| 6 | 12 |
| 7 | 14 |
| 8 | 16 |
| 9 | 18 |
| A | 20 |
| B | 20 |
| C | 24 |
| D | 24 |
| E | 30 |
| F | 30 |

Incremento base medido, sem PM:

`phase_step = ((FNUM * ML[MUL]) << BLOCK) >> 1`

O acumulador deve manter os 19 bits efetivos; o cálculo intermediário do incremento pode exceder 19 bits, mas a soma observável é reduzida ao domínio de fase de 19 bits. FNUM=0 não avança a fase. MUL=0, BLOCK=0, FNUM=1 também resulta em incremento zero por causa do deslocamento à direita.

### 14.2 Phase Modulation / Vibrato

As notas de medição apresentam uma tabela PM de 8 × 8 indexada por `FNUM[8:6]` e pela posição do LFO. O índice de posição avança a cada 1024 amostras internas; o ciclo de oito posições dura 8192 amostras. Com 3.579.545 / 72 ≈ 49.716,04 amostras/s, o período corresponde a aproximadamente 6,069 Hz.

`pm = pmTable[FNUM >> 6][(global_sample_counter >> 10) & 7]`

`phase_step = (((2 * FNUM + pm) * ML[MUL]) << BLOCK) >> 2`

A expressão reduz-se ao incremento base quando PM=0. A tabela medida é:

| FNUM[8:6] | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 1 | 0 | 0 | 1 | 0 | 0 | 0 | -1 | 0 |
| 2 | 0 | 1 | 2 | 1 | 0 | -1 | -2 | -1 |
| 3 | 0 | 1 | 3 | 1 | 0 | -1 | -3 | -1 |
| 4 | 0 | 2 | 4 | 2 | 0 | -2 | -4 | -2 |
| 5 | 0 | 2 | 5 | 2 | 0 | -2 | -5 | -2 |
| 6 | 0 | 3 | 6 | 3 | 0 | -3 | -6 | -3 |
| 7 | 0 | 3 | 7 | 3 | 0 | -3 | -7 | -3 |

**PENDENTE:** fase inicial e alinhamento do contador global com reset, slots e atualização dos registradores; ordem exata entre incremento de fase e leitura da fase no mesmo slot; confirmação do comportamento de PM em todas as combinações de frequência/MUL.

### 14.3 Envelope Generator — níveis e escala

As medições publicadas sustentam um nível de envelope de 7 bits (0–127), com passo de 0,375 dB. O nível 127 corresponde a -47,625 dB; 128 níveis cobrem aproximadamente 48 dB. O manual convertido para HTML informa 0,325 dB, valor incompatível com 128 níveis em 48 dB e com as medições. A hipótese de erro de transcrição no manual é forte, mas a divergência deve permanecer registrada até que os vetores de referência do projeto a encerrem.

O sustain level foi medido como:

`EG_sustain_level = 8 * SL`

Cada incremento de SL representa 3 dB. O KSR usa:

`BF = (BLOCK << 1) | FNUM[8]`

`RKS = KSR ? BF : (BF >> 2)`

`RATE = min(63, 4 * R + RKS)`

O texto do manual confirma a relação RATE=4R+RKS e que RATE=0 não altera o envelope. A tabela de medições de andete detalha o escalonamento do RKS e as curvas de ataque/decay. Há listagens C++ nas notas de 2017 que exibem `max(63, 4*R + RKS)` e `max(127, env + step)`; essas expressões contradizem os limites superiores documentados/medidos e o próprio comportamento descrito. Tratá-las como provável erro tipográfico da listagem: a hipótese coerente é saturar superiormente com `min(63, ...)` e `min(127, ...)`, sujeita a vetores antes de codificar.

### 14.4 EG — temporização candidata e transições observadas

O modelo de andete descreve um contador global compartilhado, incrementado uma vez por amostra de saída (uma travessia completa dos 18 slots, equivalente a 72 clocks de entrada), que distribui as atualizações de envelope. Para as taxas de decay usuais:

- `eg_shift = 13 - (RATE / 4)`
- `eg_select = RATE & 3`
- a atualização só ocorre quando os bits inferiores exigidos do contador global são zero;
- uma tabela 4 × 8 seleciona se o nível avança 0 ou 1 passo.

Para ATTACK, o mesmo escalonamento é usado com exceção de que os dois bits inferiores são ignorados; quando a entrada da tabela permite atualização, o nível é transformado por uma das funções discretas:
- `f4(x) = x - (x >> 4) - 1`
- `f3(x) = x - (x >> 3) - 1`
- `f2(x) = x - (x >> 2) - 1`
- `f1(x) = x - (x >> 1) - 1`

As taxas rápidas 12–14 usam combinações específicas dessas funções. RATE 15 é instantâneo quando selecionado na transição DAMP→ATTACK; se alterado para 15 durante uma fase ATTACK já iniciada, as medições indicam que o avanço pode ficar parado. Não simplificar as taxas rápidas como uma curva exponencial contínua.

As medições também registram:
- DAMP com taxa-base 12, aproximadamente 10,22 ms para a queda de 0 a -48 dB; a transição observada para ATTACK pode ocorrer perto de -45 dB.
- O início do ATTACK pode ser nível 124 ou 127, dependendo se a sequência anterior concluiu RELEASE ou permaneceu em DAMP.
- EG_TYPE seleciona comportamento percussivo/sustentado: ao atingir SL, o tom sustentado mantém o nível até KEY-OFF; o percussivo passa para RELEASE.
- O modelo de engenharia reversa atribui estados DAMP, ATTACK, DECAY e SUSTAIN, com RELEASE condicionado a KEY-OFF e aos parâmetros EG_TYPE/SUS.

**PENDENTE:** alinhar exatamente o contador global e as atualizações EG aos 18 slots e aos clocks de entrada; resolver o ponto de amostragem de RATE e dos parâmetros alterados durante uma fase; fixar a semântica do operador modulator em KEY-OFF e os estados de release; criar vetores para todas as taxas 0–63, incluindo transições durante a execução.

### 14.5 Feedback do modulator

O modelo que reproduz as medições usa duas saídas históricas do modulator (`p0`, `p1`), cada uma representada em 12 bits com faixa assinada aproximada -2047…+2047:

`feedback_phase = FB == 0 ? 0 : (p0 + p1) >> (8 - FB)`

O resultado do modulator é reduzido antes de atualizar o histórico; o carrier recebe modulação derivada da saída recente do modulator. As medições mostram FB=0 sem feedback e aumento de instabilidade/caos para feedback alto em determinadas configurações.

**PENDENTE:** inicialização física de p0/p1, ordem de atualização por slot, larguras e truncamentos intermediários, tratamento de overflow e qualquer offset de fase no primeiro ciclo. O modelo de andete reproduz os dados medidos após sincronização, mas reporta divergências iniciais dependentes das condições iniciais.

### 14.6 Log-Sine / EXP e limites de reutilização

As notas de engenharia reversa descrevem duas tabelas de 256 entradas:
- `logsin[i] = round(-log2(sin((i + 0.5) * PI / 512)) * 256)`
- `exp[i] = round((2^(i / 256) - 1) * 1024)`

O caminho de lookup usa espelhamento de quadrante e sinal; a conversão EXP usa mantissa, expoente e complemento de um para o sinal negativo. Essas fórmulas constituem um modelo de referência que deve ser transformado em tabelas inteiras fixas e testado por vetor. Não executar trigonometria/logaritmos no caminho de síntese.

### 14.7 Pipeline de slots — refinamento arquitetural

A entrada é dividida por quatro; cada clock interno processa um dos 18 slots. O pipeline não deve ser interpretado como 18 operadores físicos paralelos: PG, EG e operador/DAC são estágios sequenciais com offsets de slot, trabalhando em contextos de slots distintos no mesmo clock interno. Um período completo contém 18 clocks internos, ou 72 clocks de entrada.

A implementação Rust de referência `emu198x_yamaha_ym2413` documenta seguir o pipeline e os offsets de Nuked-OPLL 1.0.2 (commit `1269cf5a`) e comparar os pinos com Nuked clock a clock. É uma referência técnica útil para ordenar os estágios, não prova independente de silício. O crate e Nuked-OPLL são GPL-2.0-or-later: não copiar código ou tabelas para o projeto sem decisão explícita de compatibilidade de licença. A arquitetura do Sega Emulator deve manter implementação própria e vetores de validação independentes.

### 14.8 Critérios de aceitação M1 adicionais

| ID | Critério | Estado |
|---|---|---|
| YM-021 | Vetores do incremento base para todos os MUL, FNUM e BLOCK, incluindo casos de incremento zero e wrap de 19 bits | NOT RUN |
| YM-022 | Sequência PM 8 × 8 e período do LFO, com alinhamento de reset documentado | NOT RUN |
| YM-023 | Níveis EG 0–127 e escala 0,375 dB reconciliados com vetores | NOT RUN |
| YM-024 | KSR/RKS e RATE 0–63, inclusive padrões de atualização global | NOT RUN |
| YM-025 | ATTACK f1–f4, taxas 12–15 e condições de início 124/127 | NOT RUN |
| YM-026 | DAMP, KEY-ON/OFF, EG_TYPE, SUS, DECAY e RELEASE por operador | NOT RUN |
| YM-027 | Feedback p0/p1 com reset, FB 0–7, TL e alinhamento por slot | NOT RUN |
| YM-028 | Pipeline PG/EG/OP por slot e saídas MO/RO em cada clock interno | NOT RUN |

### 14.9 Fontes desta revisão

- Yamaha YM2413 Application Manual, seções PG, EG e OP/DAC: https://www.smspower.org/maxim/Documents/YM2413ApplicationManual
- Andete, phase counter / fórmula base: https://www.smspower.org/Development/YM2413ReverseEngineeringNotes2015-03-16
- Andete, PM/vibrato: https://www.smspower.org/Development/YM2413ReverseEngineeringNotes2015-12-01
- Andete, decay/release: https://www.smspower.org/Development/YM2413ReverseEngineeringNotes2015-03-20
- Andete, SL/KSR: https://www.smspower.org/Development/YM2413ReverseEngineeringNotes2015-12-24
- Andete, feedback: https://www.smspower.org/Development/YM2413ReverseEngineeringNotes2015-12-12
- Andete, envelope, ATTACK e transições: https://www.smspower.org/Development/YM2413ReverseEngineeringNotes2015-12-31 e https://www.smspower.org/Development/YM2413ReverseEngineeringNotes2017-01-26
- Pipeline Rust comparativo (Nuked-OPLL 1.0.2): https://docs.rs/emu198x-yamaha-ym2413/latest/emu198x_yamaha_ym2413/
- Implementação de referência emu2413 (MIT; usar somente para comparação e conferir branch/commit): https://github.com/digital-sound-antiques/emu2413

**Status M1:** PG base e PM têm fórmulas candidatas com suporte de medições; a escala EG de 0,375 dB e a temporização por tabela estão caracterizadas; feedback possui modelo que reproduz medições após sincronização. Pipeline exato, estados iniciais, todos os offsets, integração de slots e vetores completos continuam pendentes. Nenhum core YM2413 foi implementado por esta atualização documental.


### 14.10 Auditoria de integração SMS/áudio contra o `main` (SHA `c65594cf840090bad6036810a0df98f22b9d1484`)

A leitura dos fontes ativos confirmou os seguintes limites concretos:

1. **Domínio de clock compartilhável.** O coordenador SMS registra exatamente três domínios: Z80 (ID 0), PSG (ID 1) e VDP (ID 2). O restore exige exatamente três domínios. O PSG recebe 3.579.545 Hz; o YM2413 também recebe 3.579.545 Hz. Como ambos compartilham a mesma fonte/razão de clock no perfil SMS NTSC considerado, o adapter FM deve consumir o mesmo `ClockAdvance` do domínio 1, sem registrar um quarto domínio nem avançar o Scheduler duas vezes. O estado interno de fase/divisor de cada chip continua independente.

2. **Barramento FM ausente.** `SmsIo::write_port` trata somente o registrador de controle $3F e declara explicitamente que FM ainda não está conectado. `SmsTemporalBus::write_port` intercepta VDP, PSG e memória; $F0/$F1/$F2 atualmente caem no caminho genérico de I/O. `read_port` também não fornece o contador/control readback do FM. O decode de portas, a ordem do latch/endereço, a escrita de dados e o readback de $F2 precisam ser implementados no barramento SMS, com timestamp de conclusão do ciclo Z80.

3. **Save State incompleto para FM.** `SmsTemporalBusState` serializa atualmente `psg`, `vdp`, `io` e o estado do coordenador, mas não contém latch FM, registradores, slots, operadores, LFOs, pipeline, saídas MO/RO ou estado de mix/resampling FM. A adição do chip exige extensão/versionamento do estado do barramento e validação cruzada com o perfil físico do SMS. Como não se adiciona domínio ao Scheduler, a versão do estado do coordenador não deve ser incrementada apenas por essa razão; a versão do estado do barramento deve ser revista.

4. **Mixer multi-source ainda não está exposto pela sessão.** `AudioMixer::mix_mono_into` aceita várias `MixSource` com o mesmo número de frames, mas `NativeAudioSession::process_samples` constrói sempre um array com uma única fonte. Chamá-lo uma vez para PSG e outra para FM não mistura os chips: concatena dois blocos temporalmente no resampler e envia blocos separados. É necessário expor uma operação de sessão que receba todas as fontes alinhadas e invoque o mixer uma única vez por lote; manter `process_samples` como wrapper de fonte única é compatível.

5. **Taxas nativas diferentes.** O SN76489 produz amostras na taxa solicitada ao core e aplica seu próprio filtro/downsampling a partir do clock interno dividido por 16. O `PcmResampler` de `sega-audio-runtime` assume entrada fixa de 44.100 Hz e converte para a taxa de saída do host. O YM2413 produz uma amostra por 72 clocks de entrada (aprox. 49.716,04 amostras/s). Portanto, a saída YM2413 não pode ser entregue diretamente ao atual `NativeAudioSession` como se fosse 44.100 Hz. Antes da mistura, deve existir uma conversão determinística para uma taxa comum (ou uma generalização explícita do mixer/resampler para fontes com taxas e timestamps próprios). A fase dos conversores precisa ser derivada do tempo do Scheduler, não do tamanho dos blocos da GUI/host.

6. **MO/RO e circuito analógico.** O manual Yamaha identifica MO e RO como saídas source-follower e exige circuito integrado/amplificador posterior; portanto, os pinos não devem ser tratados como amostras digitais já normalizadas nem somados sem modelo de carga. O manual mostra um circuito de interface de áudio SFG-05/MSX, que não deve ser transplantado automaticamente para o SMS. Existe esquema do SMS japonês traçado a partir das trilhas da placa, mas o próprio autor alerta que ele pode não ser 100% preciso. O core deve preservar MO e RO separadamente; a conversão para a fonte mono deve seguir a rede de soma/ganho da revisão SMS escolhida, após confirmação por esquema confiável ou medição. O controle de mute/roteamento de $F2 não deve parar os geradores internos sem evidência física.

**Decisão arquitetural M1:** o YM2413 recebe os mesmos avanços de clock do domínio PSG (ID 1), mas mantém estado e pipeline próprios. O adapter não cria um relógio paralelo nem um novo domínio no Scheduler. O barramento SMS passa a possuir o adapter e o estado de portas FM. A sessão de áudio passa a misturar fontes alinhadas antes do resampling PCM final.

**Bloqueios para implementação:**
- confirmar o circuito MO/RO e as revisões/modelos SMS japoneses alvo;
- definir a taxa comum de mistura e o ponto único de conversão, preservando latência/fase entre PSG e FM;
- especificar o alinhamento temporal entre o ciclo de escrita $F0/$F1/$F2, a amostra YM2413 e o avanço compartilhado do domínio 1;
- projetar os vetores de $F2 (mute/enable e contador C-Sync) e a compatibilidade de perfil antes de habilitar FM por região/modelo.

Fontes de código auditadas:
- `native/sms_core/src/temporal.rs`: IDs 0/1/2 e restore com três domínios.
- `native/sms_core/src/io.rs`: I/O atual e ausência explícita de FM.
- `native/sms_core/src/temporal_bus.rs`: decode de portas, estado do barramento e avanço do PSG.
- `native/audio_runtime/src/dsp.rs`: mixer multi-source.
- `native/audio_runtime/src/session.rs`: sessão que atualmente passa uma única fonte ao mixer.
- `native/audio_runtime/src/lib.rs`: entrada fixa de 44.100 Hz do resampler PCM.
- `native/sn76489/src/psg.rs`: geração/downsampling interno do SN76489 para a taxa configurada.

**Status:** auditoria de integração PASS (leitura estática dos fontes no SHA acima); implementação FM, validação elétrica da mistura MO/RO e testes de integração permanecem NOT RUN.
