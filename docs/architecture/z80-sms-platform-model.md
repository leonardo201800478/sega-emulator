# Modelo Z80 para o Sega Master System — contrato de implementação

**Estado:** especificação de trabalho / plano de auditoria; não certifica completude da CPU atual.  
**Branch de origem:** `main` em `fc1c09a6b4082c60930b04cf0f89d4722c96ccee`.  
**Objetivo:** definir uma implementação Z80 nativa em Rust, determinística, testável e adequada ao barramento SMS, com perfis explícitos para o Master System original americano, o sistema doméstico japonês com FM quando presente e o Master System III brasileiro.

## 1. Princípios obrigatórios

1. O comportamento da CPU pertence a Rust. Python é frontend, automação, ferramentas e testes externos; não há callbacks Python por instrução, byte ou acesso de barramento.
2. Não usar `sg1000_core` como dependência do SMS. A implementação atual do SG-1000 permanece isolada. Reutilização futura exige crate neutro, contrato aprovado e regressões em ambas as máquinas.
3. Não chamar a CPU de “completa”, “precisa ciclo a ciclo” ou “compatível com hardware” apenas porque compila ou executa ROMs. Cada afirmação deve apontar para cobertura, vetor e nível temporal efetivamente testados.
4. Separar três modelos: **semântica de instrução**, **T-states nominais** e **sequência temporal de barramento**. T-states nominais não substituem M-cycles, wait states, contenção ou ordem dos acessos.
5. Perfil de máquina não altera arbitrariamente a semântica da CPU. Região, revisão do console, padrão de vídeo e presença do YM2413/OPLL são capacidades independentes.
6. Os testes devem usar vetores fixos/oráculos independentes; não gerar a expectativa com a mesma rotina que está sendo testada.
7. O modelo deve ser determinístico: mesma ROM/homebrew sintético, configuração, entrada e sequência de eventos produzem os mesmos checkpoints e hashes de estado.

## 2. Perfis de plataforma e configuração

A CPU Z80 é um componente; as variantes de console são configurações do sistema/barramento. Não criar três cópias da CPU para as três regiões.

| Perfil | Identidade | Região e vídeo | FM | Estado de evidência |
|---|---|---|---|---|
| Master System original — exportação/americano | SMS original export | Exportação; perfil de vídeo escolhido explicitamente (NTSC/PAL) | Ausente no perfil de fábrica por padrão; opção só se uma revisão/modificação documentada a oferecer | Perfil inicial proposto; confirmar detalhes de revisão em fontes |
| Master System doméstico japonês | SMS original domestic | Japão; vídeo e diferenças de I/O configurados separadamente | YM2413/OPLL configurável conforme revisão/modelo; não inferir apenas da região ou da ROM | A presença de FM depende do modelo/implementação concreta |
| Master System III brasileiro | Tec Toy SMS III | Brasil; revisão/modelo e perfil de vídeo próprios | Capacidade configurável somente quando a variante física suportar FM; não presumir que todos os SMS III tenham FM | Mapear variantes e diferenças com fontes e evidências antes de fixar defaults |

O perfil do sistema deve registrar, no mínimo: `model_id`, `territory`, `video_standard`, `fm_hardware`, revisão conhecida, mapa de portas/entradas e proveniência de cada decisão. Valores `Unknown` devem continuar explícitos até haver evidência; não os converter silenciosamente em “americano”, “japonês” ou “brasileiro”.

O FM é um dispositivo de áudio e uma capacidade do hardware, não uma propriedade universal do Z80. O caminho de portas/registradores e a mixagem YM2413 devem ser ativados apenas em configurações que declarem suporte. A presença física e os detalhes de detecção/controle devem ser comprovados separadamente para cada modelo.

## 3. Estado completo a modelar

### 3.1 Registradores e estado interno

