# Revisão minuciosa dos artefatos de auditoria SMS_CORE — 2026-10-30

## Estado, escopo e proveniência

**Tipo:** auditoria estática dos artefatos fornecidos e confronto com o código remoto.  
**Base de código consultada:** branch `main`; SHA-base observado na base da PR #271: `756666ae6216d8466f40259c11c345bbc1921848`.  
**Runtime alterado:** não.  
**Build/testes executados nesta revisão:** não; as conclusões abaixo são inspeção de fontes e validação estrutural do JSON, não um `QUICK_PASS` do runtime.

### Artefatos recebidos e disponibilidade

| Artefato | Disponibilidade | Observação |
|---|---|---|
| `AUDITORIA_SMS_CORE.md` | Disponível, 6.543 bytes | Relatório narrativo de CPU Z80 e VDP |
| `z80_opcodes_audit.json` | Disponível, 251.732 bytes | 1.452 registros de opcode/prefixo |
| `z80_opcodes_audit.csv` | Não localizado entre os arquivos fornecidos | Não foi reconstruído a partir do JSON |
| `vdp_audit.csv` | Não localizado entre os arquivos fornecidos | A tabela completa de 50 itens não pôde ser auditada registro a registro |
| Manuais Zilog em PDF | Anexados na conversa | Referências para arquitetura/timing do Z80; não especificam os latches do VDP Sega |

A ausência dos dois CSVs é uma limitação material: o relatório afirma que eles contêm a matriz integral, mas esta revisão não presume nem inventa seus registros. A auditoria VDP abaixo cobre somente os itens descritos no Markdown e as verificações possíveis no código.

## 1. Integridade estrutural do JSON de opcodes

O arquivo foi carregado e validado integralmente como JSON. Resultado:

| Verificação | Resultado |
|---|---:|
| Registros | 1.452 |
| Campos por registro | 7, consistentes em todos |
| Chaves obrigatórias ausentes | 0 |
| Pares `prefix + opcode` duplicados | 0 |
| Opcode fora do formato hexadecimal de 2 dígitos | 0 |
| Registros sem `status=IMPLEMENTADO` | 0 |
| Registros com observação preenchida | 843 |

### Distribuição por espaço

| Espaço | Registros |
|---|---:|
| Base (sem prefixo) | 256 |
| CB | 256 |
| ED | 256 |
| DD | 86 |
| FD | 86 |
| DDCB | 256 |
| FDCB | 256 |
| **Total** | **1.452** |

A distribuição é internamente coerente com a convenção do relatório: DD/FD registram somente 86 formas explicitamente tratadas em cada prefixo; combinações restantes retornam ao decoder base e não aparecem como linhas próprias nessa matriz. A tabela base também inclui quatro entradas que representam os prefixos CB/ED/DD/FD, não instruções completas isoladas.

### Comparação declarada com ares no JSON

| Valor de `vs_ares` | Registros |
|---|---:|
| `=` | 1.448 |
| `DIFERE (menor)` | 2 |
| `= (ares: NOP)` | 2 |

As duas diferenças explícitas são `OUT (n),A` (D3) e `IN A,(n)` (DB), ambas por ausência de atualização de WZ. As duas entradas ED 77/7F são classificadas como NOP nos dois lados. O JSON contém 178 NOPs ED com 8 T-states, compatíveis com o resumo narrativo de 78 formas ED explícitas e 178 formas não atribuídas tratadas como NOP.

**Limite:** consistência estrutural e correspondência com o resumo não provam que os mnemonics, flags, aliases, timings ou semântica estejam corretos. Todos os registros têm `status=IMPLEMENTADO`, inclusive os NOPs de fallback e as entradas que são apenas prefixos. Esse campo mede classificação/implementação declarada, não aprovação semântica.

## 2. Confronto com os arquivos atuais de `main`

Os blobs consultados no GitHub foram:

| Arquivo | Linhas retornadas | Blob SHA |
|---|---:|---|
| `native/sms_core/src/cpu.rs` | 3.488 | `a8d358e2c3e28064d204da96e281cd324811f1ee` |
| `native/sms_core/src/alu.rs` | 199 | `84bb9ce76c4235533b3fb4888619301c06a01216` |
| `native/sms_core/src/bus.rs` | 9 | `478a8983515ca5165b6cf10aef787cd541254391` |
| `native/sms_core/src/vdp.rs` | 1.703 | `61676594275ad3c330437f4f1ede5793af3a0211` |
| `native/z80_bus/src/lib.rs` | 356 | `c6f4ef5f52b30504209a1c5d07d2b397f9256026` |
| `native/sms_core/src/machine.rs` | 464 | `2cda762a9fc2d0f4a1bdfa513e7d59fbeaaab890` |

O relatório fornecido informa 3.487/198/8/1.702 linhas para os quatro arquivos principais. A diferença uniforme de uma linha não é, isoladamente, evidência de alteração semântica: pode decorrer da convenção de contagem da quebra de linha final. O relatório original não fixa blob SHAs dos arquivos nem um SHA de commit do ares, portanto a reprodução histórica exata da comparação ainda precisa ser formalizada.

**Observação arquitetural importante:** `native/sms_core/src/bus.rs` é uma fachada de poucas linhas; os contratos `Z80BusEvent`, `Z80TStateSample` e snapshots estão em `native/z80_bus/src/lib.rs`. Uma auditoria de barramento que leia apenas `sms_core/src/bus.rs` é incompleta.

## 3. CPU Z80 — confronto das divergências narradas

| ID | Afirmação do relatório | Verificação no código atual | Conclusão / ação |
|---|---|---|---|
| C1 | IN/OUT imediato não atualiza WZ | Confirmado: os caminhos D3/DB formam a porta, executam o I/O e retornam 11 T sem atribuir WZ | Diferença de implementação confirmada; criar vetores que verifiquem WZ e efeitos subsequentes antes de corrigir |
| C2 | IM1 não define WZ=0038 | Confirmado: `service_int`, ramo IM1, empilha PC e define PC=0038 sem escrever WZ | Diferença confirmada; ausente da matriz JSON, que não modela estados de interrupção |
| C3 | Reset zera AF/SP; ares usa FFFF | `reset()` atribui `Z80Registers::default()`; a semântica efetiva depende dos defaults do tipo | Estado inicial do SMS_CORE verificável; valor esperado de power-on não está provado pelo relatório/manual anexado. Não alterar por compatibilidade com ares isoladamente |
| C4 | R incrementa durante HALT no SMS_CORE | Confirmado: cada passo HALT incrementa os 7 bits inferiores de R e soma 4 T-states | Preservar até haver evidência específica do Z80 NMOS e do perfil de hardware |
| C5 | EX (SP),HL escreve SP antes de SP+1 | Confirmado: o caminho base escreve primeiro em SP e depois em SP+1 | Ordem funcional confirmada; a alegação de que hardware real exige a ordem inversa ainda precisa de fonte/trace de barramento independente |
| C6 | EX (SP),IX/IY escreve SP+1 antes de SP | Confirmado: o caminho indexado escreve primeiro em SP+1 e depois em SP | Inconsistência interna confirmada; não normalizar a ordem sem vetor independente que fixe a sequência correta |
| C7 | CPU agrega T-states por instrução | Confirmado pelo contrato do executor e pela integração da máquina; os eventos detalhados são limitados a instruções ED selecionadas | Lacuna temporal arquitetural confirmada; não equivale a erro de contagem nominal |

### Notas de interpretação

- O JSON registra C1, HALT, C5 e C6 em observações, mas não contém um registro próprio para C2, C3 ou C7. A matriz de opcodes não substitui uma matriz de estados especiais/interrupções.
- O teste de classificação de opcodes demonstra que o decoder aceita os bytes varridos; não demonstra semântica completa, estado final, flags, acesso ao barramento ou timing de cada combinação.
- O manual Zilog fundamenta M-cycles, T-states, M1, memória, I/O, WAIT e refresh. Ele não define o mapa de memória do Master System, a arbitragem do mapper ou os latches do VDP.
- A ordem de duas escritas de `EX (SP),HL` deve ser validada com um bus observer sensível à ordem (inclusive endereços de mapper), usando um vetor independente; a concordância com ares não basta.

