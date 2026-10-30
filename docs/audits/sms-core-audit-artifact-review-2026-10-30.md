# Revisão minuciosa dos artefatos de auditoria SMS_CORE — 2026-10-30

## Estado, escopo e proveniência

**Tipo:** auditoria estática dos artefatos fornecidos e confronto com o código remoto.  
**Base de código consultada:** branch `main`; SHA-base da PR #273: `151eae75c8851aeb9cdbc056d99cfa093013e46c`. A PR #271 estava baseada no SHA anterior `756666ae6216d8466f40259c11c345bbc1921848`; `main` avançou seis commits antes da criação desta revisão.  
**Runtime alterado:** não.  
**Build/testes executados nesta revisão:** não; as conclusões abaixo são inspeção de fontes e validação estrutural do JSON, não um `QUICK_PASS` do runtime.

### Artefatos recebidos e disponibilidade

| Artefato | Disponibilidade | Observação |
|---|---|---|
| `AUDITORIA_SMS_CORE.md` | Disponível, 6.543 bytes | Relatório narrativo de CPU Z80 e VDP |
| `z80_opcodes_audit.json` | Disponível, 251.732 bytes | 1.452 registros de opcode/prefixo |
| `z80_opcodes_audit.csv` | Recebido em 2026-10-30 | 1.452 registros, 7 colunas; validado estruturalmente |
| `vdp_audit.csv` | Recebido em 2026-10-30 | 50 registros, 6 colunas; revisados individualmente |
| Manuais Zilog em PDF | Anexados na conversa | Referências para arquitetura/timing do Z80; não especificam os latches do VDP Sega |

Os dois CSVs originais foram recebidos posteriormente e incorporados nesta revisão. A matriz VDP de 50 registros foi conferida linha a linha; a matriz Z80 foi validada estruturalmente e comparada com as classificações e observações declaradas. Isso não equivale a executar cada opcode contra um oráculo semântico independente.

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
| `native/sms_core/src/cpu.rs` | 3.487 | `a8d358e2c3e28064d204da96e281cd324811f1ee` |
| `native/sms_core/src/alu.rs` | 198 | `84bb9ce76c4235533b3fb4888619301c06a01216` |
| `native/sms_core/src/bus.rs` | 8 | `478a8983515ca5165b6cf10aef787cd541254391` |
| `native/sms_core/src/vdp.rs` | 1.702 | `61676594275ad3c330437f4f1ede5793af3a0211` |
| `native/z80_bus/src/lib.rs` | 355 | `c6f4ef5f52b30504209a1c5d07d2b397f9256026` |
| `native/sms_core/src/machine.rs` | 463 | `2cda762a9fc2d0f4a1bdfa513e7d59fbeaaab890` |

A contagem normalizada de linhas reais confirma exatamente os valores do relatório para os quatro arquivos principais: 3.487/198/8/1.702. O relatório original não fixa blob SHAs dos arquivos nem um SHA de commit do ares, portanto a reprodução histórica exata da comparação ainda precisa ser formalizada.

**Observação arquitetural importante:** `native/sms_core/src/bus.rs` é uma fachada de poucas linhas; os contratos `Z80BusEvent`, `Z80TStateSample` e snapshots estão em `native/z80_bus/src/lib.rs`. Uma auditoria de barramento que leia apenas `sms_core/src/bus.rs` é incompleta.

## 3. CPU Z80 — confronto das divergências narradas

