# Auditoria A1 — fechamento do PSG SN76489 (SMS)

**Status:** auditoria estática inicial; não certifica fidelidade física.
**PR de arquitetura:** #646 — `docs/sms-complete-audio-architecture-20261107`
**Baseline auditada:** `main` — `f47f71830344b85f72329376505d400bfbfaae69` (2026-11-07)
**Fonte principal de código:** `native/sn76489/src/psg.rs`, blob `b0eaec6b1917001be729f5524115c1b3e43b19aa`.
**Integração:** `native/audio_runtime/src/psg.rs`, blob `9b8f71846f84195068d3753875ab43851369e89d`.

## 1. Resultado executivo

O PSG não é um esqueleto vazio: há um núcleo funcional compartilhado com 979 linhas, dois perfis (`Sms` e `Sg1000`), geração digital, buffer PCM, FIR e 23 testes unitários. A integração `ScheduledPsg` consome `ClockAdvance` do Scheduler e aplica escritas no boundary de dispatch.

O trabalho A1 deve ser uma **auditoria de fechamento**, não uma reescrita. O foco é provar a semântica SMS que ainda está pendente, formalizar reset/estado, separar a resposta digital do chip dos estágios de saída e produzir vetores diferenciais independentes.

### Classificação inicial

| Área | Estado | Fundamentação |
|---|---|---|
| Protocolo latch/data dos 8 registradores | IMPLEMENTADO — cobertura automatizada existente | Testes de todos os latches, nibbles e data writes |
| Três canais de tom | IMPLEMENTADO — cobertura parcial | Períodos exaustivos e frequência medida por teste |
| Valores de período 0/1 | IMPLEMENTADO — precisa fixar oráculo | Código mantém saída constante positiva |
| Atenuação 0..15 | IMPLEMENTADO — cobertura funcional existente | Tabela de 2 dB aproximados; 15 silencia |
| Ruído periódico/branco SMS | IMPLEMENTADO — cobertura interna existente | Seed 0x8000, shift de 16 bits, taps SMS e vetores fixos |
| Continuidade do divisor ao escrever noise register | IMPLEMENTADO — teste existente | LFSR reinicia, contador/fase permanecem |
| Noise rate 0/1/2 | IMPLEMENTADO — cobertura parcial | Divisores fixos 0x10/0x20/0x40 |
| Noise rate = Tone 2 | PARCIAL / BLOQUEADO PARA FECHAMENTO | Frequência usa contador dedicado; fase relativa ao flip-flop Tone 2 ainda não provada |
| Perfil SG-1000 | IMPLEMENTADO — regressão obrigatória | Perfil de 15 bits precisa permanecer inalterado |
| Clock regional PAL | NOT RUN / PENDENTE | Há suporte parametrizado por clock, mas faltam vetores de integração PAL |
| Portas SMS 0x40..0x7F | FORA DO CORE / AUDITORIA DE INTEGRAÇÃO | Deve ser validado no decode do barramento SMS |
| Estado serializável | PENDENTE | Core deriva Clone/PartialEq; não há contrato DTO versionado serde próprio |
| Diferencial contra oráculo independente | PENDENTE | Testes atuais são majoritariamente internos ao mesmo algoritmo |
| Saída analógica física SMS | NOT RUN | FIR é conversão digital; não representa circuito/placa medidos |

Os termos IMPLEMENTADO e cobertura automatizada descrevem apenas o código observado. Não equivalem a PASS de hardware.

## 2. Inventário do código existente

### 2.1 Núcleo `sega-sn76489`

Arquivo: `native/sn76489/src/psg.rs`.

