# Auditoria de consolidação dos núcleos Z80 SG-1000 e SMS

## Objetivo

Reutilizar no núcleo SMS o trabalho já realizado no núcleo SG-1000 sem perder correções específicas do SMS nem propagar divergências sem validação independente.

## Baseline inspecionada

- Base principal no início da auditoria: `315d665d4b8295fc5427976249db136c587ae37b`.
- SG-1000 `native/sg1000_core/src/cpu.rs`: blob `a3cfed55db611a84e78f714bb3e985b64cf72ae5`.
- SMS `native/sms_core/src/cpu.rs`: blob `9c272a023ffc8f3ae6110be329a46d5af707f60e`.
- ALU SG-1000: blob `ec3bb672d24525cfb4eebfec743a62690d580947`.
- ALU SMS: blob `84bb9ce76c4235533b3fb4888619301c06a01216`.

Esses hashes identificam os arquivos comparados, não afirmam que os testes desses blobs passaram.

## Resultado da comparação

### Já compartilhado

Os dois núcleos já contêm a mesma estrutura principal do CPU, ALU, tratamento de interrupções, prefixos e a maior parte dos testes unitários. A ALU tem diferenças textuais de comentário, mas não foi encontrada divergência funcional na comparação direta do arquivo.

Os testes unitários do SMS já cobrem as áreas que aparecem no conjunto do SG-1000; além disso, o SMS tem um teste que percorre todas as codificações `DD CB` e `FD CB` para verificar que não retornam erro. Isso é cobertura de classificação/execução, não prova semântica correta para todos os 512 casos.

### Diferenças funcionais que exigem tratamento explícito

1. **WZ/MEMPTR no SMS:** o SMS possui atualizações de WZ em operações de memória, saltos, aritmética e instruções BIT que não aparecem da mesma forma no SG-1000. Preservar essas mudanças durante a consolidação.
2. **Latch Q em `EX AF,AF'`:** o SG-1000 atualiza Q com o novo F após a troca; o SMS não faz isso. Não portar essa diferença sem prova: o vetor independente `SingleStepTests 08 0000` espera Q=0 após a instrução, embora F passe a 0x47. No candidato inicial, adicionar a atribuição SG-1000 fez o vetor falhar (Q observado 71, esperado 0). A atribuição foi removida.
3. **`DAA`:** o SG-1000 só aplica a condição `A > 0x99` no caminho de adição; o SMS aplica a correção quando C está definido ou A>0x99, inclusive no caminho N=1. O vetor independente `SingleStepTests 27 0000` inicia com A=0xF3, F=0x4E e espera A=0x93. A condição restrita portada inicialmente do SG-1000 deixou A=0xF3 e falhou. A lógica SMS foi restaurada.

As duas diferenças rejeitadas são evidência de que copiar código do SG-1000 sem comparar com o oráculo pode introduzir regressões.

## Validação observada

### SHA inicial com diferenças SG-1000 portadas sem auditoria

SHA `5184db70345bfcf9356bc16723893b2448774f37`.

- **PASS:** Rust formatting, Workspace Clippy e workflow de qualidade.
- **FAIL:** testes do workspace e validação focada SMS por duas divergências nos vetores independentes: `08 0000` (Q) e `27 0000` (DAA).
- Resultado: as duas alterações comportamentais foram revertidas; os vetores foram preservados como regressões.

### Consolidação e expansão de vetores

PR #160 foi integrada em `84dc09e0ba7b14153ad5f5b26809a072cb116faa`; PR #161 foi integrada em `160eba417b7852a81c89aaed260d137117da675b`.

No candidato da PR #160, SHA `6f0fd7d81b1536bbfb5f8e36825591a2362711ae`:
- **PASS:** Rust formatting, Workspace Clippy, Workspace Rust tests, SMS focused format/lint/tests e workflow geral CI/quality.
- A suíte de referência SingleStepTests passou com **57 testes aprovados e 1 ignorado**.
- Foram acrescentados 10 vetores para `DI`, `EI`, `IM 0/1/2`, `LD I,A`, `LD R,A`, `LD A,I`, `LD A,R` e `RETI`.

A PR #161 adicionou um helper que compara também os bytes finais de RAM e nove vetores de escrita/leitura de memória, pilha, troca em `(SP)`, `LDI` e store indexado. Eles revelaram duas falhas reais de WZ/MEMPTR no SMS:
1. **`LD (nn),A` (`0x32`):** WZ deve combinar A no byte alto com o byte baixo do endereço seguinte. O valor anterior era apenas o endereço incrementado.
2. **`EX (SP),HL` (`0xE3`):** WZ deve receber a palavra carregada da pilha e trocada com HL; a implementação deixava o WZ anterior.

As duas correções foram aplicadas e os vetores passaram nos workflows da PR #161 antes da integração.

Na PR #162, SHA candidato `3d0f5ee0197d9481104740c90c4889fd67f2cd8f`:
- **PASS:** Rust formatting, Workspace Clippy, Workspace Rust tests, SMS focused format/lint/tests e workflow geral CI/quality.
- A suíte de referência passou com **76 testes aprovados e 1 ignorado**.
- Foram acrescentados 10 vetores para `LD (BC),A`, `LD A,(BC)`, `LD (DE),A`, `LD A,(DE)`, `ADD HL,DE`, `LD A,(nn)`, `JP (HL)`, `LD SP,HL`, `EX DE,HL` e `DJNZ`.