| ID | Afirmação do relatório | Verificação no código atual | Conclusão / ação |
|---|---|---|---|
| C1 | IN/OUT imediato não atualiza WZ | Confirmado: os caminhos D3/DB formam a porta, executam o I/O e retornam 11 T sem atribuir WZ | Diferença de implementação confirmada; criar vetores que verifiquem WZ e efeitos subsequentes antes de corrigir |
| C2 | IM1 não define WZ=0038 | Confirmado: `service_int`, ramo IM1, empilha PC e define PC=0038 sem escrever WZ | Diferença confirmada; ausente da matriz JSON, que não modela estados de interrupção |
| C3 | Reset zera AF/SP; ares usa FFFF | `reset()` atribui `Z80Registers::default()`; a semântica efetiva depende dos defaults do tipo | Estado inicial do SMS_CORE verificável; valor esperado de power-on não está provado pelo relatório/manual anexado. Não alterar por compatibilidade com ares isoladamente |
| C4 | R incrementa durante HALT no SMS_CORE | Confirmado: cada passo HALT incrementa os 7 bits inferiores de R e soma 4 T-states | Preservar até haver evidência específica do Z80 NMOS e do perfil de hardware |
| C5 | EX (SP),HL escreve SP antes de SP+1 | Confirmado: o caminho base escreve primeiro em SP e depois em SP+1 | O manual Zilog UM008011-0816, p. 127, descreve H ↔ (SP+1) e L ↔ SP, mas não fornece a ordem cronológica dos dois ciclos de escrita. A divergência é plausível, ainda requer trace/vetor de barramento independente |
| C6 | EX (SP),IX/IY escreve SP+1 antes de SP | Confirmado: o caminho indexado escreve primeiro em SP+1 e depois em SP | A notação do manual para IX (p. 128) também lista IXH ↔ (SP+1) antes de IXL ↔ SP; isso é compatível com a ordem implementada, mas não substitui um trace temporal dos ciclos. A inconsistência entre caminhos base/indexado permanece confirmada |
| C7 | CPU agrega T-states por instrução | Confirmado pelo contrato do executor e pela integração da máquina; os eventos detalhados são limitados a instruções ED selecionadas | Lacuna temporal arquitetural confirmada; não equivale a erro de contagem nominal |

### Notas de interpretação

- O JSON registra C1, HALT, C5 e C6 em observações, mas não contém um registro próprio para C2, C3 ou C7. A matriz de opcodes não substitui uma matriz de estados especiais/interrupções.
- O teste de classificação de opcodes demonstra que o decoder aceita os bytes varridos; não demonstra semântica completa, estado final, flags, acesso ao barramento ou timing de cada combinação.
- O manual Zilog UM008011-0816 (08/2016), pp. 127–128, descreve a operação de EX (SP),HL/IX e a duração agregada em M-cycles/T-states. A notação dos operandos não especifica, por si só, a cronologia elétrica de cada write. O manual fundamenta M-cycles, T-states, M1, memória, I/O, WAIT e refresh; não define o mapa de memória do Master System, a arbitragem do mapper ou os latches do VDP.
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
3. **Proveniência dos CSVs:** os arquivos foram recebidos em 2026-10-30; o relatório de origem ainda não fixa hashes dos CSVs nem o SHA exato do ares usado na comparação.
4. **Status ambíguo:** `IMPLEMENTADO` não diferencia decoder presente, semântica testada, timing nominal conferido e validação independente.
5. **T-states em texto livre:** valores como `17/10`, `12/7` e `20/23` não codificam qual condição seleciona cada duração; a tabela precisa de campo de condição/estado e referência.
6. **Escopo incompleto da matriz de opcodes:** estados de reset, NMI/INT, EI delay, HALT e WZ não cabem como linhas comuns de opcode sem uma matriz complementar.
7. **Afirmações de hardware sem evidência direta:** separar explicitamente manual/documentação, observação de emulador, inferência e medição em hardware.

## 7. Prioridades e critérios de aceitação

### P0 — tornar a auditoria reproduzível
- [x] Receber e validar os dois CSVs originais.
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
- Confronto registro a registro da matriz VDP (50 itens): **PASS**, com divergências e extensões classificadas.
- Integridade estrutural da matriz Z80 CSV (1.452 itens): **PASS**; semântica individual dos 1.452 opcodes: **NOT RUN**.
- Execução de testes/build nesta revisão: **NOT RUN**.
- Teste em hardware/oráculo independente: **NOT RUN**.
- `TEMPORAL_PASS`: **NÃO DECLARADO**.

## 8. Próximo passo

Os CSVs agora estão disponíveis. O próximo passo é fixar SHA do ares e cruzar as 1.452 entradas Z80 com o decoder e as 50 entradas VDP com os caminhos de produção, convertendo divergências priorizadas em vetores independentes. Esta PR não altera o runtime e não declara que a fidelidade Z80/VDP está concluída.


## 9. Revisão dos CSVs recebidos em 2026-10-30