- `CLOCK_DIVIDER = 16`: o core recebe clocks do chip e mantém `clock_divider_phase`; não deve ser confundido com 16 T-states de CPU.
- `PsgProfile::Sms`: seed `0x8000`, máscara `0xFFFF`, feedback inserido no bit 15 e função de feedback branco baseada nos taps SMS.
- `PsgProfile::Sg1000`: seed `0x4000`, máscara `0x7FFF`; deve ser preservado como perfil distinto.
- Registradores: três períodos de 10 bits, quatro atenuações, controle de ruído e latch persistente.
- Escritas de tom atualizam os bits recebidos imediatamente, preservando os demais bits.
- Escritas de volume e ruído por data byte são aceitas no registrador latched.
- O divisor de ruído mantém contador e fase independentes do LFSR; uma escrita no noise register reinicializa o LFSR sem reiniciar o divisor.
- A geração de amostras usa fase inteira com intermediário `u128`, buffer limitado a um segundo e contador explícito de amostras descartadas.
- O FIR Kaiser é calculado na construção do dispositivo e aplicado ao sinal misturado; é estágio de filtragem/conversão, não modelo medido do circuito analógico do SMS.
- `render()` e `drain_samples()` são duas fronteiras de consumo que precisam continuar com contratos testados.

### 2.2 Adaptador `ScheduledPsg`

Arquivo: `native/audio_runtime/src/psg.rs`.

- Associa o core a um `ClockDomainId`.
- Consome `ClockAdvance.elapsed_cycles`; rejeita domínio incorreto.
- Aplica a escrita depois do avanço temporal entregue pelo dispatch.
- Encaminha amostras para `NativeAudioSession`.
- Não cria relógio privado nem calcula ciclos por taxa de amostragem do host.

**Decisão:** manter esse contrato. Não adicionar `catch_up(target_cycle)` paralelo ao Scheduler.

### 2.3 Cobertura unitária identificada

Foram encontrados 23 testes em `native/sn76489/src/psg.rs`:

1. resposta espectral do FIR;
2. reset e avanço repetível;
3. round-trip de todos os valores de período de tom;
4. decodificação de latch/nibbles;
5. seleção das taxas de ruído;
6. reset do LFSR sem reinício do divisor;
7. atenuação e mute;
8. vetor dourado do LFSR branco;
9. ciclo de estados do LFSR de ruído branco;
10. saída do bit deslocado;
11. reciclagem do LFSR periódico;
12. limites exaustivos do contador de tom;
13. frequência de tom renderizado;
14. invariância de avanço em blocos;
15. contrato público render/drain;
16. limites aritméticos de u64;
17. validação de parâmetros;
18. sequência SMS de LFSR branco;
19. sequência SMS de LFSR periódico;
20. distinção/repetibilidade dos perfis;
21. reset por perfil;
22. protocolo de registradores SMS;
23. geração SMS determinística e invariância em blocos.

Esses testes são uma boa base, mas vários validam propriedades calculadas pelo próprio core. A1 deve acrescentar oráculos independentes e cobrir as fronteiras ainda sem prova.

## 3. Pontos que exigem decisão técnica antes de alterar o core

### PSG-A1-D01 — ruído com rate Tone 2

O código atual deriva `noise_period` de `tone_periods[2].max(1)` e mantém um contador de ruído próprio. A documentação SMS Power descreve a seleção de Tone 2 como clock do gerador de ruído e que o LFSR desloca na transição apropriada do divisor.

A frequência média pode coincidir com uma implementação que usa diretamente o flip-flop Tone 2, mas isso não prova a mesma fase após:
- escrita no período de Tone 2;
- escrita no registrador de ruído;
- reset;
- avanços interrompidos em qualquer fase do divisor;
- escrita exatamente no boundary de expiração.

**Ação:** criar testes de trace de bordas para Tone 2 e noise, comparando as duas relações de fase. Não substituir o contador dedicado por uma conexão direta até haver evidência específica do Sega PSG de 16 bits. Classificação atual: PENDENTE.

### PSG-A1-D02 — modelo de saída e amplitude

A tabela `ATTENUATION` implementa níveis aproximados em passos de 2 dB e mute no nível 15. O mixer soma quatro canais e divide por quatro.

**Ação:** congelar os valores esperados em vetores independentes, verificar polaridade/nível inicial e saturação, e separar:
1. amplitude digital normalizada do chip;
2. mistura de canais;
3. filtragem FIR para conversão;
4. resposta analógica externa do console.

Não calibrar ganho ou LPF analógico com base apenas em preferência auditiva ou em amostra produzida pelo mesmo algoritmo.

### PSG-A1-D03 — reset e estado inicial