- AF, BC, DE, HL primários e alternativos; IX, IY, SP e PC.
- I e R, incluindo preservação do bit 7 de R e incrementos em M1/prefixos/acknowledge conforme o modelo adotado.
- IFF1/IFF2, modos de interrupção IM 0/1/2, HALT, EI delay e estado de requisições INT/NMI.
- WZ/MEMPTR, latch Q e demais estados auxiliares somente onde a semântica e as evidências os exigirem.
- Estado de reset inicial explícito e distinto de estado de inicialização Rust por `Default`.
- Estados serializáveis/checkpointáveis com formato versionado quando forem expostos à aplicação.

### 3.2 Espaço de instruções e prefixos

A matriz de auditoria deve enumerar todos os 256 bytes de cada tabela aplicável, classificando cada entrada como implementada e testada, alias documentado, comportamento não definido/compatibilidade específica, ou lacuna explícita:

- Base (unprefixed).
- CB: rotações/deslocamentos, BIT, RES e SET, formas em registrador e `(HL)`.
- ED: aritmética 16-bit, transfers absolutos, I/O, registradores especiais, operações de bloco e repetição, aliases e opcodes não atribuídos.
- DD/FD: substituição IX/IY, formas indexadas, deslocamentos assinados e exceções documentadas em que H/L ou `(IX/IY+d)` interagem.
- DD-CB/FD-CB: operações indexadas de bit e escrita/duplicação para registradores conforme opcode.
- Prefixos repetidos, combinações de prefixos e opcodes não atribuídos: definir a política observada por oráculo/documentação, nunca converter lacunas silenciosamente em NOP sem evidência.

A cobertura de bytes deve ser complementada por cobertura semântica. Uma tabela 100% enumerada não prova correção de flags, ordem do barramento, ciclos ou estados auxiliares.

### 3.3 Flags e semântica

Testar S, Z, H, P/V, N, C e bits não documentados X/Y para cada família de instrução. Cobrir overflow assinado, carry/borrow, meia-carry, paridade, DAA, INC/DEC, 8/16-bit arithmetic, CP, operações lógicas, rotações, BIT e flags de instruções de bloco. Incluir casos de borda e amostras exaustivas quando o espaço de entrada for pequeno (por exemplo, ALU 8-bit).

Separar:
- resultado aritmético;
- flags documentadas;
- flags não documentadas/efeitos de WZ;
- flags em condições especiais de interrupção e instruções de bloco.

### 3.4 Interrupções e HALT

Matriz obrigatória:
- NMI: prioridade, saída de HALT, retorno, pilha, vetor e IFFs.
- INT mascarável em IM 0, IM 1 e IM 2, incluindo byte fornecido pelo barramento, vetor indireto, latência e comportamento quando IFF1 está desabilitado.
- EI seguido de instrução e janela de inibição de INT; DI.
- Alteração/desativação da linha INT; requests concorrentes e prioridade NMI/INT.
- HALT com incremento de R, avanço de PC conforme o modelo e saída por interrupção.
- RETN/RETI e quaisquer efeitos externos de RETI apenas quando o contrato de barramento os representar.

### 3.5 Tempo e barramento

A API de execução precisa declarar seu nível de fidelidade:
- **Nível A — instrução:** resultado/flags e T-states nominais por instrução; não declara precisão de acesso por ciclo.
- **Nível B — M-cycle:** sequência ordenada de leituras/escritas/I/O e ciclos de máquina com timestamps discretos.
- **Nível C — T-state/barramento:** fronteiras temporais necessárias para wait states, contenção e interação com dispositivos.

O nível B/C é uma evolução explícita, não um rótulo. Cada instrução/prefixo deve ter regressões para duração, acessos e ordem observável. Instruções que acessam memória e portas não podem ser validadas apenas pelo total de T-states. O scheduler SMS é responsável por arbitrar o tempo comum entre CPU, VDP, PSG, FM e interrupções.

## 4. Contrato de integração SMS

- Usar o trait de barramento neutro para memória e I/O; os métodos default de portas são placeholders e não podem representar silenciosamente o hardware SMS.
- O barramento SMS decodifica o mapa de memória, portas de controle, portas de VDP, PSG e FM conforme o perfil.
- O Z80 não conhece o tipo de console, região, Qt, Vulkan, WASAPI ou PyO3.
- CPU e dispositivos devem avançar em um domínio de tempo comum; escrita de porta precisa preservar timestamp e ordem.
- O runtime deve expor checkpoints para PC, SP, registradores, flags, IFF/IM, HALT, total de T-states, estado de barramento e eventos pendentes relevantes.
- A máquina SMS deve poder executar sem Python; a UI recebe blocos/frame/PCM por chamadas grossas.

