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

### Candidato corretivo com vetores ampliados

SHA validado: `6f0fd7d81b1536bbfb5f8e36825591a2362711ae`.

- **PASS:** `cargo fmt --all -- --check` via workflow Rust Formatting.
- **PASS:** Workspace Clippy.
- **PASS:** Workspace Rust tests.
- **PASS:** SMS focused format, lint and tests.
- **PASS:** workflow geral CI/quality.
- A suíte de referência SingleStepTests passou com **57 testes aprovados e 1 ignorado**. O caso ignorado continua sendo `ED B1` (CPIR), devido a divergência de flags não documentadas ainda em investigação.
- Foram acrescentados 10 vetores independentes para `DI`, `EI`, `IM 0/1/2`, `LD I,A`, `LD R,A`, `LD A,I`, `LD A,R` e `RETI`.
- **SKIPPED:** revisão automática Sourcery, por limite de orçamento do serviço; isso não é aprovação nem falha de código.
- **NOT PROVEN:** completude semântica do Z80 e precisão temporal ciclo a ciclo. Os vetores verificam estado arquitetural e T-states agregados, não trace temporal completo do barramento.

## Critérios para continuar a mescla

1. Reexecutar todos os workflows no SHA corretivo atual.
2. Comparar as rotinas de interrupção, refresh R, prefixos repetidos, indexação e flags internas com vetores independentes; não inferir correção pela mera existência de um decoder.
3. Expandir vetores por classes de opcode e estados de fronteira, incluindo wraparound, flags, efeitos de memória, interrupções e estados internos documentados pelo oráculo.
4. Registrar cada divergência com entrada reproduzível, saída observada, saída esperada e fonte.
5. Não declarar precisão de barramento ciclo a ciclo sem um modelo e testes de eventos ordenados/timestamped.

## Conclusão

A maior parte do trabalho do SG-1000 já está presente no SMS. A mescla correta é uma consolidação auditada, não uma substituição integral do arquivo. As diferenças comprovadamente específicas do SMS devem permanecer até que um teste independente justifique alterá-las.