O reset atual limpa contadores, outputs, LFSR, fases, buffers e histórico FIR. A documentação SMS Power descreve o estado inicial integrado como diferente de alguns chips discretos.

**Ação:** definir separadamente power-on, reset do chip e reset da máquina SMS; validar quais memórias/filas são limpas e qual estado elétrico é observável. Não generalizar o estado inicial de um chip discreto para o ASIC integrado.

### PSG-A1-D04 — estado persistível

O core deriva `Clone` e `PartialEq`, mas não expõe DTO serializável versionado.

**Ação:** especificar `Sn76489StateV1` contendo perfil, clock, taxa interna de amostragem, registradores, latch, contadores, outputs, LFSR, fases, configuração e histórico FIR, buffers pendentes, posição do filtro e contador de drops conforme a fronteira de save state adotada. Validar invariantes antes do restore. Não serializar estado do dispositivo WASAPI/XAudio2 como estado do chip.

### PSG-A1-D05 — buffer e backpressure

O buffer é limitado e descarta a amostra mais antiga quando cheio, incrementando `dropped_samples`.

**Ação:** documentar a política, testar exatamente o limite/capacidade, provar que overflow do consumidor não altera o estado digital do PSG e expor telemetria ao host sem bloquear o Scheduler. Avaliar se a política de descarte é adequada ao runtime determinístico ou se o limite deve ocorrer exclusivamente na fronteira host.

### PSG-A1-D06 — clock regional e amostragem

O core recebe `clock_hz` e `sample_rate`; a fase de saída usa aritmética inteira. A taxa do host não pode retroagir sobre os clocks do chip.

**Ação:** acrescentar vetores NTSC e PAL com razões racionais do Scheduler, validar quantidade/fase de amostras para 22.05/44.1/48/96 kHz e testar execução contínua versus blocos arbitrários. Não codificar PAL-M como PAL ou NTSC sem perfil de placa comprovado.

## 4. Plano incremental A1

| Etapa | Entrega | Critério de aceite |
|---|---|---|
| A1.1 | Matriz PSG preenchida com código, evidência e status por requisito | Cada requisito tem fonte, teste e classificação |
| A1.2 | Vetores de referência de latch, tom, volume, ruído e reset | Esperados independentes do core; regressão SMS + SG-1000 |
| A1.3 | Fechamento do rate Tone 2 | Trace de fase validado ou limitação explicitamente documentada |
| A1.4 | DTO versionado e round-trip de estado | Continuidade futura bit-a-bit após restore em fases arbitrárias |
| A1.5 | Buffer, drops e fronteira de saída | Overflow/underrun mensurados; estado digital não afetado |
| A1.6 | Validação regional e de chunking | NTSC/PAL, taxas de saída múltiplas, mesmo estado final e amostras |
| A1.7 | Auditoria e regressão | fmt, clippy, testes direcionados, workspace completo e SG-1000 |

Cada etapa deve ser uma alteração pequena e revisável. Se uma decisão depender de hardware físico indisponível, a implementação fica BLOCKED nesse ponto; testes diferenciais podem avançar, mas não são rotulados como medição de hardware.

## 5. Contrato de aceite para declarar PSG fechado

O PSG só poderá sair de A1 quando todos os itens abaixo tiverem evidência:

- protocolo de registradores e espelhos de portas SMS;
- frequências e transições de tom, inclusive períodos 0 e 1;
- atenuação, polaridade, mistura e mute;
- ruído periódico e branco SMS, seed, taps, saída e cadência;
- relação de fase de Noise Rate = Tone 2;
- reset/power-on por perfil;
- clock NTSC/PAL e fase de amostragem;
- invariância de chunking incluindo escritas nos boundaries;
- snapshot/restore completo e versionado;
- buffers limitados e telemetria de perda;
- regressão SG-1000;
- vetores diferenciais independentes identificados por versão/commit;
- gates Rust verdes no HEAD exato.

**Validação física:** teste em Master System real, revisão identificada, permanece um item independente. Se não estiver disponível, registrar BLOCKED/NOT RUN; não converter validação de software em PASS de hardware.

## 6. Preparação do YM2413 em paralelo documental