### 9.1 Z80 — CSV de 1.452 registros

- Dimensão: 1.452 linhas × 7 colunas (`prefix`, `opcode`, `mnemonic`, `tstates_impl`, `status`, `vs_ares`, `notes`).
- Todos os 1.452 registros usam `status=IMPLEMENTADO`; esse rótulo não expressa cobertura de teste.
- Distribuição de prefixos: base 256, CB 256, ED 256, DD 86, FD 86, DDCB 256, FDCB 256.
- `vs_ares`: 1.448 `=`, duas `DIFERE (menor)` e duas `= (ares: NOP)`.
- 843 linhas têm observações.
- As duas divergências explícitas são base D3 `OUT (n),A` e DB `IN A,(n)`, ambas por WZ não atualizado no SMS_CORE.
- A matriz registra duas entradas ED (77/7F) como NOP no ares e no SMS_CORE.

**Ressalva de cobertura:** 1.452 linhas não são 1.452 instruções semanticamente independentes. DD/FD só enumeram 86 formas específicas; DDCB/FDCB cobrem espaços próprios; a base inclui prefixos. A matriz também não codifica NMI/INT, EI delay, HALT, reset e estado WZ como cenários de máquina. O CSV foi verificado quanto à estrutura e aos dados declarados; não foi executado um teste semântico independente para cada linha.

### 9.2 VDP — matriz completa de 50 itens

| # | Área | Item | Resultado declarado | Severidade |
|---:|---|---|---|---|
| 1 | Porta | $BE leitura (dados) | DIVERGE | média |
| 2 | Porta | $BE escrita (dados) | IGUAL | — |
| 3 | Porta | $BF leitura (status) | DIVERGE (bits 4..0: 0x00 vs 0x1F) | baixa |
| 4 | Porta | $BF escrita, 1º byte | DIVERGE | média |
| 5 | Porta | $BF escrita, 2º byte | IGUAL (exceto 1º byte) | — |
| 6 | Porta | Espelhamento de portas | IGUAL em efeito | — |
| 7 | Comando | code 0 (leitura VRAM) | IGUAL | — |
| 8 | Comando | code 1 (escrita VRAM) | IGUAL | — |
| 9 | Comando | code 2 (escrita registrador) | IGUAL | — |
| 10 | Comando | code 3 (escrita CRAM) | IGUAL | — |
| 11 | Registrador | R0 bit0 (sync externo) | AUSENTE (sem efeito visual) | baixa |
| 12 | Registrador | R0 bit1 (M2) | DIVERGE | alta |
| 13 | Registrador | R0 bit2 (M4) | IGUAL | — |
| 14 | Registrador | R0 bit3 (sprite shift -8) | IGUAL | — |
| 15 | Registrador | R0 bit4 (IRQ de linha) | DIVERGE | média |
| 16 | Registrador | R0 bit5 (máscara coluna esq.) | DIVERGE | média |
| 17 | Registrador | R0 bit6 (trava H-scroll linhas 0-15) | IGUAL | — |
| 18 | Registrador | R0 bit7 (trava V-scroll colunas 24-31) | DIVERGE (SMS_CORE segue doc de hardware) | baixa |
| 19 | Registrador | R1 bit0/1 (zoom/tamanho sprite) | IGUAL | — |
| 20 | Registrador | R1 bit3 (M3) / bit4 (M1) | DIVERGE | alta |
| 21 | Registrador | R1 bit5 (IRQ de frame) | DIVERGE | média |
| 22 | Registrador | R1 bit6 (display) | DIVERGE | baixa |
| 23 | Registrador | R2 (name table) | IGUAL (sem quirk SMS1) | baixa |
| 24 | Registrador | R3/R4/R6 (TMS e sprite pattern) | IGUAL (Graphics II difere, ver abaixo) | — |
| 25 | Registrador | R5 (SAT) | IGUAL | — |
| 26 | Registrador | R7 (backdrop) | IGUAL | — |
| 27 | Registrador | R8 (scroll X) | DIVERGE (escrita no meio da linha) | baixa |
| 28 | Registrador | R9 (scroll Y) | DIVERGE | média |
| 29 | Registrador | R10 (contador de linha) | DIVERGE | alta |
| 30 | Temporização | Linha | DIVERGE (modelo) | — |
| 31 | Temporização | Total de linhas | IGUAL | — |
| 32 | Temporização | IRQ de frame (VBLANK) | DIVERGE (SMS_CORE 1 linha/228 T antes) | alta |
| 33 | Temporização | IRQ de linha | DIVERGE (SMS_CORE 1 linha depois) | alta |
| 34 | Contador | V-counter NTSC 192/224 | IGUAL | — |
| 35 | Contador | V-counter PAL 192/224 | IGUAL | — |
| 36 | Contador | V-counter PAL 240 | DIVERGE (ares fora por 1) | baixa |
| 37 | Contador | H-counter | DIVERGE | baixa |
| 38 | Vídeo | Mode 4 BG (tile 32B, flip, paleta, prioridade) | IGUAL | — |
| 39 | Vídeo | Wrap vertical 192 linhas | IGUAL | — |
| 40 | Vídeo | Pixels iniciais do fine-scroll | IGUAL | — |
| 41 | Sprites | Mode 4 – 64 sprites, 8/linha, Y>=F0 negativo, D0 só em 192 | IGUAL | — |
| 42 | Sprites | Overflow (flag + índice do 9º) | IGUAL | — |
| 43 | Sprites | Modo 'unlocked' (sem limite) | EXTENSÃO do SMS_CORE | — |
| 44 | Sprites | Colisão | IGUAL (exceto R0 bit5, R1 bit6) | — |
| 45 | Legado | Graphics I | IGUAL | — |
| 46 | Legado | Graphics II | DIVERGE | baixa |
| 47 | Legado | Text e Multicolor | EXTENSÃO do SMS_CORE | — |
| 48 | Legado | Sprites TMS (4/linha) | IGUAL | — |
| 49 | Cor | CRAM 6 bits → RGB | DIVERGE (apresentação) | baixa |
| 50 | Extras | Light Phaser, SegaScope 3D, anaglyph/side-by-side | EXTENSÃO | — |