## 4. Barramento e scheduler — o que a auditoria precisa separar

A implementação real dos tipos temporais está em `native/z80_bus/src/lib.rs`:

- `Z80BusEvent` é documentado como evento lógico observacional, não callback de scheduler nem sinal físico.
- `Z80TStateSample` representa amostras simplificadas de endereço/dados e RD/WR/MREQ/IORQ.
- A própria documentação restringe a instrumentação inicial a instruções ED de transferência/comparação selecionadas.
- A fase Refresh é inferida do perfil do corpus; não é uma medição física direta do pino RFSH.

Na máquina SMS, a CPU executa uma instrução e o VDP recebe avanço agregado depois da instrução. Isso confirma a lacuna de ordenação CPU↔VDP descrita no relatório. A próxima implementação deve definir proprietário único do tempo, instante/duração dos acessos, waits, prioridades determinísticas e integração de IRQ antes de migrar famílias de instruções. Não somar os T-states novamente no scheduler.

## 5. VDP — confronto dos principais pontos narrados

| ID/tema | Verificação no código atual | Estado |
|---|---|---|
| Altura 192/224/240 | `visible_height()` testa somente R1 bit 3 e bit 4; não decodifica M2/M4 | Diferença estrutural confirmada; requer matriz de 16 combinações de modo e vetores externos |
| IRQ de linha | O contador é testado ao completar uma linha visível; quando zero, marca pending e recarrega R10 | Fase do código confirmada; a comparação exata com ares/hardware requer versão e vetor independentes |
| IRQ de frame/VBlank | VBLANK é marcado quando `scanline` alcança `visible_height()` após completar a linha anterior | Fase do código confirmada; não promover a divergência de uma linha a verdade de hardware sem teste específico |
| Escrita R10 | Escrever R10 também atribui imediatamente `line_counter` | Confirmado |
| R9 | Escrita durante área visível fica em `pending_scroll_y`; o valor pendente é aplicado na transição para VBlank | Confirmado como comportamento do código; timing elétrico exato ainda não demonstrado |
| R8 | Sem latch horizontal/timestamp; renderer consome registrador corrente no fechamento da linha | Confirmado como limitação do modelo |
| VRAM/CRAM | Writes mutam os arrays imediatamente; não há fila temporal nem renderer parcial por pixel | Confirmado como limitação do modelo |
| Leitura de data port ($BE) | Limpa `cram_write` e `control_first` | Confirmado |
| Leitura de status ($BF) | Retorna status e depois zera status e `line_irq_pending` | Confirmado; os bits baixos retornados após leitura devem ser comparados com fonte SMS específica |
| H-counter | Fórmula linear aproximada baseada em T-states dentro da linha, exceto latch do Light Phaser | Confirmado como aproximação documentada no código |
| Paleta | Conversão RGB usa multiplicação linear por 85 para cada componente de 2 bits | Confirmado |
| V-counter PAL 240 | Implementação retorna D2 a partir da linha 267; há teste unitário para esse valor | Confirmado como contrato interno; não invalida sozinho o valor D3 de outro emulador |
 
A tabela narrativa enumera 16 divergências de VDP, embora o resumo diga 20. Os quatro itens restantes provavelmente estão no `vdp_audit.csv`, que não foi fornecido. Assim, não é possível reproduzir a contagem completa de 50 itens (26 iguais, 20 divergentes, 3 extensões e 1 ausente) a partir dos arquivos disponíveis.

Os itens sobre quirk SMS1, colisão de sprites sob a máscara, primeiro byte do controle, gating de IRQ e regras específicas de Graphics II devem receber testes focados e referência de versão antes de qualquer alteração. Ares e MAME são comparadores úteis, não oráculos absolutos.