A especificação existente em `docs/architecture/ym2413-fm-model.md` já cobre os elementos principais: 18 slots sequenciais, 4 clocks por slot, 72 clocks por período, 9 canais melódicos, 15 presets + patch de usuário, PG/EG, feedback, AM/PM, Rhythm Mode, registradores e temporização de barramento.

Não duplicar essa especificação nem iniciar síntese FM antes de reconciliar os itens PENDENTES nela. A sequência após A1 é:
1. A2 — contrato de registradores, latch, wait timing e estado versionado;
2. A3 — operador reutilizado por slot, PG/EG e caminho de síntese melódica;
3. A4 — Rhythm Mode e gerador de ruído de percussão;
4. A5 — integração com Scheduler, barramento SMS, mixer e resampler.

O OPLL deve permanecer um núcleo compartilhado, opcional por perfil físico. PSG e FM podem coexistir no Master System japonês; a política do Mark III + FM Unit e os controles $F0/$F1/$F2 permanecem no adaptador SMS, não no core genérico.

## 7. Fontes e limites de evidência

- SMS Power — SN76489: https://www.smspower.org/Development/SN76489
- SMS Power — YM2413: https://www.smspower.org/Development/YM2413
- Yamaha YM2413 Application Manual (transcrição): https://www.smspower.org/maxim/Documents/YM2413ApplicationManual
- MAME histórico SN76496: https://github.com/mamedev/historic-mame/blob/master/src/emu/sound/sn76496.c

A documentação SMS Power é a referência primária comunitária utilizada para protocolo e comportamento observado da família Sega. O código MAME é comparação independente, não prova física. O estado de ruído e o alinhamento Tone 2 devem ser conferidos para o perfil SMS de 16 bits, não inferidos de um TI SN76489 de 15 bits ou de outro console.

## 8. Status da auditoria

| Suíte / atividade | Estado |
|---|---|
| Inspeção estática do core no SHA indicado | PASS |
| Inventário de 23 testes existentes | PASS |
| Revisão documental SMS Power | PASS |
| Comparação diferencial executada nesta etapa | NOT RUN |
| Novos testes implementados | NOT RUN |
| Cargo fmt / clippy / test nesta etapa | NOT RUN |
| Teste físico em Master System | BLOCKED — mantenedor indisponível para teste no momento |
| Fechamento PSG A1 | IN PROGRESS |

Nenhum resultado de CI anterior é transferido para commits novos. A implementação permanece inalterada nesta etapa documental.


## 9. A1.1 — atualização da auditoria e confronto de referências (2026-11-12)

**Baseline de código:** `main` em `3703b9d5b12e654d4f2a50e6ebb9d12915b4c1f1`.
**Core auditado:** `native/sn76489/src/psg.rs`, blob `55e1f856ae172026d53de979f55ed81beee37ad5`.
**Branch de trabalho:** `feat/psg-a1-closure-20261112`.
**Escopo desta atualização:** somente auditoria documental e comparação de fontes. Nenhum comportamento do runtime foi alterado.

### 9.1 Evidência confirmada na documentação SMS Power

A página técnica SMS Power distingue o PSG discreto SN76489AN dos PSGs Sega integrados ao VDP. Para o comportamento documentado da família:

- O registrador de tom define o valor de recarga do contador; a onda quadrada tem período de duas vezes esse valor.
- A página afirma explicitamente que valores de tom 0 e 1 produzem saída constante +1.
- O contador do ruído usa divisores 0x10, 0x20, 0x40 ou Tone 2.
- O LFSR avança quando sua entrada muda de 0 para 1, uma vez a cada duas expirações do contador relacionado.
- Para SMS, o LFSR tem 16 bits; o ruído branco usa taps 0 e 3 (máscara 0x0009), com feedback no bit 15.

Fonte: https://www.smspower.org/Development/SN76489 — seções “Tone channels”, “Noise channel” e “The Linear Feedback Shift Register”.

### 9.2 Divergência que impede fechar os períodos 0/1

A implementação atual do core executa:

```rust
if period <= 1 {
    self.tone_outputs[channel] = 1;
    self.tone_counters[channel] = period;
}
```

Esse comportamento coincide com a descrição textual do SMS Power para valores 0 e 1.

