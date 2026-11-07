# E7.7 — ED 71 / OUT (C),0: perfil de hardware Master System

**Data:** 2026-11-07  
**Baseline auditada:** `main` = `43323df653497a98c7afae52a6f3ea198f46cdb0`  
**Escopo:** determinar a semântica de `ED 71` para as variantes de CPU documentadas em placas Sega Master System e verificar o runtime compartilhado.  
**Decisão:** manter a saída `0x00` no perfil SMS NMOS documentado; não introduzir configuração CMOS/clones sem hardware suportado que exija essa variante.

## 1. Conclusão executiva

1. `ED 71` é uma instrução não documentada `OUT (C),0`. O comportamento documentado por referências de variantes é `0x00` em Zilog NMOS e `0xFF` em Zilog CMOS.
2. A documentação de placas SMS consultada lista CPUs Zilog `Z0840004PSC` em diversas revisões e, na revisão PAL inicial 171-5385, `NEC D780C-1` ou `Zilog Z0840004PSC`.
3. A especificação oficial ZiLOG classifica `Z08400` como NMOS e `Z84C00` como CMOS. A evidência secundária consultada caracteriza NEC D780C-1 como clone NMOS.
4. O executor Rust atual escreve `0x00` para o subopcode ED 71. O teste existente de I/O ED verifica a transação `(BC, 0x00)`.
5. Portanto, o runtime atual é compatível com o perfil NMOS indicado pelas placas SMS documentadas. **Não há divergência de runtime que justifique mudança de código nesta subquestão.**
6. Isto não é prova de que todas as unidades/revisões SMS possíveis usem exclusivamente essas peças, nem valida todos os clones. A exceção Sharp LH5080A impede generalizar simplesmente CMOS → FF.
7. E7.7 global continua IN_PROGRESS por outras famílias/efeitos semânticos ainda não reconciliados. E8 continua BLOCKED.

## 2. Frente 1 — engenharia reversa e comportamento observado

### 2.1 redcode/Z80 — evidência comparativa de variante

A página de bugs do projeto redcode documenta que `OUT (C),0` em CPUs Zilog CMOS se comporta como `OUT (C),255`. A página registra a descoberta original por Simon Cooke (1996), a redescoberta por Colin Piggot (2004) e a confirmação/rediscovery da comunidade MSX (2008).

Fonte: https://github.com/redcode/Z80/wiki/Bugs

**Classificação:** referência técnica comparativa que aponta para relatos/testes históricos; não é uma medição executada nesta auditoria.

### 2.2 Testes de identificação de CPU

O Z80 CPU Tester e projetos de identificação de variantes usam ED 71 para distinguir parte dos modelos: saída zero é compatível com Zilog NMOS; FF é compatível com Zilog CMOS. As próprias fontes registram exceções e variação entre espécimes. A Sharp LH5080A, embora CMOS, é relatada como emitindo zero como os NMOS nesse opcode.

Fontes:
- https://www.malinov.com/sergeys-blog/z80-type-detection.html
- https://github.com/Megatokio/z80-tests-zx
- https://www.8bit-museum.de/

**Limitação:** testes de identificação são evidência comportamental útil, mas não devem ser convertidos numa regra universal baseada apenas na tecnologia nominal do processo. Alguns clones têm comportamento específico.

### 2.3 Implementações de referência

MSX Wiki e CPCWiki também descrevem ED 71 como saída zero em Z80 NMOS e FF em CMOS. A documentação do Z88 registra o caso CMOS com FF e explicita que o Z80 NMOS emitiria zero.

Fontes:
- https://www.msx.org/wiki/Z80_Opcodes
- https://www.cpcwiki.eu/index.php/Z80
- https://cambridgez88.jira.com/wiki/spaces/DN/pages/9568467/The+Zilog+Z80+undocumented+instructions

Essas referências são corroborativas, não substituem a documentação da peça efetivamente montada na placa nem a medição do hardware SMS.

## 3. Frente 2 — documentação primária e hardware SMS

### 3.1 ZiLOG: identificação de famílias

A especificação oficial `Z8400/84C00 NMOS/CMOS Z80 CPU` separa explicitamente:
- `Z08400`: versão NMOS;
- `Z84C00`: versão CMOS.

Fonte oficial: https://www.zilog.com/docs/z80/z8400.pdf

A especificação de produto não documenta diretamente o resultado de ED 71. Ela estabelece a distinção entre as famílias de componentes, enquanto a semântica não documentada de ED 71 é corroborada pelas fontes da seção 2.

### 3.2 SMS Power: componentes das placas

As páginas de hardware consultadas identificam:

| Revisão/placa | CPU listada | Interpretação |
|---|---|---|
| PAL M4 POWER BASE 837-6097, board 171-5385 | NEC D780C-1 ou Zilog Z0840004PSC | Clones/peças NMOS documentados |
| NTSC M4 POWERBASE 837-6405, board 171-5532 | Zilog Z0840004PSC | Zilog NMOS |
| PAL VA2 837-6424, board 171-5672 | Zilog Z0840004PSC | Zilog NMOS |
| NTSC 837-6629, board 171-5538 | Zilog Z0840004PSC | Zilog NMOS |
| PAL Master System II M4Jr, board 171-5922A | Zilog Z0840004PSC | Zilog NMOS |