Na PR #163, SHA candidato `85384bd67b76ed364b7ecb4861ae6120b3adf36e`:
- **PASS:** Rust formatting, Workspace Clippy, Workspace Rust tests, SMS focused format/lint/tests e workflow geral CI/quality.
- A suíte de referência passou com **86 testes aprovados e 1 ignorado**.
- Foram acrescentados 10 vetores para `RST 00`, `JP NZ,nn`, `CALL NZ,nn`, `RET NZ`, `POP AF`, `PUSH AF`, `EXX`, `JR C` não tomado, `BIT 0,(HL)` e `RES 0,(HL)`.
- Os vetores identificaram e orientaram quatro correções adicionais:
  1. `JP cc,nn`: WZ recebe o alvo independentemente do resultado da condição.
  2. `CALL cc,nn`: WZ recebe o alvo mesmo quando a chamada não ocorre.
  3. `RST`: WZ recebe o endereço do vetor.
  4. `POP AF`: a carga de F não deve atualizar o latch Q como se flags fossem calculadas.
- **SKIPPED:** revisão automática Sourcery, por limite de orçamento do serviço; isso não é aprovação nem falha de código.

Na PR #164, SHA candidato `afcc1264ce722fa7d142d90ee73b09457c78c618`:
- **PASS:** Rust formatting, Workspace Clippy, Workspace Rust tests, SMS focused format/lint/tests e workflow geral CI/quality.
- A suíte SingleStepTests passou com **89 testes aprovados e nenhum ignorado**; o vetor CPIR que estava ignorado foi reativado.
- O vetor `ED B1 0000` mostrou que, em uma iteração repetida de CPIR, as flags X/Y vêm do byte alto do PC depois de recuar dois bytes, não do resultado ajustado da comparação. A correção foi generalizada para LDIR/LDDR, CPIR/CPDR e instruções de bloco de I/O repetidas.
- O vetor `ED B0 0001` valida a mesma regra em LDIR. Os vetores `ED B2 0002` e `ED BA 0000` mostraram que flags de bloco de I/O repetido também exigem ajuste de PV/H durante os T-states adicionais; a implementação agora aplica a regra condicional ao carry e ao bit 7 do byte transferido. O vetor `ED B2 0002` passou após corrigir a fórmula base de PV para usar o B pós-decremento e, em seguida, aplicar o ajuste da repetição.
- Evidência externa consultada: o manual independente *The Undocumented Z80 Documented* descreve os flags de CPI/CPIR e os efeitos de instruções de bloco; o wiki do Z80Decoder registra efeitos de flags não documentadas em operações de bloco interrompidas. O vetor SingleStepTests fixado fornece a expectativa executável usada pelos testes: https://github.com/SingleStepTests/z80/tree/ebe1875d48f374bcfd4b505d8eb8ee751568b5f7.
- **SKIPPED:** revisão automática Sourcery, por limite de orçamento do serviço.

Na PR #165, SHA candidato `57d4f78c30afca019de94789c24716ad17bf7744`:
- **PASS:** Rust formatting, Workspace Clippy, Workspace Rust tests, SMS focused format/lint/tests e workflow geral CI/quality.
- A suíte SingleStepTests passou com **94 testes aprovados e nenhum ignorado**.
- Foram adicionados vetores para `LDDR`, `CPDR`, `INDR`, `OTIR` e `OTDR`; o harness compara também portas de entrada/saída.
- Os vetores `INDR` e `OTDR` revelaram que, nas repetições de bloco de I/O, não basta sobrescrever X/Y: PV/H também sofrem efeitos dos T-states extras. A correção agora aplica a fórmula dependente de carry e do bit 7 do byte transferido; todos os cinco vetores passaram.
- Fonte independente complementar para esses efeitos não documentados: https://www.reddit.com/r/EmuDev/comments/1vkkk21/z80_proper_handling_of_undocumented_x_and_y_flags/ e https://github.com/hoglet67/Z80Decoder/wiki/Undocumented-Flags. Os detalhes de hardware ainda devem ser distinguidos de comportamento documentado pelo fabricante.

Esses resultados confirmam os checks nos SHAs candidatos informados, não a completude total da CPU. A suite verifica estado arquitetural, T-states agregados e os endereços de RAM explicitamente listados pelos vetores; não compara trace temporal completo do barramento.

## Critérios para continuar a mescla

1. Reexecutar todos os workflows no SHA corretivo atual.
2. Comparar as rotinas de interrupção, refresh R, prefixos repetidos, indexação e flags internas com vetores independentes; não inferir correção pela mera existência de um decoder.
3. Expandir vetores por classes de opcode e estados de fronteira, incluindo wraparound, flags, efeitos de memória, interrupções e estados internos documentados pelo oráculo.
4. Registrar cada divergência com entrada reproduzível, saída observada, saída esperada e fonte.
5. Não declarar precisão de barramento ciclo a ciclo sem um modelo e testes de eventos ordenados/timestamped.

## Conclusão

A maior parte do trabalho do SG-1000 já está presente no SMS. A mescla correta é uma consolidação auditada, não uma substituição integral do arquivo. As diferenças comprovadamente específicas do SMS devem permanecer até que um teste independente justifique alterá-las.