Entretanto, a implementação atual do MAME para `segapsg_device` documenta que PSGs Sega tratam frequência 0 como zero, e sua atualização de canal alterna a saída quando o contador chega a zero. Isso produz comportamento diferente da saída constante descrita pelo SMS Power. O código MAME identifica essa semântica como verificada em hardware, mas é uma implementação de referência, não uma medição reproduzida neste projeto.

Fonte: https://github.com/mamedev/mame/blob/master/src/devices/sound/sn76496.cpp — comentários de variante Sega, construtor `segapsg_device`, atualização de registradores e rotina `sound_stream_update`.

**Decisão A1.1:** não alterar a implementação nem transformar o comportamento atual em PASS. A semântica SMS de período 0/1 permanece **CONFLICT / ORACLE REQUIRED** até que a diferença de variante/revisão ou a evidência de hardware seja reconciliada. O perfil SG-1000 não deve herdar automaticamente a regra Sega.

### 9.3 Noise Rate = Tone 2 — o que foi e não foi provado

O core atual mantém `noise_counter` e `noise_counter_phase` próprios. Quando Noise Rate = Tone 2, `refresh_noise_period()` copia `tone_periods[2].max(1)`; a rotina de tick alterna a fase do contador de ruído e desloca o LFSR em uma das duas expirações. Assim, a taxa média de deslocamento corresponde a duas vezes o período de Tone 2.

A implementação MAME também representa esse modo como período de ruído igual a duas vezes o período de Tone 2. Isso dá suporte à relação de frequência média, mas não demonstra que a fase inicial e as transições após escritas/reset sejam idênticas às do VDP Sega. A descrição SMS Power explica a divisão por duas expirações, mas não especifica todos os alinhamentos de fase após cada sequência de escrita.

**Decisão A1.1:** preservar o contador independente por enquanto. A1.3 deve produzir traces comparativos de expiração, transição do flip-flop Tone 2 e shift do LFSR para reset, escrita de Noise, escrita de Tone 2 e avanço interrompido em boundaries. Não conectar diretamente o LFSR ao flip-flop Tone 2 sem evidência adicional.

### 9.4 Saída do LFSR — convenção temporal a fixar

A descrição SMS Power diz que o bit deslocado para fora do registrador é encaminhado ao mixer, enquanto o pseudocódigo apresentado na mesma página atualiza o registrador e depois lê o novo bit 0. O core atual captura o bit 0 anterior ao shift em `noise_output`. Implementações MAME também atualizam a saída após deslocar o registrador.

Essa diferença pode decorrer da convenção de direção/representação do registrador, mas não deve ser resolvida por equivalência presumida. A1.2 deve congelar uma sequência de estados **e** bits de saída por transição, com a convenção de indexação explicitada, e confrontá-la com uma referência independente do perfil Sega de 16 bits.

**Decisão A1.1:** a sequência de estados SMS existente continua coberta por teste interno; a sequência temporal de saída do bit permanece **ORACLE REQUIRED**.

### 9.5 Resultado desta etapa

| Item | Resultado A1.1 |
|---|---|
| Baseline e blob do core identificados | PASS |
| Separação de variante discreta e Sega integrada | PASS |
| Protocolo, divisores e taps documentados | PASS — documentação de referência |
| Períodos 0/1 | CONFLICT / ORACLE REQUIRED |
| Fase Noise Rate = Tone 2 | PARTIAL / TRACE REQUIRED |
| Convenção temporal do bit de saída LFSR | CONFLICT / ORACLE REQUIRED |
| Novos testes de código | NOT RUN |
| Cargo fmt / Clippy / testes | NOT RUN |
| Validação física Master System | BLOCKED / NOT RUN |

A1.1 está concluída como **atualização do inventário de evidências**, não como fechamento do PSG. A próxima alteração de código deve ser A1.2: vetores diferenciais independentes para registradores, tom, ruído, saída do LFSR, volume e reset. Nenhuma correção do core deve preceder a definição explícita do oráculo correspondente.


## 10. A1.2 — vetores diferenciais SMS de ruído (2026-11-12)