## 5. Plano de teste e aceitação

### CPU unitária e vetores independentes
- Matriz base/CB/ED/DD/FD/DDCB/FDCB com status por opcode.
- Vetores fixos para flags, WZ/MEMPTR, Q, R, wraparound de PC/SP e little-endian.
- Casos inválidos/não atribuídos e prefixos repetidos.
- Reset determinístico, estado inicial, HALT e interrupções.
- Comparação contra pelo menos um oráculo independente (testes públicos de CPU, emulador de referência ou vetor documental com licença/procedência registrada). Não copiar código GPL sem avaliação de licença.

### Integração SMS
- Instruções que atravessam fronteiras 0x3FFF/0x4000, 0x7FFF/0x8000, 0xBFFF/0xC000 e wraparound 0xFFFF/0x0000.
- Leituras/escritas de memória, portas espelhadas e efeitos do mapper, VDP, PSG/FM.
- Interrupções de VDP com checkpoints de CPU e eventos.
- Mesma ROM sintética e mesma sequência de inputs em duas execuções: igualdade de checkpoints/saídas.
- Os três perfis usam o mesmo core CPU, mas devem validar diferenças de barramento/configuração e capacidades FM.
- Suite SG-1000 permanece intacta e executada; SMS não deve importar `sg1000_core`.

### Gates antes de declarar concluído
1. Nenhuma lacuna de opcode escondida; todas classificadas e documentadas.
2. Semântica e flags cobertas por vetores independentes.
3. T-states validados para cada forma implementada.
4. Nível de fidelidade temporal declarado; não alegar nível C sem sequência de barramento e regressões.
5. INT/NMI/HALT/IM0/1/2 validados.
6. Testes Rust workspace, lint e formatação aprovados no SHA exato da PR.
7. Testes diferenciais e regressões SMS/SG-1000 aprovados; resultados e limitações registrados.
8. Perfis de hardware e FM sustentados por fontes; hipóteses mantidas identificadas como tais.

## 6. Coordenação das três frentes de revisão

Estas frentes são revisões técnicas separadas e não substituem checks automatizados. O estado de cada uma deve ser reportado com evidências, sem apresentar um workflow como agente de IA.

1. **Frente A — ISA/estado da CPU:** inventariar implementação Rust existente, comparar com o modelo mais completo disponível no repositório e produzir a matriz por opcode, flags, prefixos e estado auxiliar.
2. **Frente B — barramento/tempo SMS:** definir o contrato CPU–bus e a matriz de eventos para memória, I/O, wait states, interrupções e scheduler. Identificar claramente o que ainda é apenas nominal T-state.
3. **Frente C — plataformas/oráculos/CI:** verificar fontes para modelos americano/japonês/Tec Toy, confirmar presença de FM por variante, preparar testes diferenciais e garantir que workflows validem o SHA candidato.

Cada frente deve entregar: arquivos/áreas examinados, evidências e fontes, riscos, casos de teste faltantes e recomendação PASS/FAIL/BLOCKED/NOT RUN. Não alegar execução de agente externo sem prova de que foi executado.

## Registro de auditoria Z80 — continuidade em 2026-10-28

Este registro consolida o estado observado após a integração de `05cb8259dfaaea5844c3290945301c88ec0fdc54`. Não altera o contrato da CPU nem converte evidência anterior em aprovação do SHA atual.

### Evidências atuais e limites