A contagem literal de resultados é: 21 `IGUAL`, 12 `DIVERGE`, 2 extensões rotuladas `EXTENSÃO do SMS_CORE`, 1 extensão rotulada `EXTENSÃO`, e 14 classificações qualificadas (por exemplo, divergência específica ou igualdade com ressalva). Portanto, não se deve resumir esta coluna como "20 divergências" sem explicitar a regra de agrupamento.

### 9.3 R10 — reavaliação após inspeção do glue CPU↔VDP

A linha 29 identifica R10 como divergência alta: a escrita em R10 altera `registers[10]` e, no código original, também atribui imediatamente `line_counter = first`. Já `complete_scanline()` recarrega o contador no underflow com o valor atual de R10; o wrap do quadro também recarrega R10.

Em `SmsMachine::step()`, a sequência é: executar uma instrução Z80, avançar o VDP pelo total nominal de T-states da instrução, então consultar `vdp.interrupt_pending()` e atualizar `cpu.request_int(0xFF)`. Assim, a CPU só começa a aceitar/servir o IRQ em um passo posterior. Isso torna plausível que um handler altere R10 depois que o VDP já recarregou o contador no underflow.

**Hipótese a testar:** uma escrita em R10 durante o handler deve alterar o valor da próxima recarga, sem reiniciar o countdown que já está em andamento. Esta hipótese foi promovida a teste de regressão na PR #274. A correção proposta remove somente a atribuição imediata em `write_control`; as recargas no underflow e no wrap do quadro permanecem. O resultado ainda depende do CI e de teste com ROM/trace; não é prova de comportamento físico.

### 9.4 Contagens e limites desta revisão

- VDP: 50/50 linhas lidas; todas as áreas e severidades preservadas.
- Z80: 1.452/1.452 linhas lidas; estrutura, distribuição e divergências declaradas verificadas.
- Ares: SHA de referência ainda não fixado; os valores `vs_ares` continuam sendo comparações declaradas no CSV, não reproduções independentes.
- ROM de paralaxe: não executada; nenhum nome de jogo foi fornecido.
- Build/testes locais desta revisão: NOT RUN. Os workflows da PR #274 são a evidência de CI aplicável ao código alterado.