**Baseline de teste:** branch `feat/psg-a1-closure-20261112`.
**Referência externa:** Gearsystem, commit `1be9a12b82cae84ddd6f9b7006d546abe2d58bd4`, arquivo `src/audio/Sms_Apu.cpp`, rotinas `Sms_Noise::reset`, `Sms_Noise::run` e `Sms_Apu::write_data`.
**Escopo:** testes do traço digital SMS; nenhuma alteração no core de produção.

### 10.1 Modelo de referência usado

O código independente do Gearsystem representa o perfil SMS com:
- seed `0x8000`;
- registrador de deslocamento de 16 bits;
- feedback branco derivado dos taps 0 e 3, inserido no bit 15;
- saída do mixer baseada no bit 0 do estado anterior ao shift;
- feedback periódico igual ao bit 0 deslocado para o bit 15, sem XOR.

A referência é uma implementação independente de emulação, não uma medição elétrica do VDP. Ela é usada para comparação diferencial do traço, não para declarar equivalência física absoluta.

Fonte fixada: https://github.com/drhelius/Gearsystem/blob/1be9a12b82cae84ddd6f9b7006d546abe2d58bd4/src/audio/Sms_Apu.cpp

### 10.2 Testes adicionados

Em `native/sn76489/src/psg.rs`:
- `sms_white_noise_trace_matches_independent_gear_system_reference`: 32 tuplas estado anterior / bit de saída / próximo estado.
- `sms_periodic_noise_trace_matches_independent_gear_system_reference`: ciclo periódico completo de 16 deslocamentos, incluindo retorno ao seed.

Os testes exercitam explicitamente o perfil `PsgProfile::Sms`; não usam o construtor padrão SG-1000. A comparação inclui o bit de saída no mesmo boundary do deslocamento, evitando validar apenas o período ou o estado final.

### 10.3 Resultado A1.2 parcial

| Item | Estado |
|---|---|
| Vetor SMS white noise (16-bit, taps 0/3) | IMPLEMENTED — aguardando CI do HEAD |
| Vetor SMS periodic noise | IMPLEMENTED — aguardando CI do HEAD |
| Convenção temporal pre-shift do bit de saída | REFERÊNCIA DIFERENCIAL ADICIONADA |
| Noise Rate = Tone 2 / fase relativa | NOT CLOSED — A1.3 |
| Períodos de tom 0/1 | CONFLICT / ORACLE REQUIRED |
| Reset completo e estado versionado | A1.4 |
| Teste físico em Master System | BLOCKED / NOT RUN |

A1.2 permanece parcial: os vetores de ruído foram adicionados, mas só serão promovidos a PASS após CI verde no mesmo SHA. A sequência de LFSR não fecha por si só a temporização do divisor, o alinhamento do Noise Rate = Tone 2 nem o modelo analógico de saída.


## 11. A1.3 — caracterização parcial do divisor Noise Rate = Tone 2 (2026-11-12)

**Referência independente:** Gearsystem, commit `1be9a12b82cae84ddd6f9b7006d546abe2d58bd4`, `src/audio/Sms_Apu.cpp`.
**Escopo:** caracterizar relação de período e continuidade do intervalo pendente; não declarar alinhamento absoluto de fase.

### 11.1 Evidência comparativa

No Gearsystem, o registrador de ruído seleciona diretamente um ponteiro para `squares[2].period`. A rotina `Sms_Noise::run` calcula o intervalo de deslocamento como `2 * *period` e mantém `delay` entre chamadas. Uma escrita no registrador de tom 2 altera o período consultado nas próximas iterações; uma escrita no registrador de ruído reinicializa `shifter`, mas não zera `delay`.

Isso sustenta dois requisitos verificáveis:
1. taxa média de shift = uma transição por dois períodos de Tone 2;
2. alteração do período de Tone 2 não deve descartar o tempo parcial já transcorrido no divisor de ruído.

Fonte: https://github.com/drhelius/Gearsystem/blob/1be9a12b82cae84ddd6f9b7006d546abe2d58bd4/src/audio/Sms_Apu.cpp

### 11.2 Teste de regressão adicionado