- A matriz `docs/architecture/z80-sms-opcode-matrix.csv` enumera 1.792 codificações fixas (256 por espaço BASE, CB, ED, DD, FD, DDCB e FDCB) e 11 classes de sequências de prefixos. Enumeração completa não significa semântica completa.
- `docs/architecture/z80-sms-reference-coverage.md` registra 172 vetores SingleStepTests/Z80. O próprio upstream declara que o corpus foi traduzido do núcleo Z80 do Ares e corrigido; portanto, ele não é um oráculo independente de uma implementação derivada de Ares.
- A cobertura indexed-CB de rotate/shift foi ampliada com testes dirigidos e varreduras do modelo interno. Esses testes não comparam eventos de barramento ordenados por T-state.
- O corpus externo `raxoft/z80test` é uma linha de evidência distinta, mas seu alvo original é ZX Spectrum e o harness SMS usa memória plana, substitui a saída de vídeo e simula portas. Cada discrepância exige triagem de variante/harness antes de ser classificada como bug do core.
- Um registro histórico separado menciona 1.604.000 vetores (1.556.612 aprovados e 47.388 falhos; 97,046%) em 2026-10-24. Esse número **não** corresponde aos 204 testes reportados pelo `raxoft/z80test`; a referência ao run/artifact original ainda precisa ser recuperada, portanto esse baseline fica **NOT VERIFIED** até ser vinculado a evidência consultável.

### Gate de conclusão da auditoria semântica

1. Executar `z80full` e `z80ccf` no SHA candidato atual, preservando logs/artefatos e o limite de instruções declarado.
2. Comparar contadores e resultados com o baseline histórico e agrupar falhas por teste/opcode/família, registrando a primeira divergência reproduzível e a classificação: bug do core, hipótese de variante, limitação do harness, teste inválido ou pendente.
3. Resolver ou justificar individualmente cada discrepância relevante; nenhuma falha pode ser removida/ignoradas apenas para obter um resultado verde.
4. Completar os vetores faltantes por famílias e estados de fronteira identificados na matriz, priorizando ED/bloco, prefixos, WZ/Q/R e interrupções. Fixar a procedência e a licença de cada oráculo.
5. Validar os cenários de reset, HALT, EI delay, NMI, INT IM0/1/2, IFF1/IFF2, wraparound de PC/SP e estado de refresh nos testes dirigidos.
6. Executar formatação, Clippy, testes Rust do workspace e workflows aplicáveis no SHA candidato exato; separar resultados de candidato e pós-merge.
7. Manter o nível temporal em **Nível A — T-states nominais por instrução** até existir contrato e regressão de eventos de barramento ordenados. Não declarar `TEMPORAL_PASS` com base apenas na duração agregada.
8. Manter o gate do Z80A do SG-1000 bloqueado até todos os critérios da PR #157 serem comprovados.

### Estado de execução deste registro

