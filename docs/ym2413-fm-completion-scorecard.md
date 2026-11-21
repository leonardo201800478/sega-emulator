# YM2413 FM Completion Scorecard

**Data de referência:** 2026-11-21
**Repositório:** `leonardo201800478/sega-emulator`
**Main atual após integração FM-3A:** `624bb3dc956080b18b5b924f94f3ab4ce49c83e4` (merge commit da PR #1147, 2026-11-21)
**HEAD de código FM-3A validado antes do merge:** `c58a4215b81fff27e40a89c32b56f141f99d8964`
**PR #1154 — FM-5.2A:** HEAD `0422cc85dcb6ceacf58246f97db900b5f23ef2f4`, aberta e validada nos gates aplicáveis.
**PR #1156 — FM-5.2B:** branch draft empilhada sobre #1154; código validado no HEAD `6aed8b627b9b84b4459da1b0e65ffeb730b9fe35` (atualizações documentais posteriores na mesma branch).

Este scorecard distingue implementação de componente, integração funcional, validação temporal e evidência física. Uma etapa só é considerada concluída quando seus critérios próprios estão demonstrados.

## Estado executivo

**Status geral: EM DESENVOLVIMENTO — YM2413 ainda não está pronto para testes de ROM como subsistema de áudio funcional.**

Na `main` permanecem integrados os marcos FM-1, FM-2 e FM-3A: EG serial core-owned, serviço PG serial, operador FM, feedback, AM LFO e snapshots. As PRs #1133, #1145 e #1147 estão integradas. O FM-3A foi validado diferencialmente contra o Nuked-OPLL em 147.456 chamadas `OPLL_Clock`.

A PR #1154 acrescenta o sequenciamento serial do LFSR (FM-5.2A). A PR draft empilhada #1156 acrescenta o primeiro caminho integrado de Rhythm Mode (FM-5.2B): roteamento dos cinco seletores, parâmetros rhythm-derived para PG/operador, equações PG de HH/SD/TCY, avanço LFSR pós-EG e snapshots dos latches de fase/saída. A execução de síntese de percussão continua condicionada a uma seed explícita; o reset físico não foi certificado.

O FM-5.2B já possui diferencial isolado do operador e dos cinco latches de ritmo aprovado, além de caracterização integrada de 147.456 chamadas com estado rígido de controle/temporização/LFSR coincidente com Nuked. A caracterização reporta divergências nos parâmetros de síntese derivados da tabela candidata (`c_fb`, `c_dm`, `c_sl` e estados EG dependentes), sem mascará-las. A fixture integrada agora usa os valores de frequência da Tabela III-9 do manual Yamaha e a regressão afirma explicitamente os slots físicos BD 13/16, HH 14, TOM 15, SD 17 e TC 18. CI #9825 e PG #361 passaram no HEAD de código validado. Ainda faltam certificação física dos patches/seed, saída final MO/RO/DAC/PCM, acumulação dos nove canais, portas FM F0–F2 no SMS e mixer PSG+FM. Não há PCM FM integrado ao runtime SMS nem validação física do chip.

## Marcos já publicados

| Marco | Resultado | Evidência |
|---|---|---|
| Auditoria inicial | Integrada | PR #994; `docs/audits/ym2413-pre-implementation-audit-2026-11-17.md` |
| Fundação YM2413 | Integrada | PR #996; latch, registradores, 18 slots, 4 clocks/slot, rotação de 72 clocks, snapshot base |
| PG-1 | Integrada | PR #997; commit `a6adda38cc7db9317e93ca3c072f980fb5d239ec` |
| PM LFO global | Integrado | PR #998; commit `fc383defbe035e699f33b07e7a525cfdb5b7fa5a` |
| Auditoria de alinhamento PM | Integrada | PR #999; merge commit `1ce082a950ce2940b3f3de4c56dc7dbf37c536f9` |
| EG candidato | Integrado | PR #1001; merge commit `995dce7025b189ba25ce82a2dbf665d9cce9fb14` |
| Operadores FM e KSL | Integrados como candidatos isolados | PR #1003; merge commit `c9285d2fc2d8e3987197d5186478dae784da1570` |
| AM LFO | Integrado como candidato sample-clocked | PR #1005; merge commit `daf950d0e0bf3242dfb213d5e9e50a24db19a70e` |
| EG serial core-owned | Integrado e diferencialmente caracterizado | PR #1084; 18 operadores, 38 combinações AR/DR/KSR e replay do core |
| Hooks core-owned | Integrados | PR #1133; fronteira serial before/after EG |
| Serviço PG serial | Integrado e diferencialmente validado | PR #1145; 18 acumuladores, PG/PM latches, reset event e snapshots |
| Operador serial FM-3A | Integrado e diferencialmente validado | PR #1147; feedback, AM LFO serial e 147.456 chamadas OPLL_Clock |

## Roadmap vigente de fechamento — estado após PR #1147 e trabalho empilhado FM-5.2A/B (2026-11-21)

| Gate | Escopo | Estado atual | Evidência / bloqueio real |
|---|---|---|---|
| FM-1 | EG serial core-owned e contrato de eventos | **Integrado; diferencial de software aprovado** | PRs #1084 e #1133. O core mantém os 18 EGs e fornece os hooks before/after slot; não há certificação elétrica das subfases internas. |
| FM-2 | PG serial e integração PG↔EG | **Integrado; diferencial de software aprovado** | PR #1145. 18 acumuladores, latches PG/PM, evento de reset de fase e snapshots; diferencial Nuked e limites de restauração aprovados. O alinhamento físico absoluto continua em aberto. |
| FM-3 | Operadores, feedback e modulação | **FM-3A integrado e validado; FM-3 global parcial** | PR #1147. Operador serial, feedback de dois samples e AM LFO serial com diferencial integrado PASS. Faltam acumulação/roteamento final dos nove canais e PCM. |
| FM-4 | Nove canais melódicos e instrumentos | **Parcial** | Decoder de registradores, patches candidatos e `evaluate_channel` existem. Ainda falta serviço temporal que percorra os nove canais e acumule uma amostra YM2413 final; presets não certificados por medição física. |
| FM-5 | Rhythm Mode | **Parcial — caminho serial e caracterização de software integrados em branch draft** | PR #1154: LFSR serial FM-5.2A. PR #1156: roteamento BD/HH/TOM/SD/TCY, PG especial, operador e cinco latches. Diferencial isolado dos cinco latches PASS; caracterização integrada de 147.456 chamadas confirma controle/temporização/LFSR e reporta divergências da tabela candidata em `c_fb`, `c_dm`, `c_sl` e EG. Seed/patch físicos, MO/RO/DAC/PCM final e validação em hardware permanecem pendentes. |
| FM-6 | Saída digital FM / DAC / PCM | **Parcial — contrato MO/RO isolado** | Roteamento por slot e nível reconstruído de domínio de medição existem. Falta conversão operador→palavra bruta do DAC, definição da janela subclock e PCM final. |
| FM-7 | Perfis de hardware e integração SMS | **Pendente** | Portas F0–F2, latch/barramento, perfil Mark III/FM Unit e controle FM Auto/Off/On ainda não integrados ao runtime SMS. |
| FM-8 | Mixer PSG + FM e áudio | **Pendente** | Acumulação determinística, mixer PSG+FM, resampling/saída host e snapshots agregados da máquina ainda não integrados. |

As seções históricas abaixo registram a evolução por PR e hipótese. Em caso de divergência, esta tabela e a seção final de estado consolidado prevalecem.

## Scorecard histórico por etapa (numeração anterior — registro arquivado; não representa o estado atual)

| Etapa | Estado | Entrega existente | Pendência para conclusão |
|---|---|---|---|
| FM-0 — Auditoria e consolidação | **Concluída** | Main/PRs auditadas; PR #999 integrada; arquitetura documentada | Manter o scorecard atualizado conforme evidências novas |
| FM-1 — PG e EG | **Parcial** | PG-1; PM LFO global; EG serial de 18 slots publicado pela PR #1084; hooks core-owned na PR #1133; serviço PG serial integrado aos hooks na PR #1145, consumindo o evento de reset uma vez | PRs #1133/#1145 ainda abertas; operadores seriais, Rhythm Mode e pipeline PCM permanecem pendentes |
| FM-2 histórico — Operadores e síntese FM | **Parcial** | Tabelas Log-Sine/EXP; lookup de fase/sinal; par modulator/carrier; feedback de dois samples; KSL BLOCK/FNUM; snapshots locais | A integração serial dos operadores passa a ser FM-3 no roadmap vigente; validar latches e saída contra Nuked antes de PCM |
| FM-3 — LFO e modulação | **Parcial** | Contador PM global, tabela PM 8×8 e AM LFO de 210 posições/64 amostras, 14 níveis e snapshot | Conectar PM/AM aos operadores; comprovar fase absoluta e alinhamento temporal |
| FM-4 — Instrumentos | **Parcial** | User patch decoder; 15 presets melódicos candidatos com licença MIT/procedência; decoder FNUM/BLOCK/KEY/SUS/INST/VOL por canal | Validar patches contra hardware; ligar parâmetros ao PG/EG/operadores; confirmar os nove canais; ROM física exata ainda não disponível |
| FM-5 — Rhythm Mode | **Parcial** | Mapa de seis slots (PR #1010) e LFSR de 23 bits como primitiva explícita (PR #1011) | Síntese das cinco vozes, aplicação do ruído, volumes, seed/cadência física e temporização |
| FM-6 — Scheduler | **Pendente** | Core tem avanço de input clocks e rotação de 72 clocks | Clock domain, ordem dos slots, sample counter compartilhado, escritas temporalmente ordenadas e equivalência contínuo/blocos |
| FM-7 — Hardware SMS | **Pendente** | Nenhuma porta FM conectada | Portas F0–F2, latch, barramento, perfis Mark III/FM Unit/MK-2000/exportação e FM Auto/Off/On |
| FM-8 — Áudio e validação final | **Pendente** | Nenhuma saída PCM FM integrada | Mixer FM+PSG, resampling, clipping, continuidade, Save States agregados, regressões SMS/SG-1000 e testes com ROMs |

## Inventário funcional consolidado (main `624bb3dc`, 2026-11-21)

| Recurso | Implementação | Integração / validação |
|---|---|---|
| Latch e imagem de 64 registradores | Implementados | Integrados ao crate |
| Máquina de 18 slots | Implementada | Rotação de 72 clocks; pipeline funcional FM ainda pendente |
| PG-1 + serviço serial | **Integrado** | PR #1145 na main; 18 acumuladores, latches PG/PM, evento de reset, snapshots/restore e diferencial serial contra Nuked aprovados |
| PM LFO global | Implementado | Período 8192 amostras; não confundir avanço por amostras com clocks |
| EG serial (18 operadores) | **Integrado ao `Ym2413Core`** | PR #1084; 18 estados EG, timer compartilhado, KON/OFF/DOKON/SUS, decoder de registradores, snapshots e caracterização de 38 combinações AR/DR/KSR aprovada |
| Log-Sine / EXP | Tabelas inteiras e lookup integrados ao operador serial FM-3A | 256/256 entradas Log-Sine e 256/256 entradas EXP iguais às tabelas reconstruídas do Nuked; uso validado no diferencial FM-3A |
| Modulator / carrier | **FM-3A integrado** | PR #1147; diferencial integrado PASS em 147.456 chamadas `OPLL_Clock`, cobrindo os 18 ciclos. Acumulação final dos canais e PCM permanecem fora do escopo |
| Feedback FM | **Integrado e diferencialmente validado em FM-3A** | Histórico de dois samples por canal, seleção serial e comparação contra Nuked; ainda não há mixer/PCM final |
| KSL | Lookup BLOCK/FNUM e escala KSL=0..3 implementados | Consumido pela ponte de parâmetros por canal e pelo caminho de configuração de operadores; presets continuam candidatos |
| AM LFO | **Serial AM integrado em FM-3A** | Contador/direção/carry/step/output latch comparados contra Nuked; o antigo candidato sample-clocked permanece separado |
| Instrument ROM | 15 patches candidatos licenciados (MIT) e decodificação do user patch | Valores não são dump físico certificado; revisão de hardware ainda necessária |
| Channel register decode | FNUM/BLOCK/KEY-ON/SUSTAIN/INST/VOL para nove canais | Ponte estática `channel.rs` alimenta parâmetros por canal e `evaluate_channel`; falta o dispatcher que percorra os nove canais nos slots e acumule a saída final |
| Rhythm / LFSR | **Parcial; pipeline serial rhythm-derived e caracterização de software em branch draft** | PR #1156: snapshots e execução contínua/blocos; diferencial isolado do operador e cinco latches PASS; caracterização integrada de 147.456 slots confirma controle, seletor, LFSR e temporização, reportando divergências candidatas em feedback/waveform/sustain/EG. Seed e patches físicos, MO/RO/DAC/PCM final continuam pendentes. |
| Portas F0–F2 | Não integradas | Pendente |
| Mixer FM + PSG | **Pendente** | Nenhuma soma PCM final FM+PSG nem mixer integrado ao runtime |
| Save State agregado | **Parcial** | Core EG, serviço PG/operador e banco de síntese possuem snapshots próprios e testes de restore/continuação; falta um snapshot único agregado no estado da máquina SMS incluindo scheduler de slots, LFSR, saída e PCM |
| PCM FM | **Não disponível** | Não há amostra PCM FM final integrada; `Ym2413DacCode` é um nível reconstruído de domínio de medição, não palavra bruta do DAC |

## Contratos e incertezas que devem permanecer explícitos

1. **Contador e fronteira temporal:** o EG serial agora avança uma vez por slot completo dentro de `Ym2413Core`; o core só conclui o slot após acumular quatro input clocks. Esses quatro clocks são apenas a fronteira aritmética atual do slot, não subfases físicas EG/PG/OP. O alinhamento absoluto com reset IC e com PG/PM ainda exige evidência independente.
2. **DAMP:** o EG expõe limiar configurável. O valor 124 é um candidato do modelo de software; as notas de engenharia reversa também registram limiar próximo de -45 dB e incerteza.
3. **Coordenação modulator/carrier:** o EG oferece modelos explícitos independente e carrier-driven. Nenhum deve ser tratado como comportamento físico definitivo sem validação.
4. **AR=15:** o comportamento na fronteira DAMP/ATTACK é selecionável como candidato; não foi certificado em hardware pelo projeto.
5. **Slots:** a ordem exata de atualização de PG, EG, modulator e carrier deve ser estabelecida antes da integração temporal.
6. **Instrument ROM:** os parâmetros exatos dos instrumentos ainda exigem procedência e validação. Não copiar tabelas de emuladores sem licença e origem verificadas.
7. **Rhythm LFSR:** PR #1011 implementa a recorrência Galois de 23 bits com seed explícito. O polinômio/recorrência estão documentados por reverse engineering; o estado inicial/reset físico e a cadência de clock-enable continuam desconhecidos.
8. **Fidelidade:** nenhuma implementação atual equivale a certificação de fidelidade física completa.

## Gates CI publicados

### PR #999 — documentação PM

- HEAD validado: `701753f272e139dc89e946e5cabb3656eb06a5bf`
- Workflow #44227356163: sucesso nos gates documentais aplicáveis.
- Gates de runtime não executados para aquela alteração documental.

### PR #1001 — EG candidato

- HEAD validado: `ac08b570431042e9e9daf66211d04902d4f7964c`
- Workflow #44243755335:
  - Change impact classification: SUCCESS
  - Documentation patch hygiene: SUCCESS
  - Rust workspace regression: SUCCESS
  - Scheduler deterministic stress: SUCCESS
  - Rust quality and unit regression: SUCCESS
  - Validações de PSG/console, CPAL, SDL3, Z80, VDP e desktop não relacionadas: SKIPPED

**Interpretação:** os gates Rust aplicáveis ao EG passaram. Os itens SKIPPED não são aprovados por inferência e não validam a futura integração FM/SMS.

## Próxima dependência técnica

As PRs #1133, #1145 e #1147 estão integradas à main. A PR #1154 (FM-5.2A) e a PR draft empilhada #1156 (FM-5.2B) permanecem fora da main. Não reabrir EG/PG/operador FM-3A sem regressão demonstrada.

Próxima dependência do Rhythm Mode: reconciliar as divergências candidatas de `c_fb`, `c_dm`, `c_sl` e EG com evidência física, sem alterar parâmetros apenas para forçar paridade com Nuked; validar seed/patch em hardware; depois conectar retenção/roteamento MO/RO e definir palavra bruta do DAC/PCM. Em paralelo, FM-4 ainda precisa de dispatcher e acumulação dos nove canais melódicos.

Somente depois será correto fechar integração F0–F2 no SMS e mixer PSG+FM. Não usar integração SMS ou mixer como substituto da síntese FM ausente. Não declarar o chip pronto para ROMs enquanto não houver PCM FM reproduzível e regressões no runtime SMS/SG-1000.



### PR #1003 — operador FM e KSL

- HEAD validado: `25154edce743db8ed03f86e74641500dc2df722e`
- Merge commit: `c9285d2fc2d8e3987197d5186478dae784da1570`
- Workflow #44246095781: SUCCESS nos gates aplicáveis.
- Rust workspace regression, Rustfmt, Clippy, observability feature tests, Scheduler deterministic stress e documentation hygiene: SUCCESS.
- Validações de hardware/console/desktop não relacionadas: SKIPPED.



### PR #1005 — AM LFO

- HEAD validado: `5b929574bef91d3145c6dc818c0455a8883001a9`
- Merge commit: `daf950d0e0bf3242dfb213d5e9e50a24db19a70e`
- Workflow #44247005132: SUCCESS nos gates aplicáveis.
- Rustfmt, Clippy, workspace unit/integration tests, observability feature tests, Scheduler deterministic stress e documentation hygiene: SUCCESS.
- Validações de console/desktop não relacionadas: SKIPPED.

## Capacidade de execução multiagente

A sessão de desenvolvimento utilizada para estes marcos não disponibiliza três executores independentes. As três frentes foram coordenadas sequencialmente; não houve execução paralela de três agentes. A separação de responsabilidades permanece válida para a próxima etapa, mas não se deve registrar trabalho paralelo inexistente.

## Referências técnicas

- [SMS Power — YM2413 reverse engineering index](https://www.smspower.org/Development/YM2413)
- [Yamaha YM2413 Application Manual](https://www.smspower.org/maxim/Documents/YM2413ApplicationManual)
- [andete — decay/release measurements (2015-03-20)](https://www.smspower.org/Development/YM2413ReverseEngineeringNotes2015-03-20)
- [andete — attack measurements (2017-01-26)](https://www.smspower.org/Development/YM2413ReverseEngineeringNotes2017-01-26)
- [andete — ADSR transitions and operator behavior (2015-12-31)](https://www.smspower.org/Development/YM2413ReverseEngineeringNotes2015-12-31)
- [andete — rhythm/noise LFSR (2018-05-13)](https://www.smspower.org/Development/YM2413ReverseEngineeringNotes2018-05-13)


---

## Atualização FM-4 — ponte de parâmetros por canal

**Branch:** `feat/ym2413-channel-synthesis-bridge`
**Base de implementação:** main `a5a9eb826b086966b1436e1b584f91deeada78d4`

Foi adicionado `native/ym2413/src/channel.rs`, uma camada pura de resolução entre os registradores/instrumentos e os componentes de síntese existentes.

### Entrega

- Resolve os nove canais melódicos a partir de FNUM, BLOCK, KEY-ON, SUSTAIN, instrumento e volume.
- Resolve instrumento 0 para o user patch e 1–15 para os presets candidatos já licenciados/documentados.
- Converte MULTI e VIB do patch em entradas explícitas do PG por operador.
- Converte AR/DR/RR/SL, KSR, BLOCK/FNUM, SUS e EG-TYP em `EnvelopeParameters` por operador.
- Calcula KSL de modulator e carrier pela tabela existente.
- Constrói `OperatorPairParameters` com TL, VOL, feedback, waveform, KSL, níveis EG e AM habilitado individualmente por operador.
- Exporta os tipos e a função de resolução pelo crate `sega-ym2413`.

### Limites preservados

- A ponte não avança PG, EG, AM/PM, clocks ou slots.
- Não decide alinhamento de fase, ordem modulator/carrier, DAMP, reset, nem cadência global do EG.
- Não interpreta os canais 7–9 em Rhythm Mode; eles não podem ser enviados por este caminho melódico enquanto RHYTHM estiver ativo.
- Os presets continuam candidatos, não ROM física certificada.
- Nenhuma geração PCM, integração com Scheduler, portas SMS ou mixer é declarada.

### Testes adicionados — aguardando CI no HEAD da PR

- Resolução do user patch e mapeamento completo dos campos por operador.
- Resolução dos 15 presets nos nove canais.
- Rejeição de canal inválido e de código de preset inválido na API de resolução.
- AM enable independente por operador.
- Validação dos níveis dinâmicos EG/AM.
- Seleção de MULTI e PM no cálculo de incremento de fase.

**Estado:** FM-4 permanece PARCIAL. Esta entrega fecha somente a ponte estática de parâmetros; o próximo marco funcional é o pipeline agregado dos 18 slots, ainda sem declarar temporização física fechada.


---

## Atualização FM-4 — PR #1008 integrada

- PR #1008: feat(ym2413): connect channel patches to synthesis parameters
- Merge commit: c1b0ce50d1219bd97197901d2617b4f49f75b88e
- CI #44276258963: SUCCESS.
- Formatação, Clippy, testes unitários/integração do workspace e feature observability: SUCCESS.
- Gates de Scheduler, PSG, CPAL, SMS, SDL3, Z80 e desktop: SKIPPED por não serem afetados.
- A resolução melódica rejeita canais 7–9 quando RHYTHM está ativo.

A ponte de parâmetros está integrada, mas FM-4 continua PARCIAL: presets candidatos não foram validados em hardware e os parâmetros ainda precisam ser exercitados pelo pipeline agregado.

## Atualização FM-5 — banco agregado digital (em validação)

**Branch:** feat/ym2413-aggregate-synthesis-bank

Foi iniciado native/ym2413/src/synthesis.rs, que agrega:

- 18 acumuladores PG independentes;
- 18 geradores EG independentes;
- nove históricos de feedback modulator/carrier;
- um contador PM LFO e uma fase AM LFO globais;
- snapshot agregado versionado de todos os estados acima.

### Contrato preservado

- Fases PG, níveis/estágios/key states EG, feedback e fases iniciais dos LFOs são parâmetros explícitos; nenhum reset físico é inventado.
- A atualização PG e EG é feita por operador, permitindo que o futuro adaptador forneça a ordem de slots pesquisada.
- O contador global do EG continua externo e é fornecido em cada tick.
- A coordenação DAMP CarrierDrivesBoth permanece uma operação explícita, não automática.
- A avaliação modulator/carrier consome os estados PG/EG já atualizados e não avança clocks.
- O banco não implementa rotação de slots, PCM contínuo, mixer, portas SMS ou Scheduler.
- O avanço dos LFOs é explícito e recebe amostras concluídas externamente.

### Testes adicionados — aguardando CI

- Retenção dos 18 PGs e 18 EGs a partir de condições iniciais explícitas.
- Atualizações individuais PG/EG e avaliação de canal determinística.
- Avanço compartilhado dos LFOs independente da partição em blocos.
- Snapshot/restore de todos os estados locais e globais.
- Rejeição de versão/counter LFO inválidos e PG fora da faixa.
- Proteção contra resolver canais de ritmo como canais melódicos.

**Estado:** FM-5 PARCIAL. A agregação de estado não equivale à implementação da sequência física de 18 slots nem à geração de PCM FM.


---

## Atualização FM-6 — atribuição serial dos 18 slots

**Branch:** feat/ym2413-slot-sequencer-map

Foi adicionada a tabela constante YM2413_SLOT_MAP, baseada na Tabela II-3 do Yamaha YM2413 Application Manual. O índice do slot do core (0..17) agora pode ser consultado como descritor de canal, operador e atribuição opcional de voz rítmica.

### Ordem documentada

| Slots físicos | Canal | Operadores |
|---|---:|---|
| 1, 4 | 1 | Modulator, Carrier |
| 2, 5 | 2 | Modulator, Carrier |
| 3, 6 | 3 | Modulator, Carrier |
| 7, 10 | 4 | Modulator, Carrier |
| 8, 11 | 5 | Modulator, Carrier |
| 9, 12 | 6 | Modulator, Carrier |
| 13, 16 | 7 | Modulator, Carrier |
| 14, 17 | 8 | Modulator, Carrier |
| 15, 18 | 9 | Modulator, Carrier |

A ordem serial completa é M1, M2, M3, C1, C2, C3, M4, M5, M6, C4, C5, C6, M7, M8, M9, C7, C8, C9.

### Reatribuição em Rhythm Mode

- Slot 13 + 16: Bass Drum.
- Slot 14: Hi-Hat.
- Slot 15: Tom-Tom.
- Slot 17: Snare Drum.
- Slot 18: Top Cymbal.

O descritor de ritmo não implementa geração de percussão; preserva a atribuição física na camada de mapeamento.

### Integração no core

- Ym2413Core::next_slot_descriptor() expõe a atribuição do próximo slot sem avançar o tempo.
- O índice é o mesmo que já é avançado pela fundação de clock, com 4 input clocks por slot e 18 slots por rotação.
- Testes verificam a ordem, a reatribuição de seis slots rítmicos, rejeição de índices inválidos e o wrap após uma rotação.

### Limite técnico mantido

Esta entrega fecha apenas a tabela de atribuição serial. Ainda não implementa nem certifica:
- offsets entre as etapas PG, EG e operador;
- deslocamentos dos registradores internos de fase e envelope;
- pipeline de feedback e latência modulator → carrier;
- contador EG global e sua relação com os slots;
- eventos MO/RO, DAC e soma de saída;
- ritmo, noise LFSR e PCM FM final.

A documentação do chip confirma a reutilização da célula FM em 18 slots e a tabela de atribuição. A descrição de uma implementação Rust recente relata quatro input clocks por slot, pipeline PG/EG/OP com offsets e uma amostra por 18 clocks internos; isso é uma referência de arquitetura, não substitui a validação independente do nosso pipeline. O projeto não copia código GPL de Nuked-OPLL nem adiciona essa dependência.

**Referências externas:**
- Yamaha YM2413 Application Manual, II-5 Channels and Slots, IV-1 Clock Generation: https://map.grauw.nl/resources/sound/yamaha_ym2413.pdf
- SMS Power — YM2413 reverse engineering: https://www.smspower.org/Development/YM2413
- Descrição técnica de pipeline e clock (documentação Rust): https://docs.rs/emu198x-yamaha-ym2413/latest/emu198x_yamaha_ym2413/

**Estado:** FM-6 PARCIAL. O mapeamento serial está implementado e testado por CI pendente; o pipeline de estágios continua aberto.


---

## Atualização documental — PRs #1010 e #1011 / dossiê de evidências

**Referência:** [Dossiê YM2413 — pesquisa, evidências e lacunas](audits/ym2413-research-evidence-dossier-2026-11-18.md).

- PR #1010 integrou o mapa serial de 18 slots, com ordem baseada na Tabela II-3 do manual Yamaha e atribuição de seis slots em Rhythm Mode. Isso não implementa as subfases internas do pipeline.
- PR #1011 integrou o LFSR de ruído de 23 bits como primitiva explícita, com recorrência, snapshot e seed fornecido pelo chamador. O chip não avança esse LFSR automaticamente.
- O seed físico de reset, a cadência de clock-enable e a combinação do ruído nas quatro vozes não-Bass Drum permanecem pendentes.
- O scorecard anterior dizia que LFSR/slots não existiam; esse inventário foi corrigido. A síntese das cinco vozes de ritmo permanece pendente.
- O dossiê diferencia fatos do manual, medições publicadas, reverse engineering, inferências e lacunas. A prioridade seguinte é especificar o pipeline de 18 slots por subfase antes de conectar o banco agregado aos operadores.
- Nenhuma alteração de Scheduler, barramento SMS, mixer, saída PCM ou GUI é autorizada por esta atualização documental.

**Estado FM-5:** parcial — mapa de slots e gerador LFSR isolado; Rhythm Mode ainda não sintetiza as cinco vozes.

**Estado FM-6:** parcial — rotação e atribuição serial existem; offsets PG/EG/OP, feedback, DAC e latência continuam sem validação física.

**Validação:** alteração documental apenas. Não declara testes de hardware, PCM FM ou regressões de runtime executados.


---

## Investigação P1 — pipeline de 18 slots (2026-11-18)

Documento complementar: [YM2413 — Investigação P1 do pipeline de 18 slots](audits/ym2413-slot-pipeline-investigation-2026-11-18.md).

- Confirmada a ordem dos 18 slots e o período de 72 input clocks por rotação.
- Quatro clocks por slot permanecem uma derivação temporal; as quatro subfases funcionais não foram identificadas.
- PG de 19 bits, feedback com dois valores históricos, EG com contador global e saída MO/RO multiplexada foram mapeados às evidências disponíveis.
- Offsets de PG/EG/OP, latência M→C, commit de registradores e latch exato do DAC continuam pendentes.
- **FM-6 permanece pendente.** Esta investigação é documentação; não implementa nem valida o pipeline executável.

---

## FM-7 — decodificação estática dos registradores de Rhythm Mode

**Branch:** feat/ym2413-rhythm-register-decode

Foi adicionada a decodificação pura do estado dos cinco instrumentos de percussão, no módulo `native/ym2413/src/rhythm.rs`.

### Campos cobertos

- `$0E.D5`: habilitação de Rhythm Mode.
- `$0E.D4..D0`: triggers de Bass Drum, Snare Drum, Tom-Tom, Top Cymbal e Hi-Hat.
- `$36.D3..D0`: volume de Bass Drum.
- `$37.D7..D4`: volume de Hi-Hat; `$37.D3..D0`: volume de Snare Drum.
- `$38.D7..D4`: volume de Tom-Tom; `$38.D3..D0`: volume de Top Cymbal.
- FNUM, BLOCK, KEY-ON e SUS brutos dos canais 7–9 (índices 6–8), preservados separadamente dos registradores de volume de ritmo.

O método `is_active` representa somente o gate de registrador: Rhythm Mode habilitado **e** trigger correspondente em `$0E`. Não é um predicado completo de atividade do silício quando KEY-ON de `$26–$28` está setado. O manual manda limpar esses bits; as notas de andete sugerem uma possível combinação OR entre KEY-ON e triggers, ainda sem caracterização suficiente. O decoder preserva os bits crus e não altera os registradores do chamador.

### Testes adicionados

- Mapeamento dos cinco triggers e dos cinco nibbles de volume.
- Gate de ativação por Rhythm Mode, sem descartar o trigger bruto.
- Decodificação de FNUM/BLOCK/KEY/SUS dos canais 7–9.
- Garantia de que `$36–$38` são tratados como volumes de percussão, não como seleção melódica.

### Limites

- Nenhuma forma de onda ou envelope de percussão é sintetizado.
- O Bass Drum ainda não percorre o par FM de slots 13/16.
- As funções candidatas de fase para HH/SD/TCY, o LFSR slot-rate e os parâmetros de operadores por voz foram adicionados depois desta etapa estática; ainda não estão conectados ao banco agregado de síntese.
- A ROM física completa de ritmo não foi dumpada. O mapa de repetição de saída foi registrado, mas o buffer não foi implementado; o roteamento MO/RO continua pendente.
- Não há alteração no Scheduler, no banco agregado FM, no mixer ou no runtime SMS.

**Estado:** FM-7 PARCIAL — apenas configuração estática dos registradores. Os testes direcionados, incluindo os cinco casos one-hot, e o CI #44297715529 passaram no HEAD da PR #1015 antes do merge squash. Os gates não aplicáveis foram SKIPPED. Não equivale a Rhythm Mode funcional nem a PCM de percussão validado.

**Referência primária:** Yamaha YM2413 Application Manual, III-1-7 (Rhythm), III-1-8 (Instrument/Volume) e V-4 (Percussion Sound Synthesis): https://map.grauw.nl/resources/sound/yamaha_ym2413.pdf. A alocação dos bits do registrador $0E também foi comparada com as notas de reverse engineering de andete: https://www.smspower.org/Development/YM2413ReverseEngineeringNotes2015-12-16.

---
## Correção de estado — 2026-11-18 / PR #1014 integrada

**Main após PR #1015:** 1f0cfe37a5531c8e802ee9ba5f558eaa10b23797.

- PR #1014 foi integrada por squash. O commit de merge é o HEAD confirmado da main.
- Workflow CI #44292422118, executado no HEAD da PR b2a0b054b1fe6a5d18724dd83ff9f020e36629bc: Change impact classification, Documentation patch hygiene, Rust workspace regression e Rust quality/unit regression concluíram com SUCCESS.
- Os gates não relacionados foram SKIPPED e não são considerados validação do YM2413 em hardware.
- A decodificação estática de RHY/BD/SD/TOM/TOP-CY/HH, volumes e campos de frequência dos canais 7–9 está integrada em native/ym2413/src/rhythm.rs.
- FM-7 continua PARCIAL: não existe síntese das cinco vozes, ROM de envelopes de percussão validada, integração temporal do LFSR, PCM ou validação física.

### Reorganização da sequência de fechamento

A sequência de trabalho atual, definida para esta retomada, é:

1. **FM-7 — Rhythm Mode:** implementar e validar separadamente as cinco vozes, sem inventar cadência de LFSR ou fases temporais.
2. **FM-8 — Pipeline temporal:** fechar PG/EG/operadores/feedback/M→C/registradores/slots/DAC/MO-RO com contrato sustentado por evidências.
3. **FM-9 — Integração de áudio:** auditar native/audio_runtime e integrar somente após o contrato temporal estar validado.
4. **FM-10 — Validação independente:** vetores diferenciais, determinismo, snapshots, continuidade por blocos e comparação de hardware quando disponível.

Esta sequência substitui a numeração antiga de FM-5 a FM-8 acima, que refletia o roadmap anterior. Não altera o significado dos componentes já implementados nem promove estados de validação.

### Correção de validação da PR #1014

A frase anterior “os testes de unidade e o CI desta PR ainda precisam ser associados ao HEAD final” fica superada pelo registro acima. O CI passou no HEAD da PR antes do merge; não foi executado um workflow adicional no commit de merge, e isso não deve ser alegado.

**Referência de investigação:** audits/ym2413-rhythm-mode-investigation-2026-11-18.md.


## FM-7 — avanço a partir das capturas de Rhythm Mode (2026-11-18)

- PR #1015 integrada por squash; CI #44297715529 passou em Rust workspace regression, Rust quality/unit regression, Documentation patch hygiene e Change impact classification. Os demais gates foram SKIPPED.
- Main após merge: `1f0cfe37a5531c8e802ee9ba5f558eaa10b23797`.
- A investigação de andete em `results/rhythm/rhythm.md` e a atualização de LFSR em `results/lfsr/lfsr.txt` acrescentam capturas físicas e esclarecem a fase das vozes e a cadência interna do ruído.
- A primeira função Top Cymbal derivada do Genesis Plus GX foi descartada: usava os operadores/canais errados em relação às medições de andete. A branch #1016 foi corrigida para o candidato baseado em mod7 + car8 e na forma OR confirmada na atualização de Nuke.YKT. O relatório mantém explícitas as 16 variantes equivalentes por offsets PG iniciais desconhecidos.
- `rhythm_phase.rs` agora contém candidatos combinacionais de Top Cymbal, Hi-Hat e Snare Drum. As entradas de ruído permanecem explícitas; nenhum relógio é avançado dentro das funções.
- `rhythm.rs` preserva o LFSR decimado `x^23+x^9+x^8+x+1` e adiciona o LFSR slot-rate `x^23+x^9+1`, com um passo por slot de quatro clocks. O contrato medido é 18 passos por rotação de 72 clocks; HH no slot 13 e SD no slot 16, separados por três passos. Seed físico continua desconhecido.
- `instrument.rs` contém cinco conjuntos candidatos de parâmetros por operador derivados das capturas: BD, HH, SD, TOM e TCY. BD ainda apresenta diferença residual de alinhamento de uma amostra e pico; os bytes não são declarados como dump da ROM física.
- Testes adicionados cobrem a lógica combinacional, a relação de decimação do LFSR, o espaçamento HH→SD, limites de fase, parâmetros candidatos e snapshot/restore. São testes internos do modelo, não comparação física independente.
- A repetição medida de cada voz foi mapeada em `YM2413_RHYTHM_OUTPUT_MAP`: slots 0–2 e 6–7 reproduzem os valores originalmente gerados nos slots 13–17. O mecanismo de repetição não foi implementado. O banco de shift registers observado no die shot é apenas inferência estrutural, pois o SMS Power esclarece que as imagens disponíveis são de FHB013, não de YM2413.
- A interação entre KEY-ON melódico de $26–$28 e triggers de $0E permanece em aberto: o manual manda limpar KEY-ON; notas de andete sugerem que ambos os grupos de bits podem ser combinados por OR. `is_active` representa somente o gate $0E e não a atividade completa do hardware.

**Estado:** FM-7 segue PARCIAL. Os blocos de fase, ruído e parâmetros de operador estão implementados como candidatos isolados, mas não há integração de cinco vozes ao banco de síntese, buffer de duplicação, roteamento MO/RO, PCM, Scheduler ou validação contínua/blocos. A fase inicial dos operadores e o seed do LFSR permanecem desconhecidos.


## FM-7 — caminho carrier para operadores únicos (2026-11-18)

- A análise de andete registra que cada voz rítmica de operador único atua como carrier mesmo quando o slot físico é um modulator (HH/mod7 e TOM/mod8).
- Foi extraída a avaliação carrier pura para `evaluate_single_operator_as_carrier`, com a mesma atenuação `8×VOL + KSL + EG + AM`, saturada em 127, e a escala de saída `exponential_lookup >> 4` já usada pelo carrier do par FM.
- `OperatorPair::evaluate` reutiliza a função extraída, preservando o caminho existente e reduzindo risco de divergência entre FM normal e ritmo.
- Testes verificam equivalência com a saída carrier quando o modulator está silencioso, validação dos campos, silêncio EG e saturação de atenuação.
- Esta função não avança PG/EG, não altera feedback, não resolve o pipeline de slots e ainda não está conectada a uma configuração de voz rítmica.

**Estado:** bloco digital puro implementado; integração de HH/SD/TOM/TCY pendente.


## FM-7 — configuração estática das cinco vozes (2026-11-18)

- Adicionado `decode_rhythm_synthesis`, que compõe o decoder de registradores já existente com os cinco patches candidatos, os canais físicos 7–9, os slots 13–18, PG/EG e KSL.
- BD mantém dois operadores (mod7/slots 13 e 16); HH usa mod8/slot 14; TOM usa mod9/slot 15; SD usa car8/slot 17; TCY usa car9/slot 18.
- As quatro vozes de operador único declaram explicitamente o uso do caminho de amplitude carrier, mesmo quando o papel físico é modulator.
- Preservados FNUM/BLOCK, KEY-ON melódico bruto e SUS; a interação entre KEY-ON e trigger $0E não foi inferida.
- Testes cobrem mapeamento das cinco vozes, slots, papéis, triggers, volumes, frequência, KEY-ON/SUS, seleção de patches e RHY desligado.

**Estado:** decoder/configuração estática implementado. Nenhuma voz produz PCM; PG/EG ainda não foram conectados ao caminho de avaliação rítmica e não há pipeline temporal.


## FM-7 — ponte para avaliação dos operadores rítmicos (2026-11-18)

- `RhythmVoiceSynthesisConfig::single_operator_parameters` transforma volume, KSL, EG, AM e half-wave nos parâmetros do avaliador carrier já compartilhado.
- `evaluate_single_operator` consome fase e níveis dinâmicos fornecidos pelo chamador, sem avançar PG/EG/LFSR ou estado temporal.
- `bass_drum_operator_pair_parameters` monta os parâmetros do par FM de BD, preservando TL, feedback, KSL, volume e controles AM/half-wave; a mutação do histórico continua pertencendo a `OperatorPair`.
- Testes cobrem seleção do caminho carrier, AM enable, validação de níveis, avaliação equivalente ao helper compartilhado e parâmetros BD.

**Estado:** ponte digital pura concluída; a origem/cadência de fase e EG por slot, integração do LFSR e pipeline de áudio continuam pendentes.


## FM-7 — avaliação das vozes sobre o banco PG/EG compartilhado (2026-11-18)

- `Ym2413SynthesisBank` oferece `tick_rhythm_operator_envelope`, que mapeia o operador físico do slot para um dos 18 EGs existentes e recebe KEY-ON, política de release carrier e counter do chamador; não cria scheduler nem cadence.
- `evaluate_rhythm_voice` lê o EG do operador físico e usa a saída carrier para HH/TOM/SD/TCY. A fase é sempre explícita: HH/SD/TCY recebem o resultado dos combinadores candidatos e TOM recebe o índice do PG tonal. BD usa os dois PGs do canal 7 e o par FM existente.
- BD usa o `OperatorPair` existente do canal 7, preservando e atualizando o histórico de feedback desse par, com os parâmetros de patch e volume específicos de BD.
- O bit de trigger não é interpretado como mute gate na avaliação; o estado EG fornecido é que determina o nível atual e pode representar release tail. A origem do KEY-ON e a seleção da semântica de release do EG permanecem explicitamente sob controle do chamador.
- Testes verificam o mapeamento físico EG, equivalência do wrapper com a API existente, avaliação das quatro vozes simples com fases explícitas sem mutação de estado, feedback BD e rejeição de Rhythm Mode desabilitado.

**Estado:** ponte para avaliação digital sobre o banco compartilhado integrada na PR #1020 (squash `b989e33cf6d4903e831fd307553f49176f7a6897`). CI #8731 passou em format, Clippy, testes de workspace, Clippy/testes de observabilidade e higiene documental. Os gates não relacionados foram SKIPPED. Não representa execução serial de slots, repetição de saída, LFSR integrado, mistura MO/RO, PCM ou precisão temporal validada.


### Correção de contrato — fase explícita no avaliador de vozes (2026-11-18)

O avaliador do banco não deduz fase de HH/SD/TCY a partir do PG do operador de envelope. Essas vozes recebem `phase_index` explícito, que deve vir dos combinadores medidos e do bit LFSR no slot correto; TOM recebe o índice do PG normal do operador tonal. BD não aceita override e usa os dois PGs do par FM. Essa separação impede confundir o operador físico que fornece EG/volume com as fontes de fase combinada. O banco não avança essas fontes nem o LFSR.


## FM-7 — sequenciamento explícito do ruído nos slots HH/SD (2026-11-18)

- `Ym2413RhythmSlotNoiseSequencer` combina o LFSR slot-rate já existente com a posição zero-based de 0..17 dentro da rotação.
- `advance_slot` observa o bit atual antes de clockar o LFSR uma vez e avançar a posição. Em slot 13 retorna o bit para HH; em slot 16 retorna o bit para SD. Os dois eventos ficam separados por exatamente três avanços internos.
- Seed e posição inicial são explícitos; não existe seed/reset presumido. Snapshot inclui o índice do slot e o snapshot versionado do LFSR.
- Testes cobrem HH→SD, wrap 17→0, restore e rejeição de seed/índice/snapshot inválidos.

**Limites:** o sequenciador é chamado explicitamente uma vez por slot pelo owner futuro. Não consome clocks, não se conecta ao MachineScheduler e não determina o alinhamento absoluto de reset. O bit retornado ainda deve ser fornecido ao combinador de fase e à avaliação da voz.


## FM-7 — ponte entre amostras LFSR e candidatos de fase HH/SD (2026-11-18)

- `rhythm_noise_phase_candidate` recebe o evento emitido pelo sequenciador e um tipo explícito de entradas PG compatível com a voz.
- Hi-Hat exige slot 13 e fases do modulator do canal 7 e carrier do canal 8; Snare Drum exige slot 16 e fase do modulator do canal 7.
- O bit de ruído é consumido diretamente do evento, sem novo clock ou leitura implícita do LFSR.
- Combinações inconsistentes entre voz, índice de slot e variante de entrada são rejeitadas.
- Testes conectam o sequenciador aos candidatos existentes de HH/SD e cobrem a rejeição de eventos incompatíveis.

**Limites:** as equações permanecem candidatas de reverse engineering; offsets absolutos PG/reset não foram comprovados. O chamador ainda fornece fases PG na fronteira correta e encaminha o resultado ao avaliador de amplitude. Não há integração com o banco de slots, MachineScheduler, saída duplicada, DAC, MO/RO ou PCM. Nenhum contrato temporal novo é declarado.


## P2 — primeiros vetores temporais publicados do EG (2026-11-18)

**Branch:** `test/ym2413-eg-measured-rate-vectors`

Foram adicionados testes de regressão para confrontar o seletor de taxas do EG com sequências temporais publicadas por Andete:

- RATE=4: transições espaçadas por 8192 amostras.
- RATE=9: padrão 4096, 4096, 4096, 2048, 2048.
- RATE=14: padrão de intervalos 2048, 1024, 1024, com fase inicial do contador global explicitada.
- RATE=19: intervalo inicial de 1024 seguido por seis intervalos de 512 amostras.
- Carrier RELEASE: reutiliza o seletor temporal medido de DECAY com os parâmetros de release.
- ATTACK RATE=4: valida a janela de quatro atualizações por grupo de contador habilitado e os primeiros oito níveis 127→119→111→104→97→90→84→78→73.

Os testes usam o contador global como entrada explícita; a fase inicial é escolhida para reproduzir o segmento publicado e **não** representa alinhamento de reset do YM2413. Os casos validam a implementação do seletor/curva dentro das condições medidas, não o instante físico em que cada operador recebe o tick.

**Fonte experimental:** Andete, *YM2413 Reverse Engineering Notes 2015-03-20* (decay/release) e *2015-03-27* (attack): https://www.smspower.org/Development/YM2413ReverseEngineeringNotes2015-03-20 e https://www.smspower.org/Development/YM2413ReverseEngineeringNotes2015-03-27.

**Estado:** PR #1023 integrada por squash (`53820a140bd1109fab4a38efbea5c337ee0cc5bf`). CI #44382316288 passou em classificação de impacto, Rust workspace regression (format, Clippy, testes e observability) e higiene documental. FM-1 permanece PARCIAL e T06 continua CANDIDATE; nenhuma subfase do slot ou cadência física foi inferida.


## P2 — DAMP, KSR e retenção de sustain (2026-11-18)

**Branch:** `test/ym2413-eg-damp-ksr-sustain-vectors`

A auditoria dos vetores de EG identificou uma divergência funcional no cálculo de taxa efetiva: a implementação somava Rks mesmo quando a taxa programada R era zero. O manual Yamaha declara que R=0 mantém RATE=0; as medições de KSR documentam a soma do offset para taxas programadas não nulas. A função `EnvelopeParameters::effective_rate` agora trata R=0 como desabilitação do incremento, antes de aplicar KSR.

### Cobertura adicionada

- Matriz completa BLOCK 0..7 × FNUM8 0/1 para KSR=0 e KSR=1, com R=3.
- Matriz completa para R=0, confirmando RATE=0 independentemente de BLOCK/FNUM/KSR.
- Retenção de nível em Sustain para EG-TYPE sustentado com KSR habilitado e frequência máxima do conjunto testado.
- Vetor DAMP: com threshold de software candidato 124 e contador inicial explícito em zero, a implementação alcança o limiar no contador 494 (aprox. 9,94 ms a 3.579.545/72 updates/s), coerente com a duração aproximada de 10 ms descrita nas notas. Isso valida a aritmética do candidato, não o limiar físico nem o alinhamento de reset.

### Limites

O limiar 124 continua parametrizado e não é promovido a constante física. A coordenação DAMP modulator/carrier e o comportamento de AR=15 continuam candidatos selecionáveis. A medição de SL estabelece o alvo interno 8×SL, mas a questão de clipping versus comparação ao cruzar o alvo com incrementos rápidos permanece não resolvida. Nenhuma ordem de subfase, cadência de slot ou integração de Scheduler é inferida.

**Referências experimentais:** Andete, [2015-12-24 — Sustain Level e KSR](https://www.smspower.org/Development/YM2413ReverseEngineeringNotes2015-12-24) e [2015-12-31 — transições ADSR e DAMP](https://www.smspower.org/Development/YM2413ReverseEngineeringNotes2015-12-31); Yamaha, [YM2413 Application Manual](https://www.smspower.org/maxim/Documents/YM2413ApplicationManual).

**Estado:** alteração de código e vetores adicionados. O primeiro CI da branch (`44398643769`) detectou erro de inferência de tipo no teste da matriz FNUM (`0..=1` inferido como `i32`); corrigido na branch por tipagem explícita `0_u8..=1`. O CI do novo HEAD ainda precisa ser executado e aprovado. FM-1 permanece PARCIAL. Nenhuma alteração de runtime, Scheduler, PCM, portas SMS ou dependências.

**Caderno de evidências:** [YM2413 — saída, EG e Rhythm Mode](ym2413-research-ledger-2026-11-18.md), com pinagem MO/RO, ressalvas sobre circuitos analógicos, classificação das hipóteses de ritmo e critérios para validação ciclo-a-ciclo.

---

## Atualização FM-1 — correção RATE=0/KSR integrada (2026-11-18)

- PR #1024 integrada por squash.
- Merge commit: `f3480e453e32b6c4ce2d80314c8163c405d1f2ff`.
- CI do HEAD `b089f40cd7df1eab970943802b30c61b29bcf273`: SUCCESS (workflow #44402738227).
- `EnvelopeParameters::effective_rate` agora preserva RATE=0 independentemente de KSR.
- Cobertura: matriz BLOCK/FNUM8/KSR, retenção de sustain e vetor aritmético DAMP com threshold 124 explicitamente candidato.
- DAMP=124, coordenação modulator/carrier e AR=15 continuam sem certificação física.

FM-1 permanece PARCIAL: PG/EG ainda não estão conectados a uma execução serial de 18 slots com cadências demonstradas.

## Atualização FM-5 — vetores fixos de fase do Rhythm Mode (em validação)

**Branch:** `test/ym2413-rhythm-phase-reference-vectors`

Foi adicionado um fixture fixo de 32 combinações para a função booleana compartilhada por Hi-Hat e Top Cymbal, com vetores derivados da expressão final publicada por Andete em [YM2413 Rhythm mode](https://github.com/andete/ym2413/blob/master/results/rhythm/rhythm.md).

O teste também verifica as fases de saída de Top Cymbal e Hi-Hat para os dois estados possíveis do bit de ruído.

**Limites:** o fixture é uma regressão comportamental da expressão publicada; não determina os offsets absolutos dos PGs, o seed/reset físico do LFSR ou o instante de amostragem no pipeline. A origem permanece referência de engenharia reversa, não especificação primária nem captura produzida pelo projeto.

FM-5 permanece PARCIAL. Não foram alterados PG/EG, slots, Scheduler, PCM, barramento SMS ou dependências.

---

## FM-7 — período máximo do LFSR slot-rate (em validação)

**Branch:** `test/ym2413-slot-rate-lfsr-full-period-v2`

Foi adicionado um teste exaustivo do período da recorrência slot-rate candidata (x^23 + x^9 + 1), iniciando em uma semente explícita não nula:

- Executa os 2^23-1 passos da recorrência.
- Confirma que o estado não retorna à semente após 47 passos.
- Confirma que o estado não retorna à semente após 178481 passos.
- Confirma o retorno à semente após 2^23-1 passos.

Como 2^23-1 = 47 × 178481, e ambos são fatores primos, as verificações excluem os períodos próprios obtidos pela divisão por cada fator primo. O teste valida a propriedade algébrica da recorrência implementada e sua representação de estado.

**Limites:** não certifica o seed físico, alinhamento de reset, polaridade observada no pino, nem a cadência/instante de consumo do bit no YM2413. Não transforma o LFSR candidato em comportamento físico integralmente validado. Nenhuma lógica de runtime, slot sequencer, PG/EG, Scheduler, PCM ou barramento foi alterada.

---

## FM-8 — agregação digital das cinco vozes Rhythm Mode (em validação)

**Branch:** `feat/ym2413-rhythm-voice-batch-evaluation`

A API `Ym2413SynthesisBank::evaluate_rhythm_voices` reúne os cinco avaliadores já existentes em uma única operação lógica, recebendo endereços de fase explícitos para HH, SD, TOM e TCY. BD continua usando o par FM compartilhado de channel 7 e seu histórico de feedback.

### Contratos preservados
- Saída ordenada logicamente como BD, HH, SD, TOM e TCY.
- Avaliadores de operador único são executados antes de BD, pois não alteram estado; BD é executado por último porque atualiza o histórico de feedback.
- Fase, PG, EG, LFO, LFSR, posição de slot e tempo do chip continuam sob responsabilidade externa.
- A saída é um conjunto de cinco valores digitais assinados do caminho de síntese, não uma palavra DAC/MO/RO de nove bits nem um frame PCM.
- Não há retenção de saídas repetidas nos slots seguintes; a semântica do latch digital MO/RO continua pendente de definição independente.

### Testes
- Equivalência da avaliação em lote com as cinco avaliações individuais.
- Ordem estável das vozes na saída.
- Rejeição de fase inválida sem alteração do snapshot/feedback.
- Comparação integral do estado após avaliação em lote e execução individual equivalente.

**Limites:** esta API não cria uma ordem física de execução dos slots, não decide a chave de ativação dos operadores e não infere os instantes de PG/EG. A ordem de chamada dos avaliadores é apenas uma estratégia de atomicidade da API. Não integra Scheduler, PCM ou SMS.

## FM-2 — atraso de fase do modulator por amostra (correção candidata)

**Branch:** `fix/ym2413-modulator-one-sample-phase-history`

A auditoria da medição modulator/carrier de Andete (2015-12-03) identificou que o modelo que reproduz as capturas utiliza a fase do modulator da amostra de saída anterior (`lookupSin((i - 1) / 16)`). O vetor FNUM=0x40, BLOCK=0, MUL=0 tem incremento de 32 unidades no acumulador 10.9, equivalente a 1/16 de uma entrada da tabela por amostra. Portanto, subtrair uma entrada inteira da tabela (`phase_index - 1`) não representa esse atraso de uma amostra.

A alteração candidata:
- remove o deslocamento fixo de uma entrada da tabela;
- preserva o índice de fase do modulator utilizado na avaliação anterior;
- conserva esse histórico no snapshot de `OperatorPairState`;
- permite fornecer explicitamente o índice anterior inicial, sem presumir alinhamento de reset;
- mantém o histórico de feedback de dois outputs;
- incrementa as versões dos snapshots afetados.

**Testes adicionados/ajustados:** atraso de uma amostra para o vetor de incremento 1/16, snapshot/restore do histórico de fase, validação de índice inicial e atomicidade dos históricos diante de parâmetros/fases inválidos.

**Limites:** a nota de Andete é um modelo de saída ajustado às capturas analógicas, não uma leitura direta do latch interno. A correção estabelece o atraso em unidades de amostra do modelo; não determina a correspondência exata com os 72 input clocks, as subfases do slot ou o instante físico de consumo pelo carrier. O pipeline de 18 slots permanece pendente.

**Estado:** aguardando CI e revisão da PR. Não representa fechamento de FM-2 nem do pipeline temporal.


---

## Atualização FM-2 — histórico de fase do modulador integrado (2026-11-19)

- PR #1040 integrada à `main` por squash.
- HEAD validado pela CI: `e45b1c02fc3ad3bd86749ad2eae5897aaeb6a68c`.
- Merge commit: `60533292b2ea50376bc4dc48f0502032bc25ef9e`.
- CI #44443428226: classificação de impacto SUCCESS; Rust workspace regression SUCCESS (format, Clippy, testes unitários/integração e observability); documentation patch hygiene SUCCESS. Gates não relacionados foram SKIPPED.
- `OperatorPair` mantém o índice de fase do modulador usado na avaliação anterior; o estado é serializado e as versões dos snapshots foram incrementadas.
- O histórico de feedback de dois outputs foi preservado. O estado inicial da fase anterior pode ser fornecido explicitamente; ausência permanece sem alegação de reset físico.

**Escopo fechado:** correção do atraso de uma amostra de saída no modelo modulator/carrier, sustentada pelas capturas de Andete de 2015-12-03.

**Limites:** não identifica a subfase física de captura/consumo, não converte o atraso em input clocks, não fecha PG/EG/OP offsets e não integra PCM, portas SMS ou MachineScheduler. FM-2 permanece PARCIAL; o pipeline serial completo permanece pendente.

## Nota de proveniência — modelo de pipeline de terceiros (2026-11-19)

Foi localizada a documentação do crate Rust `emu198x_yamaha_ym2413` 0.1.0. Ela descreve um modelo em que PG, EG e operador trabalham em slots diferentes, com prescaler 4 e 18 clocks internos por amostra. A própria documentação declara que os offsets e a aritmética do pipeline seguem Nuked-OPLL 1.0.2, GPL-2.0-or-later, e que os testes comparam os pinos clock a clock.

**Classificação para este projeto:** referência comparativa de implementação, não evidência independente de silício. Por derivar explicitamente de Nuked-OPLL GPL, seu código e seus offsets não serão copiados nem adotados como especificação. A afirmação de pipeline é uma hipótese técnica útil para orientar a investigação, mas não fecha os offsets por si só.

O manual Yamaha confirma a passagem serial por 18 slots e os 72 input clocks por rotação; as medições públicas de saída confirmam multiplexação em 72 clocks. As fontes examinadas não fornecem uma tabela primária completa dos offsets de PG, EG, OP, feedback e DAC dentro dos quatro clocks de cada slot. Portanto, a decisão permanece: não implementar subfases presumidas.


---

## Atualização FM-8 — roteamento digital de saída MO/RO (2026-11-19)

**PR #1042:** [feat(ym2413): model measured MO/RO slot output routing](https://github.com/leonardo201800478/sega-emulator/pull/1042)

- Merge commit: `6c49f38bd8ca164daac0a6bc8454b0514a59392d`.
- HEAD validado: `8286d7f14652539696497c96555bb98a7ed33971`.
- CI #44450484181 (run #8804): Change impact classification SUCCESS; Cargo workspace validation SUCCESS; Rustfmt SUCCESS; Clippy SUCCESS; unit/integration tests SUCCESS; observability Clippy/tests SUCCESS; gate final Rust quality SUCCESS.
- Os jobs de Scheduler, PSG/SMS, CPAL, VDP, SDL3, Z80, desktop e higiene documental foram SKIPPED por classificação de impacto. Não são evidência de integração ou validação desses subsistemas.

### Entrega

Novo módulo `native/ym2413/src/output.rs`:

- Mapeia os slots carrier dos nove canais melódicos para MO.
- Em Rhythm Mode, restringe MO aos canais 1–6; os slots carrier 16–18 deixam de representar canais melódicos.
- Mapeia eventos originais e repetidos de Rhythm Mode para RO, validando slot, voz e indicador de repetição contra `YM2413_RHYTHM_OUTPUT_MAP`.
- Expõe a janela medida de três input clocks ativos dentro do orçamento de quatro clocks por slot.
- Preserva explicitamente o estado de repetição ainda não inicializada, sem tratá-lo como amostra zero.

### Limites preservados

- O pulso de três clocks é descrito em largura, mas o módulo não escolhe sua posição exata dentro dos quatro clocks.
- O nível neutro elétrico não foi definido.
- `Ym2413DacCode` continua sendo o domínio de nível assinado reconstruído pelas medições, não a palavra digital bruta de nove bits.
- A conversão do intermediário one's-complement do operador para o código DAC permanece bloqueada pela divergência de polaridade/offset documentada na Issue #1031.
- Nenhuma geração de PCM, mixer, filtro analógico, porta SMS ou integração MachineScheduler foi adicionada.

**Estado:** roteamento lógico de pinos e largura de pulso por slot integrados. FM-8 permanece PARCIAL; não há PCM FM integrado nem pipeline cycle-accurate completo.


## Atualização do bloqueio operador→DAC — variantes de polaridade (2026-11-19)

A auditoria adicional das notas de Andete identificou duas expressões de saída em modelos publicados:

- Nota 2015-12-03: `lookupExp(carrier) + 256`. O próprio texto informa que a forma de onda medida aparece invertida e suspeita de inversão de sinal no setup.
- Nota 2015-12-12: `255 - c`, no modelo de feedback que reproduz as capturas posteriores.
- Nota 2015-03-11: níveis reconstruídos como -255..+255, incluindo -0 e +0; a aquisição ADC apresenta ruído e espaçamento não uniforme.

Para o intermediário candidato x ∈ [-256,255], as fórmulas `x+256` e `255-x` são complementos exatos no domínio de 9 bits (soma 511). Invertem a polaridade e trocam os códigos dos dois zeros. Portanto, não são diferenças triviais de arredondamento.

A Issue #1031 continua aberta: nenhum desses modelos demonstra diretamente a palavra interna aplicada ao DAC físico. Não implementar conversão padrão, clamp de -256, saturação, wrap ou bias. A nova auditoria detalhada está em `docs/audits/ym2413-operator-dac-boundary-variants-2026-11-19.md`.


---

## Atualização FM-1 — evento de reset PG na conclusão de DAMP (2026-11-19)

A investigação do modelo Andete de 2017 identificou que a conclusão de DAMP é também uma fronteira de reset da fase PG no modelo de referência. A alteração em andamento expõe esse evento sem aplicar reset automático nem escolher a subfase/slot de consumo.

- API EG: resultado tipado com nível e indicador de reset de fase.
- Banco agregado: propagação do evento, reset explícito de um PG e helper de consumo do evento do carrier que aplica a política de coordenação selecionada.
- Compatibilidade: a API anterior de tick continua retornando o nível.
- Política CarrierDrivesBoth: continua explícita; o chamador consome o evento no limite escolhido. O helper sincroniza o estágio do modulator e reseta os dois PGs. Em IndependentPerOperator, somente o PG do operador indicado é resetado.
- Evidência e limites: docs/audits/ym2413-eg-phase-reset-events-2026-11-19.md.

**Estado:** evento tipado integrado pela PR #1045; consumo atômico integrado pela PR #1047; latch KEY-ON coerente na sincronização carrier-driven pela PR #1049; fronteira DAMP/ATTACK e atalho AR=15 alinhados ao modelo de referência pela PR #1050. FM-1 continua PARCIAL; não há integração temporal/por slots concluída.


---

## Atualização FM-1 — auditoria de transições EG e consumo de DAMP (2026-11-19)

**Main após integração:** `546274f2bd9af0474179dfa2da2669b77567e53e`.

### Entregas integradas

- **PR #1045** — expõe o evento tipado de conclusão DAMP/solicitação de reset PG. O EG não reseta PG nem avança clocks automaticamente.
- **PR #1046** — caracteriza por teste o valor intermediário candidato `-256` na fronteira do operador; não define a palavra bruta do DAC. Issue #1031 permanece aberta.
- **PR #1047** — adiciona consumo atômico do evento carrier: sob `CarrierDrivesBoth`, sincroniza o estágio do modulator e reseta os dois PGs; sob `IndependentPerOperator`, reseta somente o PG correspondente. O limite temporal continua sob responsabilidade do chamador.
- **PR #1049** — sincroniza também o latch KEY-ON do modulator ao aplicar a transição carrier-driven, evitando que o próximo tick interprete o mesmo KEY-ON como nova borda e reinicie DAMP.
- **PR #1050** — remove o retorno antecipado após DAMP já estar completo na entrada da atualização. O estágio recém-selecionado pode consumir o mesmo contador global, conforme `Operator::calcEnv` do modelo Andete `results/20170126/model5.cc`. O atalho AR=15 muda o estágio para DECAY sem zerar a atenuação DAMP.

### Validação PR #1050

- HEAD validado: `91f6fe66f893071cddad8c0a96fcee665aca6a58`.
- Merge commit: `546274f2bd9af0474179dfa2da2669b77567e53e`.
- Workflow #44512860362 (run #8835): SUCCESS.
- Rustfmt, Clippy workspace, testes unitários/integração, Clippy observability e testes observability: SUCCESS.
- Jobs de SMS/VDP, PSG, Scheduler, CPAL, SDL3, Z80, desktop e tooling não relacionados: SKIPPED; não representam validação desses subsistemas.

### Ressalvas e trabalho aberto

- O limiar DAMP=124 continua candidato, não constante física.
- A coordenação carrier/modulator continua selecionável; não há confirmação física definitiva da política.
- A cadência do contador global EG e sua fase absoluta permanecem fora do componente.
- O alinhamento do evento de reset PG com slots/subfases não foi estabelecido.
- O comportamento de KEY-OFF e a interação SUS × EG-TYPE precisam de matriz comparativa. A divergência entre a descrição experimental Andete 2015-12-31, o manual Yamaha e o modelo de software 2017 foi registrada na Issue #1051.
- `EnterAttack` com AR=15 continua sendo apenas um candidato explícito; a sequência do ataque em RATE=60..63 não foi modelada como um caminho físico separado.

**Estado:** EG permanece IMPLEMENTED como componente candidato, não INTEGRATED no sequenciador físico. FM-1 continua PARCIAL; não declarar ciclo-accurate.


---

## Atualização FM-1 — matriz de KEY-OFF carrier/modulator (2026-11-19)

**PR #1053** — [test(ym2413): cover carrier and modulator KEY-OFF paths](https://github.com/leonardo201800478/sega-emulator/pull/1053)

- Merge commit: `b343931941bb4e8b37e7253193672c20b2846284`.
- HEAD validado: `27bc6dcf7102f885d8c566b1ac7f34edbb5ffe6b`.
- CI #44513838591 (run #8840): SUCCESS.
- Rustfmt, Clippy workspace, unit/integration tests, Clippy observability e testes observability: SUCCESS.
- Gates não relacionados (SMS/VDP, PSG, Scheduler, CPAL, SDL3, Z80 e desktop): SKIPPED.

### Cobertura adicionada

- Carrier KEY-OFF partindo de DAMP, ATTACK, DECAY e SUSTAIN, verificando que o caminho de release do carrier prevalece sobre a fase anterior.
- Modulator EG-TYPE=1: mantém o nível de sustain após KEY-OFF.
- Modulator EG-TYPE=0: continua usando a taxa de release de primeira fase RR, sem entrar no caminho de segundo release do carrier.

**Limite:** testes reproduzem o comportamento candidato documentado nas medições de Andete; não resolvem a divergência carrier SUS × EG-TYPE registrada na Issue #1051. O total de testes unitários do módulo EG passa a 34. FM-1 continua PARCIAL.


---

## Atualização FM-1 — seleção SUS × EG-TYPE no KEY-OFF do carrier

- Decisão documentada em docs/audits/ym2413-sus-egtype-carrier-release-2026-11-19.md.
- Rate-base do carrier: EG-TYPE=0/SUS=0 → 7; EG-TYPE=0/SUS=1 → 5; EG-TYPE=1/SUS=0 → RR; EG-TYPE=1/SUS=1 → 5.
- Evidência prioritária: mapa de registradores e regra SUS do manual Yamaha; descrição de medições físicas de Andete 2015-12-31. O modelo de software Andete 2017-01-26 diverge para EG-TYPE=1/SUS=1 e não prevalece sobre essas duas fontes.
- O modulator permanece separado: EG-TYPE=1 mantém sustain no KEY-OFF; EG-TYPE=0 continua no primeiro RR, sem aplicar o release secundário do carrier.
- O rate-base selecionado continua passando por KSR; R=0 permanece sem incremento.
- Testes adicionados para a matriz EG-TYPE × SUS, RATE efetivo com KSR on/off e regressão de EG-TYPE=1/SUS=1 com RR=0.
- **Estado:** PR #1055 integrada à main pelo commit `98e87aecb7d76ab3ae434f6940b405f445456f9c`. CI #44516002877 (run #8847): SUCCESS nos gates Rust, classificação de impacto e higiene documental; os gates não relacionados permaneceram SKIPPED. A correção não implica integração do EG aos 18 slots, Scheduler, PCM ou conclusão FM-1.
- Com os três testes novos da PR #1055, o módulo EG passou de 34 para **37 testes unitários**.


---

## Atualização FM-1 — auditoria dinâmica de AR/DR/RR/SL/EG-TYPE/SUS (2026-11-19)

**Branch:** `audit/ym2413-dynamic-eg-registers`

A inspeção do EG e as notas de Andete 2015-12-31 indicam que AR, DR, RR, SL, EG-TYPE e SUS são consumidos dinamicamente pelo estado/fase em que se aplicam. “Imediato” significa que a taxa/condição nova é usada na próxima avaliação relevante e aguarda o gate apropriado do contador global; não significa incremento assíncrono do nível.

Foram adicionados onze testes de caracterização:
- AR durante ATTACK e DR durante DECAY;
- RR durante o primeiro release do modulator e o KEY-OFF do carrier;
- EG-TYPE alternado em SUSTAIN/primeiro RELEASE e no segundo RELEASE do carrier;
- SUS alternado durante o segundo RELEASE do carrier;
- SL alterado durante DECAY, incluindo alvo já ultrapassado, e depois de entrar em SUSTAIN;
- SL=1 no nível ímpar 9, observado por Andete.

**Ressalva SL:** o código continua usando `(level >> 3) == SL`. O nível 9 para SL=1 é compatível com essa hipótese, mas Andete não conseguiu distinguir experimentalmente se o comparador ignora um ou três bits inferiores. O teste registra a hipótese implementada, não certifica a largura física do comparador. Nenhuma lógica de produção foi alterada.

**Documentação:** `docs/audits/ym2413-dynamic-eg-register-updates-2026-11-19.md`.

**Cobertura:** módulo EG passa de 37 para 48 testes unitários se todos os novos testes forem aprovados pela CI. A execução de CI desta branch ainda é necessária.

**Limites:** a auditoria não demonstra cadência/fase absoluta do contador global, relação física com 72 input clocks/18 slots, nem instante de amostragem dos registradores. Não integra Scheduler, PG/LFO, DAC ou PCM. FM-1 permanece PARCIAL.


---

## Atualização FM-1 — integração SUS × EG-TYPE e auditoria do contador global (2026-11-19)

### PR #1055 — integrada

- HEAD: `094cc854c80cba081af43dcb59ace42e625e4d01`.
- Merge commit na main: `98e87aecb7d76ab3ae434f6940b405f445456f9c`.
- CI #44516002877 (run #8847): SUCCESS para Rust workspace regression, change impact classification, documentation patch hygiene e Rust quality/unit regression.
- Jobs de input, PSG, VDP, Scheduler, desktop, CPAL, SDL3, Z80 e tooling não relacionados: SKIPPED.
- A matriz SUS × EG-TYPE foi aplicada ao KEY-OFF do carrier; o caminho do modulator permanece separado. Issue #1051 está encerrada após a integração.

### PR #1056 — auditoria dinâmica integrada

- Merge commit: `16d9d4eff0fce8815f29c96ef2b392de11ed82d8`.
- CI #44522970048 (run #8858): SUCCESS nos gates aplicáveis.
- Onze testes de caracterização adicionados; inventário EG passa de 37 para **48 testes unitários**.
- Nenhuma lógica de produção alterada. O teste SL=1/nível 9 permanece caracterização da hipótese `(level >> 3) == SL`, não prova da largura física do comparador.

### Contador global do EG — resultado da auditoria

Documento atualizado: `docs/audits/ym2413-slot-pipeline-investigation-2026-11-18.md`, seção 10.

- Andete 2015-03-20 descreve um contador global compartilhado pelos 18 operadores, incrementado uma vez por amostra de saída.
- Uma amostra corresponde a uma rotação de 72 input clocks, portanto a cadência média derivada é aproximadamente 49.715,9027778 Hz.
- A fase desse contador altera o alinhamento dos gates e pode encurtar a primeira transição de envelope após uma mudança de fase ADSR.
- O algoritmo candidato atual consome o contador externamente. O EG não possui relógio nem contador próprio.
- `Ym2413Core::completed_rotations` conta rotações desde o reset lógico do objeto, mas não prova a origem/fase do contador físico, nem se o EG observa o valor pré ou pós-incremento na fronteira da rotação.

**Decisão:** não substituir o contador global por `completed_rotations`, não escolher fase inicial zero e não adicionar incremento por slot/input clock. A cadência média por rotação está sustentada; origem, largura/wrap, ordenação de incremento/consumo e alinhamento com PM/AM permanecem pendentes. Nenhuma lógica de produção foi alterada.

**FM-1 permanece PARCIAL.** A cadência média não fecha o pipeline de 18 slots nem o instante físico de serviço dos EGs.


---

## Atualização FM-1/FM-3 — auditoria cruzada PG, PM e AM (2026-11-19)

**Documentação:** `docs/audits/ym2413-pg-am-pm-cadence-2026-11-19.md`.

- PM: Andete 2015-12-01 mede oito posições, cada uma por 1.024 amostras, ciclo de 8.192 amostras (~6,069 Hz). A implementação candidata de 13 bits, bits 12..10, corresponde a essa cadência e mantém fase inicial explícita.
- AM: Andete 2015-11-28 mede 210 posições, 64 amostras por posição, 13.440 amostras (~3,699 Hz), 14 níveis. A implementação candidata preserva fase independente e explícita.
- PG: o acumulador de 19 bits avança uma vez por chamada de operador; a posição PM é lida no instante da chamada. A API não define a fronteira física de slot nem faz latch de PM por rotação.
- `advance_lfo_samples` avança PM e AM pelo mesmo delta em amostras, mas não demonstra que os dois LFOs compartilham o mesmo contador físico nem estabelece a ordem em relação aos updates PG.
- Andete 2015-03-20 descreve o contador global EG e sugere uso por outras funções; Andete 2015-12-01 descreve contador PM de 13 bits. A identidade física EG↔PM não está demonstrada. A nota AM não demonstra contador comum com EG/PM.

**Decisão:** manter PG, PM, AM e EG com estados explícitos e separados no software. Não reiniciar LFO em KEY-ON, não assumir reset zero, não derivar AM apenas do contador PM módulo 8.192 e não atualizar LFO no meio da rotação sem evidência de fronteira. O futuro sequenciador deve chamar os componentes nos limites de serviço pesquisados, preservando a ordem serial.

**Estado:** cadências médias PG/PM/AM descritas por medições publicadas e reproduzidas em componentes candidatos; offsets de slot, ordenação de atualização, fase inicial física, reset e identidade dos contadores continuam pendentes. FM-1/FM-3 permanecem PARCIAIS.


---

## Atualização FM-1/FM-6 — ordem de consumo do EG no modelo Andete 2017 (2026-11-19)

**Auditoria:** `docs/audits/ym2413-model5-eg-consumption-order-2026-11-19.md`.

A leitura direta de `results/20170126/model5.cc` esclarece a ordem do modelo de software:

- `YM2413::calc()` incrementa o contador global no início da amostra; o valor pós-incremento permanece compartilhado durante o processamento dos operadores/canais daquela chamada.
- Cada `Operator::calcOp()` executa PG antes de EG e usa a fase calculada antes do tick EG, mas o nível EG atualizado na saída do operador.
- Cada canal chama modulator antes de carrier.
- A transição DAMP do carrier sincroniza o EG do modulator e zera ambos os PGs após o cálculo PG do carrier naquela chamada.

A implementação Rust preserva o contador externo e expõe separadamente PG, EG e consumo do evento DAMP; ainda não impõe a sequência acima. Isso evita escolher prematuramente uma ordem de subfase física. O texto Andete `attack.txt` ressalva que o modelo ainda necessita de verificação adicional.

**Decisão:** nenhuma lógica de produção alterada. Não mapear a sequência de chamadas do modelo diretamente aos quatro clocks do slot, não assumir fase física zero e não derivar `completed_rotations` como contador EG físico. A ordem é uma hipótese de referência para vetores diferenciais, não fechamento cycle-accurate.

**Estado:** contador compartilhado por amostra e sequência de chamadas do modelo melhor caracterizados; origem/fase física, offsets PG/EG/OP e ponto de consumo do reset continuam pendentes. FM-1/FM-3 permanecem PARCIAIS; FM-6 permanece PENDENTE.

**Ressalva adicional da referência:** `model5.cc` contém literalmente `uint6 rate = max(63, 4 * basicRate + rks)`, embora `uint6` seja apenas alias de `uint8_t`. A expressão `std::max` não satura em 63 e é inconsistente com o campo de seis bits. A auditoria registra a anomalia como possível erro no modelo; não a reproduz nem presume silenciosamente que o autor pretendia `min`. O clamp do Rust permanece uma decisão separada, rastreada à largura de RATE e às medições.


---

## Atualização FM-1/FM-3/FM-6 — auditorias e vetor DAMP integrados (2026-11-19)

### PRs #1058–#1061

- **PR #1058** — auditoria da cadência do contador global EG e sua fase observável no modelo. CI #8861: gates aplicáveis SUCCESS. Documentação apenas; não cria contador nem escolhe fase física.
- **PR #1059** — auditoria cruzada das cadências PG, PM LFO e AM LFO. CI #8864: gates aplicáveis SUCCESS. Mantém fases e domínios de avanço separados; offsets de slot e alinhamento absoluto continuam pendentes.
- **PR #1060** — auditoria da ordem de consumo EG no modelo Andete 2017. Merge commit `878326d53c8815fb5f4fcf0bcf51b6331834258e`; CI #8870: gates aplicáveis SUCCESS. Documentação apenas; a ordem do modelo é referência para vetores, não prova de subfases físicas.
- **PR #1061** — vetor de teste `carrier_damp_reset_is_consumed_after_the_current_pair_sample`. Merge commit `1224d77db38274c1829c913ca46c07ac57948997`; CI #8876: Rustfmt, Clippy, testes unitários/integração dos workspaces e qualidade Rust SUCCESS.

### Resultado do vetor DAMP

O teste caracteriza, no contrato das APIs atuais, a sequência PG→EG do modulador, PG→EG do carrier, atualização do nível EG, avaliação do par e consumo posterior do evento de reset. Com AR=10, KSR desabilitado, FNUM=0, BLOCK=0 e contador 8, o nível do carrier transita de 127 para 119. A avaliação usa as fases calculadas antes do evento; o reset altera o estado PG para a atualização seguinte.

**Limite:** trata-se de uma sequência determinística de referência de software. Não estabelece o subclock físico em que DAMP, PG, EG ou avaliação FM ocorrem. Não houve alteração de lógica de produção e FM-1/FM-6 continuam PARCIAIS/PENDENTES conforme o scorecard.

### Próxima caracterização de software

A próxima validação isolada é a continuidade do par modulator/carrier após snapshot/restore, verificando conjuntamente os dois valores históricos de feedback e a fase do modulator da amostra anterior. O teste deve comparar execução contínua e restaurada sem introduzir estado inicial físico presumido, cadência de slots ou conversão DAC.


---

## Atualização FM-1/FM-3 — continuidade de snapshot do feedback (PR #1062, 2026-11-19)

**PR #1062:** `test(ym2413): verify feedback snapshot continuation`
**Merge commit:** `785bc45e1eb8c5bcba09570617ca2c852d5b00ec`
**CI #8879:** Rust workspace regression, formatação, Clippy, testes unitários/integração, observability e higiene documental SUCCESS.

O vetor avança uma amostra antes do snapshot e compara execução contínua e restaurada ao continuar com feedback 1, 4, 7 e 0, incluindo fases que cruzam o limite de 10 bits. Cada amostra e o estado serializado subsequente devem permanecer idênticos.

**Limite:** valida continuidade do estado de software do par. Não prova a posição física de captura do feedback, nem define offsets de slot/subfase.

### Próxima validação

A próxima prova de isolamento é em nível de banco: verificar que os nove canais melódicos mantêm pipelines de feedback independentes, que avaliar um canal não altera os oito restantes e que o snapshot agregado preserva os nove históricos. Isso valida a separação de estado do banco, sem declarar a ordem física de execução dos canais nem criar um sequenciador.


---

## Atualização FM-2/FM-3 — matriz de feedback assinado e isolamento por canal (2026-11-19)

### PR #1063 — isolamento dos nove canais integrado

- Merge commit: `f3c4d9b152c12263c606635f97edf10d10f6ae55`.
- CI #44558784375: Change impact classification, Rust workspace regression, documentation patch hygiene e Rust quality/unit regression SUCCESS.
- O teste `nine_melodic_channels_keep_independent_feedback_histories` verifica que avaliar cada canal altera somente seu próprio histórico de feedback e que o snapshot agregado restaura os nove históricos.
- A revisão automática de código não foi concluída por limite de uso do serviço; não reportou finding técnico. A validação obrigatória de CI aplicável passou.

**Limite:** isolamento de estado no modelo de software; não determina a ordem física dos canais, slots, subfases ou ponto de captura de feedback.

### Próxima caracterização — matriz FB=0..7 com históricos positivos e negativos

A nova matriz de teste exercita todos os oito valores de feedback com duas condições iniciais assinadas: soma positiva e soma negativa dos dois outputs históricos. Para cada caso, verifica:

- cálculo do deslocamento de fase usando os dois valores históricos;
- comportamento especial FB=0;
- uso da fase do modulator da amostra anterior;
- wrap de fase em dez bits;
- shift do histórico após a avaliação;
- retenção da fase corrente para a próxima amostra;
- históricos de entrada com soma positiva e negativa, sem interpretar o valor transportado em complemento de um como amplitude signed convencional.

**Limite:** esta matriz valida a aritmética e a continuidade do modelo de software. Não prova o ponto físico de captura do feedback, a largura/representação elétrica do sinal interno nem a latência M→C em input clocks.

### Gate para o pipeline de 18 slots

A investigação P1 permanece como referência. A ordem serial dos 18 slots está documentada, mas a função dos quatro input clocks dentro de cada slot, os offsets PG/EG/OP, a latência física M→C e o latch exato de MO/RO continuam sem evidência primária suficiente. Portanto, não será criado um dispatcher temporal nem integrado o banco ao Scheduler com subfases presumidas.

**Próxima dependência externa:** captura reproduzível de XIN, MO e RO com resolução por input clock, incluindo reset e escritas de registradores com offsets controlados. Sem esses vetores, a implementação cycle-accurate do pipeline permanece BLOCKED; não promover a hipótese de um emulador de referência a contrato físico.


---

## Atualização FM-2/FM-3 — fronteira lógica da rotação de 18 slots (PR #1065, 2026-11-19)

### PR #1064 — matriz de feedback assinada integrada

- Merge commit: `4bc964361da310e5550c0b4d4b747669266f1d65`.
- Workflow #44561067636: classificação de impacto, higiene documental, Rust workspace regression e Rust quality/unit regression SUCCESS.
- A matriz exercita FB=0..7 com somas positivas e negativas dos dois outputs históricos, verifica o offset, a fase anterior do modulator, wrap de 10 bits, shift do histórico e continuidade da fase.
- A revisão automática Codex não foi concluída por limite de uso; não há aprovação dessa revisão a registrar.
- Nenhuma lógica de produção foi alterada.

**Limite:** validação aritmética do modelo de software; não estabelece a captura física do feedback nem a latência M→C em input clocks.

### Nova caracterização de fronteira — rotação lógica

A caracterização adicionada nesta PR verifica, sem criar um sequenciador FM:

- o descritor atual permanece estável enquanto o core acumula os quatro input clocks atribuídos ao slot;
- o índice e o descritor avançam somente quando o orçamento de quatro clocks se completa;
- a sequência percorre os 18 descritores de `YM2413_SLOT_MAP` e retorna ao primeiro após 72 clocks;
- o estado de slot, fase parcial e rotação é preservado por snapshot/restore em todos os 72 offsets de clock de uma rotação.

**Interpretação:** os quatro clocks são tratados exclusivamente como intervalo aritmético do slot no contrato atual do core. Os índices de fase parcial não são associados a operações físicas PG, EG, SIN/EXP, feedback, DAC ou commit de registradores.

**Limites mantidos:** nenhum dispatcher de 18 slots, atualização automática do banco de síntese, PCM, integração com Scheduler ou fase absoluta após reset é introduzido. O teste demonstra consistência interna do contador/mapa e dos snapshots, não fidelidade física das subfases.

### Próximo gate

A implementação do pipeline interno continua BLOCKED até vetores físicos reproduzíveis de XIN, MO e RO com resolução por input clock, reset e escritas temporizadas. Em paralelo, a pesquisa pode aprofundar a latência M→C e os requisitos de captura, mas não converter a ordem de chamadas de modelos de software em contrato elétrico.


---

## Atualização FM-2/FM-3 — PR #1065 integrada e CI confirmado (2026-11-19)

- PR #1065: `test(ym2413): characterize slot rotation boundaries`.
- Commit validado no HEAD antes do merge: `de2d85c321ad17f537cb33527cf76401b87c9bf4`.
- Merge commit squash: `b36ec8519d7935fe42440207b10222799bb05ae0`.
- Workflow #8895 / run `44596283966`: SUCCESS.
- Change impact classification: SUCCESS.
- Documentation patch hygiene: SUCCESS.
- Rust workspace regression: SUCCESS, incluindo Rustfmt, Clippy workspace, testes unitários/integração, Clippy observability e testes observability.
- Rust quality and unit regression: SUCCESS.
- Gates Windows, SDL3, Z80, VDP, PSG, Scheduler determinism, CPAL e desktop: SKIPPED por classificação de impacto; não tratados como aprovados.
- A correção de `clippy::needless_range_loop` usa `YM2413_SLOT_MAP.iter().copied().enumerate()`, sem allow.
- A PR caracteriza a estabilidade lógica do descritor durante quatro clocks, rotação de 18 slots/72 clocks e snapshot/restore nos 72 offsets.

**Limite:** a rotação continua sendo um contrato aritmético do core. Nenhum subclock físico PG/EG/SIN/EXP/feedback/DAC foi identificado ou implementado. Nenhuma lógica de produção FM foi alterada.

## Caracterização M→C no domínio de amostras — PR #1067 integrada

**Branch:** `test/ym2413-modulator-carrier-sample-latency-ci`

A investigação documental foi concluída e o teste diferencial do modelo foi adicionado à PR #1067. Os workflows #8902 e #8903 concluíram com sucesso todos os gates aplicáveis: classificação de impacto, higiene documental, toolchain/Cargo, Rustfmt, Clippy, testes workspace, testes observability e quality/unit regression. Gates de Windows, SDL3, Z80, VDP, Scheduler e demais subsistemas foram SKIPPED pela classificação de impacto, não aprovados. A revisão Codex não foi concluída por limite de uso; não é tratada como aprovação. O objetivo do teste é demonstrar que o estado atual do par de operadores reproduz a relação de fase por amostra descrita no modelo publicado para FNUM=0x40, BLOCK=0, MUL=0, sem converter o atraso em input clocks.

**Não confundir:** a correspondência do modelo de saída não identifica o latch interno, o slot de captura/consumo ou a latência elétrica M→C. A integração cycle-accurate continua BLOCKED por captura física sincronizada de XIN/MO/RO.


---

## Atualização FM-2/FM-3 — caracterização M→C por amostra (PR #1067 integrada)

- PR #1067: `test(ym2413): characterize modulator carrier sample latency`.
- HEAD validado: `4eb431c60b765d290685e881b92427944dd7a946`.
- Merge squash na main: `63fb10e630b5d789e8c8887cd5cdd2cec588c988`.
- Workflows #8902, #8903 e #8904: gates aplicáveis SUCCESS, incluindo classificação de impacto, higiene documental, toolchain/Cargo, Rustfmt, Clippy, testes workspace, observability e quality/unit regression.
- Revisão Codex: não concluída por limite de uso; não contabilizada como aprovação.
- O teste percorre as 16.384 atualizações do vetor publicado FNUM=0x40, BLOCK=0, MUL=0 e caracteriza a fase anterior do modulator e a fase corrente do carrier no domínio de amostras.
- Nenhuma lógica de produção, clock, slot, Scheduler, PG, EG ou PCM foi alterada.

**Limite:** a latência de uma atualização é uma propriedade do modelo de saída documentado, não uma medição do latch interno nem uma conversão para input clocks. O pipeline físico M→C permanece BLOCKED por captura sincronizada XIN/MO/RO com resolução por input clock.

## Matriz de taxas PG × feedback — PR #1068 integrada

**Branch:** `test/ym2413-sample-domain-frequency-feedback-matrix`

Foi acrescentado um teste de regressão em `native/ym2413/src/operator.rs` que combina quatro vetores legais de FNUM/BLOCK/MUL, os oito valores FB=0..7 e históricos iniciais com somas positiva e negativa. Cada combinação percorre 512 atualizações.

O teste verifica separadamente:
- cálculo do offset de feedback a partir dos dois outputs históricos;
- consumo da fase do modulator da atualização anterior;
- fase corrente independente do carrier, incluindo MUL distinto;
- wrap do acumulador PG de 19 bits e wrap do índice de fase de 10 bits;
- atualização dos dois outputs históricos e retenção da fase corrente para a próxima chamada.

**Classificação da evidência:** esta matriz amplia a cobertura interna do modelo de software. Os vetores adicionais não são medições físicas nem novos vetores publicados de Andete. Não validam waveform analógica, reset físico, ordem dos slots, subfases, latência em XIN ou conversão DAC.

Nenhuma lógica de produção foi alterada. A matriz foi integrada após aprovação dos gates CI aplicáveis.


---

## Atualização FM-2/FM-3 — matriz PG × feedback integrada (PR #1068, 2026-11-19)

- PR #1068: `test(ym2413): cover sample latency across PG rates and feedback`.
- HEAD validado: `62eca186a932019942261f6e857726bdc4546d8a`.
- Merge squash na main: `bd7f3fa115b592117334e66f326532eb9d01c0e0`.
- Workflow #8908 / run `44622349207`: Change impact classification, Rust workspace regression e documentation patch hygiene SUCCESS; Rust quality and unit regression SUCCESS.
- Gates Windows, SDL3, Z80, VDP, Scheduler, PSG, CPAL e desktop: SKIPPED pela classificação de impacto; não são aprovações desses subsistemas.
- Revisão Codex: não concluída por limite de uso; não contabilizada como aprovação.
- O teste cobre 4 vetores FNUM/BLOCK/MUL × 8 valores FB × 2 históricos assinados × 512 atualizações = 32.768 avaliações.
- Verifica offset de feedback, fase anterior do modulator, fase independente do carrier, continuidade do histórico e wraps PG/fase.
- Nenhuma lógica de produção, Scheduler, rotação de slots, PG/EG ou PCM foi alterada.

**Limite:** a matriz amplia a regressão sample-domain do modelo de software. Não valida a latência física M→C em XIN, latch interno, subfases, ordem dos 18 slots, reset físico, waveform, DAC ou PCM.

## Próxima frente técnica — fechar a fronteira entre síntese de vozes e saída física

O banco agregado já possui avaliação das cinco vozes rítmicas, com Bass Drum usando o par modulator/carrier e as outras quatro vozes usando o caminho de operador único. A configuração estática preserva slots, patches candidatos, parâmetros PG/EG/KSL, trigger e volume. O caminho de avaliação não avança PG, EG, LFO, LFSR, slots ou clocks; exige que o chamador forneça as fases e os limites de serviço.

A próxima implementação não deve inventar a semântica de KEY-ON/trigger, a curva de volume, seed/cadência do ruído ou a conversão para código DAC. Primeiro, consolidar por testes os contratos já demonstráveis entre configuração de Rhythm Mode, estado PG/EG por slot, avaliação das cinco vozes e retenção/roteamento de saídas. Separar explicitamente qualquer resultado sample-domain da validação física.

**Bloqueios físicos mantidos:** captura sincronizada XIN/MO/RO para subfases e pipeline; curva/transferência DAC e correspondência de código físico (issue #1031); seed/cadência LFSR e interação trigger/KEY-ON quando não sustentadas por vetor primário.

---

## Atualização FM-7 — continuidade de snapshot do banco de síntese em Rhythm Mode (PR #1071, 2026-11-19)

- PR #1071: [test(ym2413): verify rhythm synthesis snapshot continuation](https://github.com/leonardo201800478/sega-emulator/pull/1071).
- Merge squash na main: `fb83533bf27aceb802985583da6c55a92e2a94b9`.
- HEAD validado antes do merge: `541711ad11bf282f322bc59e9f4a581f5a7b3dca`.
- CI run #44635637093: classificação de impacto, Rust workspace regression (Rustfmt, Clippy, testes unitários/integração e observability) e Rust quality/unit regression SUCCESS.
- Gates de Windows/Slint/WGPU, SDL3, Z80, VDP, Scheduler, PSG, CPAL e desktop foram SKIPPED pela classificação de impacto; não representam validação desses subsistemas.
- Revisão Codex não concluída por limite de uso; não foi contabilizada como aprovação.

### Cobertura integrada

O teste `rhythm_batch_snapshot_restore_preserves_all_voice_outputs_and_bd_feedback`:
- avalia o lote das cinco vozes com fases explícitas;
- exporta o snapshot agregado e restaura outra instância;
- continua ambas as instâncias com novas fases;
- compara as cinco saídas, o estado agregado completo e o histórico de feedback do Bass Drum (canal zero-based 6).

Nenhuma lógica de produção, cadência, PG/EG, LFSR, slot, Scheduler, DAC, PCM ou barramento foi alterada.

### Estado atual da fronteira Rhythm → saída

No HEAD desta atualização, os componentes digitais isolados existentes incluem:
- decodificação dos cinco triggers/volumes e dos campos FNUM/BLOCK/KEY-ON/SUS dos canais 7–9;
- configuração estática das cinco vozes e atribuição aos slots físicos;
- candidatos combinacionais de fase para HH, SD e Top Cymbal, com entradas PG explícitas;
- LFSR slot-rate e sequenciador explícito, com amostras de ruído para HH no slot 13 e SD no slot 16;
- avaliação das cinco vozes no banco agregado PG/EG, com BD usando o par FM e as quatro vozes restantes usando avaliação carrier de operador único;
- retenção/repetição digital dos valores de saída nos slots mapeados e roteamento lógico MO/RO, com largura ativa de três clocks por slot.

Esses itens são contratos de software e candidatos baseados em evidências publicadas. O avaliador de vozes ainda recebe fases e limites de serviço externamente; não existe dispatcher temporal integrado que avance PG, EG, LFO, LFSR e operadores na ordem física dos 18 slots. O caminho de saída também não converte o intermediário do operador em palavra bruta de DAC de nove bits.

### Próximos gates, sem inferência de comportamento físico

1. Consolidar por testes as interfaces já existentes entre sequenciador LFSR, candidatos de fase, PG/EG compartilhados, avaliação das cinco vozes, retenção e roteamento MO/RO; evitar duplicar cobertura existente.
2. Preservar a regra normativa do manual Yamaha: em Rhythm Mode, os KEY-ON de $26–$28 devem permanecer em zero; $0E.D4..D0 são os controles ON/OFF das cinco percussões. O decoder deve continuar expondo os bits crus, mas a futura camada de configuração pode sinalizar combinações não conformes. Não modelar comportamento do silício para KEY-ON=1 sem captura específica. Referências: Yamaha YM2413 Application Manual, §III-1-7 (https://www.smspower.org/maxim/Documents/YM2413ApplicationManual) e SMS Power — YM2413 (https://www.smspower.org/Development/YM2413).
3. Manter bloqueada a conversão operador→DAC: Issue #1031 não possui curva/transferência física conclusiva nem identificação da palavra interna aplicada ao DAC.
4. Manter bloqueado o sequenciador cycle-accurate até capturas reproduzíveis e sincronizadas de XIN, MO e RO com resolução por input clock, reset reproduzível e escritas timestampadas.
5. Não promover a cadência do LFSR, fase inicial PG, subfases dos quatro clocks por slot ou alinhamento do EG a contrato físico além do que os vetores primários sustentam.

**Status geral permanece: EM DESENVOLVIMENTO.** A PR #1071 fecha somente a regressão de continuidade de snapshot/restore do banco digital em Rhythm Mode. Não conclui FM-7, não fecha FM-8 temporal, não integra PCM ao SMS e não valida o YM2413 fisicamente.


---

## Fechamento do EG serial no core — PR #1084 (2026-11-20)

**Estado no branch:** IMPLEMENTADO + INTEGRADO AO `Ym2413Core` + VALIDAÇÃO DIFERENCIAL APROVADA.
**Estado na main:** INTEGRADO pela PR #1084; squash merge `2a1abf1e9443bd40650f1449da9f309e6d91536c` (2026-11-20). A PR #1083 foi supersedida e não precisa de merge independente.

### Entrega

- `Ym2413Core` mantém os 18 estados EG, timer compartilhado, latches KON/OFF/DOKON e sustain.
- Cada slot serial completo executa um ciclo EG; a conclusão do slot continua condicionada ao orçamento de quatro input clocks do core.
- Escritas de endereço/dados, atrasos do barramento, writes qualificados por ciclo e writes de modo alimentam o decoder EG register-driven.
- Estado EG e latches temporizados fazem parte do save state do core (formato v2).
- Rhythm Mode: decoder serial para ciclos 11–16, preservando bits crus de KON/SUS e os cinco triggers; fixture diferencial específico.
- `eg_core_trace` compara o estado EG do core com o trace do Nuked-OPLL por chamada `OPLL_Clock`.

### Evidência CI no HEAD exato

Commit: `0d7b32b2d350019880037422ae8f85b449ad4058`.

- Workflow **YM2413 Nuked EG characterization**: SUCCESS.
- Workflow **CI**: SUCCESS; Change impact classification, validação dos workflows, Rust workspace regression e Rust quality/unit regression concluídos com sucesso.
- Workflow **YM2413 external Nuked oracle adapter**: SUCCESS.
- Sequenciador completo: 1.140.000 chamadas OPLL de replay em 38 configurações, zero divergências nos campos do sequenciador e zero divergências nos campos do timer.
- Replay do core integrado: canal único, quatro canais e as 38 configurações de taxa executados com sucesso.
- Operador EG: 506.768 amostras no replay de referência, zero divergências de nível/estágio.
- Análise de fase efetiva: 243.580 amostras, zero divergências de nível/estágio.
- Rhythm Mode: captura repetida idêntica e replay serial oracle-driven aprovados; testes unitários separados cobrem seletores, taxas de patch e triggers do decoder. O fixture de percussão não é uma comparação diferencial completa do decoder register-derived.
- Snapshots do core são exercitados durante os replays; o teste de round-trip do estado também é executado.

### Leitura correta dos resultados

A varredura mantém métricas de candidatos de fase-base que ainda divergem: 44.064 divergências de nível e 71 de estágio no conjunto de comparação baseline. Essas métricas não foram ocultadas nem tratadas como aprovação. O caminho de fase efetiva analisado separadamente é o que apresenta zero divergências nas 243.580 amostras. O replay integrado do core e o replay do sequenciador são gates independentes e passaram.

### Limites que permanecem

- Não há comprovação elétrica de subfases dentro dos quatro input clocks por slot.
- Não há captura física sincronizada XIN/MO/RO para estabelecer o alinhamento temporal absoluto.
- O EG não está conectado ao PG, aos operadores FM ou ao pipeline de PCM.
- O teste Rhythm Mode valida a sequência EG usando inputs do oracle; testes unitários separados verificam o decoder register-driven contra os valores candidatos de patch. Isso não certifica a ROM interna completa, cadência física LFSR, DAC ou comportamento não caracterizado de KEY-ON conflitante.
- Não há ainda validação com ROMs no runtime SMS, pois o chip não produz PCM FM integrado.

**Conclusão da etapa:** o escopo de EG serial digital integrado ao core está publicado na main e possui CI diferencial aprovado no HEAD validado. A etapa FM-1 como um todo permanece PARCIAL, pois PG/EG ainda não formam um pipeline temporal conjunto. O fechamento do EG não autoriza declarar o YM2413 completo nem iniciar validação de ROMs como se houvesse saída FM funcional.


---

## Current-main PG convergence — PR #1119 (2026-11-20)

**Estado:** PG integer increment and tested 18-slot accumulator path — SOFTWARE DIFFERENTIAL PASS. YM2413 pipeline — STILL IN DEVELOPMENT.

**Main merge:** `fc166cc8cca2e3a4a4d5c3565b5d125d3dc20408`.

### Production correction

`PhaseGenerator::phase_step` now preserves the two integer truncation points from the pinned Nuked-OPLL `OPLL_PhaseCalcIncrement` path:

```rust
freq = (corrected_fnum << block) >> 1;
pg_inc = (freq * pg_multi[mul]) >> 1;
```

This is not interchangeable with combining the shifts for odd PM-adjusted frequencies.

### Current-main Nuked differential

Workflow `44840972144`, job `136648936949` — SUCCESS:

- PG increments: 147,456 calls; 146,885 PM-enabled calls; PM positions 0..7; FNUM high rows 0..7; BLOCK 0..7; zero mismatches.
- 18-slot phase accumulator: 2,654,208 values; zero low-19-bit mismatches; all 18 slots active; 5,076 19-bit wrap events.
- KEY-ON response: 2,600 calls; 46,800 phase values; zero mismatches; 54 wrap events; 35 nonzero `eg_dokon` observations; two reset events supplied from the Nuked trace.

**Important boundary:** the KEY-ON comparison validates PG's response to oracle-supplied reset events. It does not yet validate reset-event generation from the Rust serial EG core or the integrated EG→PG pipeline.

### MAME/ymfm secondary audit

The same workflow completed a static source-level arithmetic comparison:

- Base/no-PM: 65,536 tuples; zero floor-normalized mismatches; 256 exact common-unit half-LSB cases.
- PM: 524,288 tuples; the quantized PM table matches after cyclic phase-origin offset 6 and truncation toward zero.
- Final PM increments still differ: 135,136 floor-normalized and 140,625 exact common-unit mismatches at the best rotation.

No MAME executable/runtime state was captured. This is not an independent MAME output PASS and does not override Nuked as the primary software oracle.

### CI gates

- CI run `44840972118`: SUCCESS — Rust workspace regression, Rust quality/unit regression, workflow validation and documentation patch hygiene passed.
- Nuked PG differential run `44840972144`: SUCCESS.
- External Nuked oracle adapter run `44840972169`: SUCCESS.
- Existing EG characterization run `44840972157`: SUCCESS.
- Unrelated Z80, Scheduler, SMS integration, Windows audio/input and Slint/WGPU jobs were SKIPPED; they remain SKIPPED, not PASS.
- No physical Windows or real-chip test was performed.

### Next gate

FM-1B remains open: connect the serial EG reset event to the unique PG consumer, preserve all delayed latches and prove continuous/partitioned equivalence. The slot-service mapping and offsets were integrated by PR #1124 (merge 8a80fc784c3b9b2563cea248f5c3ba1b4073dfc9). The core-owned pre/post EG service hooks are implemented in open PR #1133 (code HEAD 4b71a1f3093a3a3a184831a84d83b36abf15751b). The current code HEAD has Rust workspace regression/Clippy, PG arithmetic differential, external oracle adapter, and the 16-decision Rhythm/Test Mode reset differential passing. EG characterization is still running. The synthesis bank is not connected yet.

No PCM FM, complete operator/feedback pipeline, Rhythm Mode PCM, SMS FM ports or PSG+FM mixer is claimed complete.


---

## Atualização FM-2 — PR #1145 / serviço PG serial

**Branch:** feat/ym2413-fm2-serial-pg-integration
**PR:** #1145, aberta e empilhada sobre a PR #1133
**HEAD de código validado:** 56dfe6262d6e98ccae1bd32a3e622050a16ebf20

### Implementação

- Serviço PG conectado aos hooks core-owned before_eg_update / after_eg_update.
- Escrita substitutiva do acumulador físico: pg_phase[(cycle + 17) % 18] = pg_phase_next + pg_inc.
- Leitura independente do slot corrente e atualização do latch pg_phase_next.
- Latches pg_inc, FNUM, BLOCK, MUL, VIB e contador PM.
- Ordem serial preservada: PG Generate → PhaseCalcIncrement → tick EG do core → DoLFO → PreparePatch2.
- Test Mode bits 1, 2 e 3 tratados nos estágios correspondentes.
- Serviço rejeita Rhythm Mode e estados de transição rm_enable/rm_select até FM-5, sem mutar estado em caso de erro.
- Clone independente, rollback transacional, snapshots e API coarse-grained advance_input_clocks_with_pg_service.
- O EG continua com autoridade única no Ym2413Core; o serviço PG não avança o EG do banco de síntese.

### Evidência diferencial executável

Workflow **YM2413 Nuked PG differential**, run 44856572960, **SUCCESS** no HEAD acima:

- 147.456 chamadas de OPLL_Clock.
- 2.654.208 valores de fase comparados (18 slots por chamada).
- Todos os 18 slots ativos.
- 7.607 eventos de wrap de 19 bits no caminho Rust.
- 2 decisões de reset de fase e 35 chamadas com DOKON não zero durante KEY-ON/OFF.
- Comparação de pg_inc, pg_phase_next, pg_out, ciclo serial, contador PM e os 18 acumuladores.
- Differential separado de KEY-ON/KEY-OFF: 2.600 chamadas, 46.800 valores de fase, **PASS**.
- PG arithmetic differential e PG accumulator differential: **PASS**.
- Audit MAME/ymfm de normalização aritmética source-level: **PASS**. Não houve execução do binário MAME nem captura de estado runtime; não declarar triangulação de PCM ou de estado interno MAME.

**Limite da conclusão:** FM-2 está implementado e diferencialmente validado na branch da PR #1145, mas não foi integrado à main. Isso não valida ainda os operadores FM, PCM, Rhythm Mode completo, integração SMS ou mixer PSG+FM.

### CI no HEAD de código

Workflow CI run 44856572957: Rustfmt, Clippy workspace, testes unitários/integração do workspace e Clippy/testes da feature observability concluídos com **SUCCESS**. As validações não relacionadas de Windows/desktop/console permanecem SKIPPED e não são consideradas PASS.

A caracterização EG run 44856572947 ainda estava em execução no momento desta atualização.


---

## FM-3 — serial operator pipeline (2026-11-21)

**Status: IN PROGRESS — FM-3A implementation present; differential and integrated validation pending.**

- Added `native/ym2413/src/serial_operator.rs` with serial `OPLL_Operator` state, two-sample feedback arrays for nine channels, operator modulation, log-sine/EXP pipeline, waveform delay latches, EG output, and KSL/TL latches.
- Connected the operator stage to the core-owned slot hooks in Nuked order: PreparePatch1 → PhaseGenerate → Operator → PhaseCalcIncrement → EnvelopeOutput → EnvelopeKSLTL → core-owned EG update → DoLFO → PreparePatch2.
- `Ym2413Core` remains the only owner and updater of the 18-slot EG. The serial operator reads the core's envelope levels and does not create a second EG.
- Serial operator state and output latches are included in the PG service snapshot; PG service snapshot format is now version 2 on the FM-3 branch.
- Log-sine table comparison against pinned Nuked `logsinrom`: 256/256 values equal. EXP reconstruction `1024 + YM2413_EXP_TABLE[255-index]` against pinned Nuked `exprom`: 256/256 values equal.
- Added pre-call operator latch capture to the external Nuked trace adapter, a Rust replay example, and `compare_operator.py`. The workflow fixture covers nine melodic channels, a user patch with feedback/AM/VIB/DC/DM, KEY-ON/KEY-OFF, live writes, and Test Mode bits 0–3.
- The independent operator differential has **not yet run to PASS** on an FM-3 HEAD. No operator or integrated-pipeline differential success is claimed.
- Added `Ym2413SerialAmLfo` with the nine-bit AM counter, direction, carry, step and cycle-0 output latch. The operator preserves the full `u16` `pg_out` bus and masks the combined phase to ten bits, matching Nuked. It advances once per serial slot and implements Test Mode bits 1 and 3 from `OPLL_DoLFO`; the independent differential is pending. The older measured sample-domain AM candidate remains separate and is not used by the serial service.
- Rhythm Mode, PCM channel accumulation, digital output, DAC mapping, SMS FM ports, and PSG+FM mixing remain later roadmap stages.

### Required FM-3 closure gates

1. Operator internal-state differential PASS against pinned Nuked on the exact final HEAD.
2. Integrated PG + operator + core-owned EG differential with Rust-generated inputs, not oracle-injected inputs.
3. Latch timing and active-note parameter-write coverage for all nine channels.
4. Test Mode and AM LFO serial behavior differential (the serial AM comparator is now part of the Nuked workflow).
5. Continuous/partitioned equivalence, clone independence, snapshot/restore, rollback and retry idempotence.
6. Rust workspace regression, Clippy, formatting, and relevant CI gates PASS.
7. Independent review with no unresolved P1/P2 findings.


## Evidência FM-3A — 2026-11-21

**PR:** [#1147](https://github.com/leonardo201800478/sega-emulator/pull/1147) — branch `feat/ym2413-fm3-serial-operator` — HEAD `b538a6904c344fec5e1c5dc8bb08401d92c4c220`. PR aberta, sem merge. Base empilhada: #1145 sobre #1133.

| Gate | Execução | Resultado |
|---|---:|---|
| YM2413 Nuked PG differential | 44865109031 | PASS; operador/feedback, AM LFO, FM-3 integrado, PG serial, 18 acumuladores, KEY-ON/KEY-OFF e auditoria aritmética MAME/ymfm |
| Rust CI | 44865109035 | PASS; Rustfmt, Clippy, workspace unit/integration tests, observability feature tests, workflow validation e documentation hygiene |
| External Nuked oracle adapter | 44865109047 | PASS |
| Rhythm reset differential | 44865109138 | PASS; não implica Rhythm Mode completo |
| EG characterization | 44852382767 / 44865109038 | PASS no HEAD exato de #1133 (`c25511a7`) e no código FM-3A empilhado (`b538a690`); sweep de 38 configurações AR/DR/KSR completo |

- EG characterization no HEAD exato da PR #1133: run `44852382767` (run #258) **SUCCESS**, incluindo 38 configurações AR/DR/KSR.
- EG characterization no código FM-3A empilhado: run `44865109038` (run #402) **SUCCESS**.
- CI após a atualização documental: run `44865413665` **SUCCESS**; PG differential no mesmo HEAD documental: run `44865413667` **SUCCESS**.

### Cobertura diferencial FM-3A

- 147.456 chamadas de `OPLL_Clock`, cobrindo os 18 ciclos seriais.
- Operador: `op_exp_m`, `op_exp_s`, `op_logsin`, `op_neg`, `op_mod`, feedback, `ch_out`, índice de fase e saída.
- AM LFO: contador, direção, carry, step e saída; Test Mode 0, 1, 2, 4 e 8.
- PG serial: acumuladores `uint32_t`, `pg_phase_next`, `pg_inc` e latch `pg_out` `uint16_t`; 2.191.388 observações com bits acima do bit 18; dois eventos de reset de fase e 35 chamadas com DOKON ativo.
- O diferencial integrado preserva as comparações de `c_instr` e `c_op` na fronteira de seleção efetivamente consumida pelo Nuked.
- O temporizador EG recebe Test Mode bit 3 e o bit 2 de `write_data` no estágio de atualização correto.

### Evidência temporal e snapshots

- Execução contínua versus blocos irregulares: PASS nos testes do workspace.
- Snapshot/restauração conjunta de `Ym2413Core` e `Ym2413PgSerialService`, inclusive checkpoint após 37 input clocks: teste adicionado e executado pelo CI.
- Rollback transacional após falha injetada no ciclo 3 e retry comparado com execução de referência: teste adicionado e executado pelo CI.
- Clone independente e snapshots do serviço PG: testes existentes preservados e executados pelo CI.
- Findings: P1 da PR #1133 (aguardar EG characterization), P1 da PR #1145 e dois P2 da PR #1147 estão resolvidos com evidências e respostas. Nenhum thread de revisão permanece aberto nas três PRs empilhadas.

**Limite da conclusão:** FM-3A tem equivalência diferencial integrada demonstrada e seus testes Rust passaram. Isso não conclui FM-3 inteiro, FM-4, Rhythm Mode, acumulação digital de canais, PCM, mixer ou integração SMS. Não fazer merge antes da auditoria final das dependências empilhadas e do resultado final da caracterização EG.


## Registro histórico — reconciliação FM-3A pré-merge (superado pelo estado consolidado ao final)

- PR #1133: integrada à main pelo merge commit `f65e13c0e85d103faddd31845324432d8641a308`.
- PR #1145: branch `feat/ym2413-fm2-serial-pg-integration`, HEAD `6571ab9207cb3058d71b383b6682ee29d79676af`. O limite de restauração `pg_increment <= 987_840` e os testes de limite foram incorporados. CI, PG differential, Rhythm reset e external Nuked oracle adapter passaram nesse HEAD. A caracterização EG (38 combinações AR/DR/KSR) ainda está em execução; #1145 não foi mergeada.
- PR #1147: branch reconciliada sobre o HEAD `6571ab9207cb3058d71b383b6682ee29d79676af` da PR #1145, preservando a ancestry e o delta FM-3A. O novo HEAD será registrado após a criação do merge commit; os cinco workflows devem ser executados novamente nesse HEAD.
- Os dois threads P2 de #1147 permanecem resolvidos. A PR continua empilhada sobre #1145 até a integração desta última à main.
- Limite de escopo: FM-3A não conclui FM-3 inteiro, FM-4, Rhythm Mode, PCM, mixer ou integração SMS.

---

## Estado consolidado após merge da PR #1147 — 2026-11-21

**Estado de publicação**
- `main`: `624bb3dc956080b18b5b924f94f3ab4ce49c83e4`.
- PR #1133: merged — hooks core-owned do slot/EG.
- PR #1145: merged — serviço PG serial, latches PG/PM, evento de reset e snapshots.
- PR #1147: merged — operador serial FM-3A, feedback, AM LFO serial e diferenciais Nuked.
- PR #1146: encerrada sem merge por ser implementação anterior/sobreposta à #1147.

**Validação no HEAD de código FM-3A**
O HEAD `c58a4215b81fff27e40a89c32b56f141f99d8964` passou os cinco gates exigidos antes do merge:
1. CI/Rust workspace regression.
2. YM2413 Nuked PG differential, incluindo PG serial, operador/feedback, AM LFO e diferencial integrado.
3. YM2413 Nuked Rhythm reset differential.
4. YM2413 external Nuked oracle adapter.
5. YM2413 EG characterization — sweep completo de 38 combinações AR/DR/KSR.

O diferencial integrado comparou 147.456 chamadas `OPLL_Clock` nos 18 ciclos seriais, com entradas geradas pelo caminho Rust de produção e sem injetar no Rust os latches do oracle. O escopo aprovado é FM-3A; isso não significa que todos os blocos FM estejam completos.

**Validação do merge commit na main**
- CI run `44870216755`: SUCCESS. Change impact classification, documentation patch hygiene, Rust workspace regression, workflow validation e Rust quality/unit regression passaram.
- YM2413 external Nuked oracle adapter run `44870216690`: SUCCESS.
- Os gates de Scheduler stress, PSG/console, SMS, CPAL, SDL3, Slint/WGPU e Z80 foram SKIPPED pela classificação de impacto. SKIPPED não é PASS e não valida esses subsistemas.

**Estado técnico por fronteira**
- **Concluído no nível de software validado:** core EG serial; PG serial; operador FM-3A; feedback; AM LFO serial; snapshots/restauração e equivalência de execução em blocos nos serviços validados.
- **Implementado em blocos, ainda sem integração temporal completa:** nove canais/patches; cinco vozes Rhythm Mode; LFSR slot-rate; candidatos de fase; retenção e roteamento lógico MO/RO.
- **Ainda não implementado como pipeline final:** dispatcher determinístico dos 18 slots que coordene todas as unidades, acumulação dos nove canais, conversão operador→palavra física do DAC, PCM FM, portas FM F0–F2 no SMS e mixer PSG+FM.
- **Sem comprovação física:** seed/reset/cadência absoluta do LFSR, subfases dentro dos quatro input clocks por slot, alinhamento elétrico XIN/MO/RO e transferência analógica/DAC. O diferencial contra Nuked é validação comportamental de software, não medição do silício.

**Próxima sequência recomendada**
1. FM-4: consolidar o caminho dos nove canais melódicos e a resolução de parâmetros, sem ainda somar PCM.
2. Dispatcher FM-5: integrar o serviço serial dos 18 slots com PG, EG, operador, AM/PM, LFSR e eventos de saída, preservando uma única autoridade temporal.
3. FM-6: definir com evidência a representação digital e fechar o caminho operador→saída MO/RO→PCM; manter explícitas as lacunas do DAC físico.
4. FM-7: integrar portas F0–F2 e perfis de hardware SMS.
5. FM-8: mixer determinístico PSG+FM, snapshots agregados e validação no runtime SMS/SG-1000.

**Critério de conclusão geral:** não marcar YM2413 como concluído nem iniciar campanha de ROMs como validação de áudio funcional antes de haver PCM FM determinístico, integração ao Scheduler/runtime SMS, snapshots agregados e regressões reproduzíveis. A aprovação de FM-3A não substitui esses gates.

---

## FM-5.2C evidence reconciliation update — 2026-11-21

**Current investigation result: PARTIAL; candidate table intentionally unchanged.**

- Retrieved and reviewed the requested andete files at commit
  `4893d1c09faa0450b5d3355f19b7682d72822501`, including Rhythm Mode,
  LFSR, feedback, KSL, envelope, sweep/model6 and DAC measurement notes.
- Confirmed that the BD capture-derived patch is a close match, not an exact
  match; the source reports a one-sample alignment and peak-amplitude residual.
- Confirmed that the BD candidate's FB/DM bits are modulator-specific. The
  pinned Nuked rhythm patch model has separate BD modulator and carrier
  entries. Rust now latches FB=7/DM=1 only for the BD-modulator selector and
  FB=0/DM=0 for BD carrier and the other single-operator rhythm selectors.
- Added a directed six-selector regression for this per-slot latch contract.
- **Unresolved and preserved:** the measured SD carrier candidate is SL=6,
  while the pinned Nuked commit selects SL=4. The source material does not
  justify changing the Rust candidate. The associated EG-state divergence
  remains visible and has not been attributed exclusively to this SL mismatch.
- LFSR slot cadence is supported by the 2020 andete update: x^23+x^9+1,
  one step per operator slot, HH sample, three steps, SD sample, fifteen
  steps. The physical reset seed remains unknown.
- MO/RO measurements support two separate analog outputs, a 9-bit measured
  domain with 511 observed values (-255..+255), and repeated percussion
  outputs. They do not establish the internal DAC mapping. The suggested
  9x5 retention structure is inferred from an FHB013 derivative die shot,
  not a YM2413 die shot.

**Implementation commit:** `c9a5f73176ee34e585c5b912f4b705256ba84afd`.
**Audit:** `docs/audits/ym2413-fm5-2b-serial-rhythm-pipeline-2026-11-21.md`,
section “FM-5.2C investigation”.

**Gate status:** do not mark FM-5.2C complete until the new code HEAD passes
the isolated operator and integrated Rhythm Mode differential workflows,
and the remaining SD SL/EG divergence is either physically resolved or
explicitly carried as an accepted, documented limitation. No physical-chip
validation is claimed.

**Next dependency:** FM-5.3 — independently model the serial MO and RO output
events and retained/repeated rhythm outputs. Keep the two pins separate;
do not infer PCM or DAC codes from the signed operator intermediate.


### FM-5.2C post-change differential update — 2026-11-21

- PG differential #364: **SUCCESS** on code commit
  `c9a5f73176ee34e585c5b912f4b705256ba84afd`.
- Integrated Rhythm Mode trace: 147,456 calls across all 18 cycles.
- The corrected BD modulator-only FB/DM latches remove `c_fb` and `c_dm`
  from the integrated report's candidate-difference set.
- Remaining: `c_sl` (8,092 rows; Nuked 4 vs Rust 6),
  `eg_sl` (8,092 rows; Nuked 4 vs Rust 6), and
  `eg_state` (15,984 rows; first divergence at call 79,001).
- The SD SL candidate remains 6. The pinned Nuked model uses 4. This is
  unresolved and intentionally not normalized.
- CI #9828 failed only at rustfmt on code commit `c9a5f731...`; formatting
  was corrected in `6249ee472cde2f9f3615d5680d7ad0ddcdc3ff7e`. No fresh CI
  result exists yet for the updated branch HEAD.
- EG characterization #537 was still in progress at this update.

**FM-5.2C remains PARTIAL.** The per-slot FB/DM ownership correction is
validated against the pinned oracle, but the SD SL/EG divergence and physical
patch uncertainty remain open. Do not start FM-5.3 implementation until the
EG characterization result and exact-head CI are checked; then use the
measured MO/RO schedule as the basis for a separate serial output stage.