Fontes SMS Power:
- https://www.smspower.org/Development/PALMasterSystemM4POWERBASEPAL837-6097
- https://www.smspower.org/Development/NTSCMasterSystemM4POWERBASENTSC837-6405
- https://www.smspower.org/Development/PALMasterSystemPOWERBASEVA2PAL837-6424
- https://www.smspower.org/Development/NTSCMasterSystemM4POWERBASEPAL837-6629
- https://www.smspower.org/Development/PALMasterSystemIIICBDM4JrPAL

**Escopo da conclusão:** as revisões listadas acima, não um inventário exaustivo de cada placa, substituição de componente ou clone regional. A página da 171-5385 identifica NEC D780C-1 ou Zilog Z0840004PSC; não prova qual das duas peças está em cada unidade individual.

### 3.3 Consequência para o perfil SMS

As placas consultadas não listam Zilog Z84C00 CMOS. As CPUs listadas são Z08400 NMOS ou NEC D780C-1. Logo, para o perfil de hardware SMS documentado nesta amostra, a saída zero é a escolha suportada pelas evidências disponíveis.

A validação direta de ED 71 em consoles físicos SMS não foi executada nesta auditoria. A classificação é documental/por identificação de componente, não uma medição de cada placa.

## 4. Frente 3 — auditoria do runtime e decisão arquitetural

### 4.1 Código integrado

Arquivo: `native/z80_cpu/src/cpu.rs`.

O decoder ED usa a classe de opcode de OUT (C),r. Para o código de registrador 6 (ED 71), a implementação seleciona explicitamente o valor `0`; para os demais códigos, lê o registrador selecionado. A escrita usa o endereço BC e preserva F.

O teste existente no módulo de CPU percorre as codificações OUT:
- ED 41, ED 49, ED 51, ED 59, ED 61, ED 69, ED 71 e ED 79;
- espera `0x00` especificamente para ED 71;
- verifica endereço de porta BC e flags preservadas.

### 4.2 Ausência de perfil configurável

Na estrutura de CPU examinada não foi identificado um seletor explícito de variante NMOS/CMOS para ED 71. O comportamento atual é fixo em zero.

Isso é aceitável para os alvos SMS documentados nesta auditoria. Não se deve adicionar um enum genérico de variantes somente para cobrir uma possibilidade de hardware que não aparece nas placas SMS listadas e não é exigida por um console atualmente suportado.

Se uma futura plataforma suportada identificar uma CPU CMOS que exija ED 71 = FF, a decisão deverá ser reaberta com:
- identificação do componente/placa;
- evidência de comportamento do opcode na peça;
- escopo de compatibilidade e testes específicos;
- seleção do perfil no nível de configuração do hardware, não em lógica da GUI.

### 4.3 ADR-0016

ED 71 é uma transação de I/O observável. O requisito atual é o valor correto escrito no porto BC e a ordem causal da transação. A subquestão não demonstra necessidade de adicionar microfases ou eventos temporais novos: o caminho existente já representa a escrita de porta e o teste verifica o valor.

## 5. Matriz de evidência

| Questão | Resultado |
|---|---|
| ZiLOG Z08400 classificado como NMOS | PASS — especificação oficial |
| Z84C00 classificado como CMOS | PASS — especificação oficial |
| ED 71 = 00 em Zilog NMOS | Corroborado por referências técnicas/relatos de teste |
| ED 71 = FF em Zilog CMOS | Corroborado por redcode, MSX Wiki, CPCWiki e documentação Z88 |
| Exceções em clones | PASS — fontes identificam Sharp LH5080A como exceção reportada |
| Placas SMS amostradas usam Z08400 ou NEC D780C-1 | PASS — páginas de hardware SMS Power consultadas |
| ED 71 runtime emite zero | PASS — inspeção estática do código integrado |
| Teste de runtime verifica ED 71 = zero | PASS — teste presente no código; execução específica não foi feita nesta auditoria |
| Teste em hardware físico SMS | NOT RUN |
| CMOS/clones em geral | NOT VERIFIED |
| Alteração de runtime necessária | NO — não identificada divergência para o perfil SMS documentado |

## 6. Decisão e próximos passos

- Classificar ED 71 como **RESOLVED_FOR_DOCUMENTED_SMS_NMOS_PROFILE**.
- Manter ED 71 = 0 no runtime compartilhado atual.
- Não adicionar variante CMOS ou lógica de detecção de CPU sem um alvo suportado que precise dela.
- Não afirmar comportamento universal para todos os clones ou todas as placas SMS.
- Manter E7.7 global como **IN_PROGRESS**. A auditoria integral de todas as famílias/efeitos semânticos ainda não foi encerrada.
- Manter E8 como **BLOCKED** até o fechamento formal de E7.
- Não executar ZEXDOC/ZEXALL nesta etapa.

**Resultado desta auditoria:** evidência suficiente para a escolha ED 71 = 0 no perfil de placas SMS documentadas; nenhuma alteração de runtime justificada.  