- Inventário de codificações: **IMPLEMENTED**.
- Correção semântica global: **NOT PASS**; evidência incompleta.
- Baseline externo anterior: **SHORT_PASS histórico**, com 47.388 falhas ainda por triagem.
- `raxoft/z80test` no SHA candidato `59ee21fd55184f77443431bf71ab7a10209fea74`: **PASS** no workflow [SMS Z80 Platform Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40773702379). `z80full`: 188.022.588 instruções, 1.132.230.280 T-states agregados, 204 testes reportados, 0 falhas. `z80ccf`: 88.340.467 instruções, 602.728.667 T-states agregados, 204 testes reportados, 0 falhas. Esses contadores não equivalem a 1,604 milhão de vetores por opcode nem validam barramento ciclo a ciclo.
- `cargo fmt --all -- --check`, Clippy do workspace e `cargo test --workspace --locked`: **PASS** no mesmo SHA pelo workflow acima; CI/quality também **PASS** no [run 40773702436](https://github.com/leonardo201800478/sega-emulator/actions/runs/40773702436).
- Reexecução do corpus separado de 1.604.000 vetores: **NOT RUN / artefato original não identificado nesta sessão**.
- Validação por eventos de barramento em T-state: **NOT RUN**.
- Liberação do gate Z80A/SG-1000: **BLOCKED**.

Este documento só será atualizado para promover um estado após evidência verificável do SHA e dos artefatos correspondentes.



## Estado efetivo no repositório — 2026-10-29

Este documento permanece um contrato de arquitetura, não uma certificação. O executor Z80 Rust ativo no SMS e os testes de controle de fluxo e trace ED estão integrados na `main` em `a662b7256ed5b92d06fd33875e4031b932e25945`. A máquina SMS completa, sua integração de dispositivos e a precisão temporal integral continuam sendo gates separados da semântica isolada da CPU.



## Estado consolidado após as PRs #227–#229 — 2026-10-29

**SHA de `main` verificado após os merges:** `a662b7256ed5b92d06fd33875e4031b932e25945`.  
**CI da `main` nesse SHA:** PASS — [workflow 40792001983](https://github.com/leonardo201800478/sega-emulator/actions/runs/40792001983).  
**Limite:** o CI verde comprova apenas os checks executados pelo workflow; não certifica completude global do Z80 nem fidelidade física/ciclo a ciclo.

### Mudanças integradas

- **PR #227**, candidato `d4447db5f0dce9a2ecde9e50c97f39b10ce8da17`, merge `28da159634aa60b9a804f3bd7f781ccf82f3e1cf`: trace por T-state para LDI/LDD/LDIR/LDDR e CPI/CPD/CPIR/CPDR sem prefixos DD/FD, além de trace M-cycle associado. Validação de plataforma no candidato: [run 40782245521](https://github.com/leonardo201800478/sega-emulator/actions/runs/40782245521) — PASS.
- **PR #229**, candidato `56cf4743bf0f05660f21a798de72e19afcfab34a`, merge `7b6fef3300c753c2b089f6b0dd287f8120b9f93f`: testes determinísticos para JR, JP, CALL, RET e DJNZ condicionais, caminhos tomados/não tomados, fronteiras de deslocamento, wraparound de PC/SP, ordem de acesso à pilha e T-states nominais; correção de `RET cc` para atualizar WZ/MEMPTR com o endereço de retorno somente quando a condição é tomada. [Fast Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40791517040), [CI](https://github.com/leonardo201800478/sega-emulator/actions/runs/40791516982) e [SMS Z80 Platform Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40791516943) — PASS no candidato exato.
- **PR #228**, candidato documental `a2c0850c20232c95a5210a39fbd53cc7aeb5101e`, merge `a662b7256ed5b92d06fd33875e4031b932e25945`: auditoria de continuidade incorporada; [CI documental](https://github.com/leonardo201800478/sega-emulator/actions/runs/40791436529) — PASS.

### Estado técnico depois dos merges

| Área | Estado | Limite/evidência |
|---|---|---|
| Executor Z80 Rust em `native/sms_core` | IMPLEMENTED | Caminho ativo consolidado; não significa que todos os estados de todas as instruções estejam provados. |
| Testes de controle de fluxo condicional | IMPLEMENTED; SHORT_PASS no candidato #229 | Cobertura adicionada às famílias JR/JP/CALL/RET/DJNZ e aos limites declarados; não é prova combinatória de todos os estados. |
| Trace ED de bloco | IMPLEMENTED; SHORT_PASS no candidato #227 | Restrito às oito instruções indicadas e ao perfil de observação documentado. |
| Classificação dos espaços de opcode | IMPLEMENTED | Enumeração/classificação não equivale à correção semântica integral. |
| Semântica global, flags não documentadas, Q/P, WZ, interrupções e aliases | INCOMPLETO / auditoria contínua | Exigir vetores discriminantes e proveniência explicitada por família. |
| Integração temporal com scheduler/dispositivos SMS | INTEGRATION_PENDING | A instrumentação atual não controla o scheduler nem representa todos os pinos/esperas. |
| Fidelidade ciclo a ciclo integral | TEMPORAL_PASS NÃO DECLARADO | Não há evidência para afirmar cobertura temporal completa ou equivalência física. |

### Proveniência e regras de interpretação

O corpus SingleStepTests/Z80 fixado em `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7` tem ancestralidade no núcleo Z80 do Ares; é útil para regressão, mas não é um oráculo independente de hardware. Separar sempre comportamento descrito pelo manual ZiLOG, comportamento documentado por referências de terceiros, observação em emuladores e hipótese ainda não comprovada. Não promover um estado de validação com base apenas no número de opcodes enumerados ou no CI geral.