`sms_tone_two_noise_rate_tracks_period_without_resetting_partial_divider`:
- configura Tone 2 = 3 e Noise Rate = Tone 2;
- avança um tick e altera Tone 2 para 2 com dois ticks restantes no contador;
- verifica que o contador parcial permanece;
- verifica o primeiro shift na expiração prevista;
- verifica o próximo shift após dois períodos do novo valor.

O teste não compara a fase inicial absoluta com hardware e não prova acoplamento elétrico ao flip-flop de Tone 2. É um teste de relação temporal média e continuidade do intervalo pendente, derivado do modelo independente.

### 11.3 Limite de conclusão

| Propriedade | Estado |
|---|---|
| Noise period acompanha o valor de Tone 2 | TEST ADDED |
| Shift a cada duas expirações após alinhamento conhecido | TEST ADDED |
| Escrita Tone 2 preserva contador parcial | TEST ADDED |
| Fase inicial após power-on/reset | ORACLE REQUIRED |
| Fase após escrita Noise Register | PARTIAL — precisa trace independente |
| Fase após escrita Tone 2 em todos os boundaries | PARTIAL |
| Relação elétrica com flip-flop Tone 2 do VDP | NOT PROVEN |

A1.3 permanece **PARTIAL** até que a fase absoluta seja reconciliada com evidência independente ou captura de hardware. O contador de ruído independente permanece preservado; não deve ser substituído por conexão direta ao flip-flop de Tone 2 com base apenas na equivalência de frequência média.


## 12. A1.4 — DTO versionado e restauração in-memory (2026-11-12)

**Implementação:** `native/sn76489/src/psg.rs`, API exportada por `native/sn76489/src/lib.rs`.
**Versão do schema:** `SN76489_STATE_FORMAT_VERSION = 1`.

### 12.1 Conteúdo preservado

`Sn76489StateV1` captura:
- perfil físico, clock e sample rate;
- registradores de tom/atenuação/ruído e registrador latched;
- contadores e saídas dos três canais de tom;
- contador/fase do divisor de ruído, LFSR e saída de ruído;
- fase do divisor de clock e fase fracionária de amostragem;
- contador de samples descartados;
- histórico FIR e cursor;
- filas PCM pendentes, incluindo a fila interna de renderização.

Os coeficientes FIR não são duplicados no DTO: são reconstruídos deterministicamente a partir do par imutável `clock_hz/sample_rate`. O histórico e o cursor — que carregam a memória temporal do filtro — são preservados.

### 12.2 API e invariantes

- `snapshot_state()` captura o estado completo.
- `Sn76489Core::from_state(state)` constrói uma instância restaurada.
- `restore_state(state)` valida e substitui atomicamente; em erro, o dispositivo ativo permanece inalterado.
- Validação rejeita versão desconhecida, perfil LFSR inválido, valores fora da largura dos registradores, fases fora do domínio, período de ruído inconsistente, buffers incompatíveis e amostras não finitas.

### 12.3 Testes adicionados

- `versioned_state_round_trip_preserves_arbitrary_pipeline_phase`: round-trip após avanço não alinhado ao divisor 16, com fase de amostragem não nula, FIR aquecido e PCM pendente; compara estados após avanços adicionais em blocos diferentes.
- `invalid_versioned_state_is_rejected_without_mutating_live_device`: versão desconhecida e restauração atômica.
- `invalid_lfsr_state_is_rejected_without_partial_restore`: rejeição de estado impossível sem mutação parcial.

### 12.4 Codec binário e resultado

O DTO agora possui `encode()` e `decode()` em formato canônico little-endian:
- magic `SN76489\0`, versão e comprimento explícitos;
- checksum CRC-32 sobre o corpo;
- tags estritas para o perfil;
- vetores de FIR/PCM com comprimentos limitados antes da alocação;
- rejeição de magic inválido, versão desconhecida, truncamento, dados excedentes, checksum divergente e invariantes inválidas.

Testes adicionais:
- `versioned_state_codec_round_trip_is_canonical`;
- `versioned_state_codec_rejects_corruption_truncation_and_trailing_data`;
- `versioned_state_codec_rejects_unknown_schema_version`.

**Status A1.4:** IMPLEMENTED — aguardando CI no HEAD atual. A validação física continua separada; o codec confirma persistência digital, não equivalência analógica nem saída física.