## 6. Problemas de método/proveniência encontrados

1. **Ares sem commit fixado:** `master` é móvel. A auditoria deve registrar SHA, caminhos e trechos/linhas exatos do ares usados em cada comparação.
2. **Sem identificação completa da base SMS_CORE no relatório original:** registrar commit SHA e blob SHA por arquivo, em vez de apenas contagem de linhas.
3. **CSV ausentes:** sem `z80_opcodes_audit.csv` e `vdp_audit.csv`, parte da matriz não é reproduzível.
4. **Status ambíguo:** `IMPLEMENTADO` não diferencia decoder presente, semântica testada, timing nominal conferido e validação independente.
5. **T-states em texto livre:** valores como `17/10`, `12/7` e `20/23` não codificam qual condição seleciona cada duração; a tabela precisa de campo de condição/estado e referência.
6. **Escopo incompleto da matriz de opcodes:** estados de reset, NMI/INT, EI delay, HALT e WZ não cabem como linhas comuns de opcode sem uma matriz complementar.
7. **Afirmações de hardware sem evidência direta:** separar explicitamente manual/documentação, observação de emulador, inferência e medição em hardware.

## 7. Prioridades e critérios de aceitação

### P0 — tornar a auditoria reproduzível
- Anexar ou localizar os dois CSVs originais.
- Fixar SHA do SMS_CORE e SHA do ares.
- Associar cada divergência a arquivo, símbolo, trecho/linha, evidência e teste.
- Separar status de implementação, teste semântico, timing nominal e precisão temporal.

### P1 — fechar divergências CPU de baixo risco com vetores
- C1: WZ após IN/OUT imediato, incluindo wrap de byte do imediato e valor alto da porta.
- C2: WZ após IM1 e estado completo após acknowledge.
- C5/C6: ordem exata de leitura/escrita e efeitos em mapper; manter a divergência aberta até haver oráculo adequado.
- C3: documentar reset determinístico do emulador separadamente do estado elétrico de power-on do chip.

### P1 — VDP com vetores de modo e IRQ
- Cobrir todas as combinações relevantes de M1/M2/M3/M4 e bits R1, não apenas dois bits isolados.
- Testar HINT/VINT em fronteiras de linha e frame, R10 escrito antes/durante/depois da linha e leitura de status.
- Manter R8/VRAM/CRAM como limitações do modelo até existir vetor que determine o primeiro pixel visível.

### P2 — expandir a evidência temporal
- Completar cobertura por família de instrução: fetch/M1, memória, pilha, CB, ED, DD/FD, DDCB/FDCB, I/O, HALT, NMI e INT.
- Introduzir eventos que dirijam o scheduler apenas após especificar timestamp, duração, WAIT, desempate e ausência de dupla contagem.
- Manter traces de diagnóstico opcionais e sem efeitos sobre a semântica normal.

### Critérios de encerramento desta revisão
- Integridade estrutural do JSON: **PASS**.
- Confronto estático dos itens C1–C7 com código: **PASS**, com ressalvas de hardware acima.
- Confronto estático dos principais itens VDP: **PASS**, limitado ao relatório narrativo disponível.
- CSVs completos de opcode e VDP: **BLOCKED — não fornecidos/localizados**.
- Execução de testes/build nesta revisão: **NOT RUN**.
- Teste em hardware/oráculo independente: **NOT RUN**.
- `TEMPORAL_PASS`: **NÃO DECLARADO**.

## 8. Próximo passo

Depois de disponibilizar os CSVs, executar a revisão linha a linha das 50 entradas VDP e das 1.452 entradas Z80, cruzando cada registro com o decoder atual e uma referência ares fixada por SHA. Em seguida, abrir PRs pequenas de correção apenas para divergências confirmadas por vetores independentes. Esta PR não altera o runtime e não declara que a fidelidade Z80/VDP está concluída.
