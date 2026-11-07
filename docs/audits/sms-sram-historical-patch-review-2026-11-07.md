# SMS — revisão técnica dos patches históricos de SRAM

**Data:** 2026-11-07  
**Baseline histórica de comparação:** `main` = `3ca0f648c841bfe7c85a301fb561273e5c62f676` (não é o HEAD atual).  
**PR de acompanhamento:** #616  
**Escopo:** revisão dos patches de #424 e #543, confrontados com o runtime atual e com a documentação primária SMS Power.  
**Resultado:** revisão estrutural e de semântica do mapeamento; execução local de testes NOT RUN.

## 1. Commits funcionais identificados

A consulta às listas completas de commits corrigiu a interpretação dos SHAs finais:

- PR #424: os commits funcionais começam em `588b326998e2b372a46d7073b6adefb4cb0314b8` e incluem a implementação de `SmsMemory`, persistência, integração convencional/temporal e adaptadores. O SHA final `a69356f0797518fc9036d01555f3b79aa32cf453` é formatação, não o commit que introduziu SRAM.
- PR #543: a implementação de mapper começa em `a856a4ed50e0ba590f1abce5ddd6782b29cd3f8b`; `7682cb6290269ebc02237ecdca543c59e3c63b27` corrige o gating pelo disable do cartucho. O SHA final `3bb07dc981d22d4242e86bd89090ed212d3e4f71` reorganiza testes e código, não representa isoladamente a introdução da funcionalidade.

A lista de arquivos do endpoint de PR confirma 13 arquivos no delta #424 e 9 no #543. A #543 inclui mudanças de VDP/Z80 e está empilhada sobre #542; não é uma unidade segura para cherry-pick.

## 2. Confronto com a implementação atual

No HEAD atual de `main`, `native/sms_core/src/memory.rs`:

- `SmsMemory` contém ROM, mapper, latch `mapper_control`, bancos ROM, work RAM, BIOS e `control_port`.
- Não contém armazenamento SRAM de cartucho.
- A leitura de `$8000-$BFFF` continua indo ao banco ROM.
- `$FFFC` grava o latch, mas os bits 2/3/4 não habilitam SRAM.
- `$C000-$FFFF` usa apenas a work RAM de 8 KiB espelhada, ou retorna open/unmapped conforme o latch `$3E`.
- `SmsTemporalBusState` contém `SmsMemory`; adicionar SRAM dentro de `SmsMemory` faz o conteúdo participar de `Clone` e do snapshot em memória. Isso, sozinho, não comprova serialização persistente por serde nem Save State durável.

O barramento temporal aplica o efeito de leitura/escrita somente no limite de conclusão agendado. Portanto, a implementação de SRAM deve continuar dentro do caminho de memória já invocado nesse limite, sem I/O de arquivo no acesso de barramento.

## 3. Contrato de hardware consolidado

A documentação SMS Power define:

- `$FFFC bit 3`: seleciona RAM de cartucho em `$8000-$BFFF`, sobrepondo o ROM do slot 2.
- `$FFFC bit 2`: seleciona a primeira ou segunda página SRAM de 16 KiB, **somente para o slot 2**.
- `$FFFC bit 4`: mapeia RAM de cartucho na janela de RAM do sistema `$C000-$FFFF`; a RAM interna deve ser desabilitada via porta `$3E` para evitar contenção.
- Cartuchos de 8 KiB normalmente espelham o conteúdo para preencher a janela de 16 KiB.
- Os registradores `$FFFC-$FFFF` são write-only; leituras retornam o valor armazenado na RAM espelhada. Escritas nos registradores também alcançam a cópia de RAM.

Fontes primárias:
- https://www.smspower.org/Development/Mappers
- https://www.smspower.org/Development/MemoryMap
- https://www.smspower.org/forums/6807-OffficalSMSDocumentationOfMapperRegisterFffc

## 4. Revisão semântica de #424

### 4.1 Aspectos reaproveitáveis

- SRAM separada da work RAM dentro de `SmsMemory`.
- Capacidade explícita de 8/16/32 KiB, sem inferência arbitrária por nome ou tamanho da ROM.
- Bit 3 e bit 2 aplicados à janela do slot 2.
- Bit 4 tratado separadamente para a janela alta e condicionado ao estado da porta `$3E`.
- Gate de cartucho desabilitado via `$3E` antes de permitir o acesso à SRAM.
- Persistência em módulo host-side separado do hot path, com cabeçalho, versão, tamanho e CRC32.
- Integração prevista em máquinas convencional e temporal, APIs PyO3 e adaptador Python.
- Testes de corrupção e recuperação de arquivo, além de testes de mapeamento e persistência.

### 4.2 Pontos que precisam de correção ou comprovação antes do port

1. **Overlay de `$FFFC-$FFFF` com bit 4:** no patch de #424, o caminho de leitura/escrita da SRAM alta exclui endereços a partir de `$FFFC`. A documentação de Memory Map diz que os registradores sobrepostos continuam escrevendo na RAM e que leituras retornam a cópia de RAM. É necessário definir e testar como esse contrato se aplica quando a SRAM externa substitui a work RAM; não copiar a exclusão sem uma decisão de hardware documentada.
2. **Teste direto de capacidade 16 KiB:** o índice por máscara de potência de dois faz o segundo banco lógico retornar à mesma área quando a imagem tem 16 KiB. Isso é coerente com uma única página física, mas deve haver teste explícito demonstrando o alias esperado; o bit 2 só deve distinguir páginas quando houver 32 KiB.
3. **Bit 7 e bank shift:** a documentação descreve write-protect de ROM em hardware de desenvolvimento e shift dos bancos nos bits 1–0, mas declara que software conhecido não os usa. Não ampliar o escopo da SRAM para implementar esses comportamentos sem evidência e testes próprios.
4. **Persistência e Save State:** arquivo `.srm` e estado serializado de emulação são contratos distintos. O conteúdo SRAM deve entrar no estado completo do chip; o arquivo do host deve ser carregado/salvo em fronteiras explícitas. Não declarar Save State completo só porque `Clone` preserva a memória.
5. **Capacidade e identidade:** a configuração explícita é correta para a primeira fatia. A seleção automática depende de registry de mídia/fingerprint confiável e permanece uma tarefa separada.

## 5. Revisão semântica de #543

A #543 fornece uma fatia menor: 8 KiB fixos, espelhamento em slot 2, janela alta com RAM interna desabilitada, export/restore e integração PyO3/Python. O ajuste `7682cb6` adiciona a condição de cartucho habilitado (`$3E bit 6`) para o acesso à SRAM.

Limites declarados e observados no escopo da proposta: não cobre 16/32 KiB nem seleção por bit 2; não resolve descoberta de capacidade; não oferece persistência automática do frontend; não cobre SRAM Codemasters. Além disso, o delta contém alterações não relacionadas à SRAM e herda a base de #542.

**Decisão:** usar #543 como referência histórica de testes e de gating de cartucho, não como base de port.

## 6. Critérios de aceite para a implementação atual

| Caso | Comportamento exigido |
|---|---|
| SRAM não configurada | Comportamento legado idêntico; bits de SRAM não devem expor memória inexistente |
| 8 KiB, bit 3 | SRAM visível em `$8000-$BFFF`, com espelhamento de 8 KiB |
| 16 KiB, bit 3 | Uma página física de 16 KiB; alternar bit 2 não cria uma segunda página |
| 32 KiB, bit 3 | Bit 2 seleciona páginas independentes de 16 KiB |
| Bit 4 | Janela `$C000-$FFFF` somente com cartucho habilitado e RAM interna desabilitada via `$3E` |
| Overlay `$FFFC-$FFFF` | Leituras/escritas preservam o contrato de registradores write-only e espelho de RAM |
| `$3E` desabilita cartucho | SRAM externa deixa de responder |
| Snapshot/restore | SRAM, latch mapper, porta `$3E` e bancos restaurados sem divergência |
| Temporal | Efeito da SRAM ocorre apenas no deadline da transação |
| Persistência | Arquivo validado por versão/tamanho/CRC; sem I/O no caminho de CPU/barramento |

Testes diferenciais devem comparar o caminho convencional e temporal para a mesma sequência de transações. Testes de snapshot devem capturar o estado antes e depois de uma troca de banco e continuar a execução em blocos diferentes, verificando igualdade byte a byte.

## 7. Classificação e próximo passo

- Auditoria das listas de commits e arquivos alterados de #424/#543: **PASS**.
- Confronto do contrato principal de SRAM com SMS Power: **PASS**.
- Decisão de não fazer cherry-pick de branches obsoletas/empilhadas: **PASS**.
- Compatibilidade exata do overlay `$FFFC-$FFFF` no slot alto: **BLOCKED** até teste/decisão documentada.
- Execução local de Cargo/Ruff/Pytest nesta sessão: **NOT RUN**.
- Boot/gameplay com ROM legal de teste: **NOT RUN**.
- Port de runtime: **NOT STARTED**.

Próximo incremento permitido: implementar primeiro a SRAM explícita de 8 KiB no `SmsMemory` atual, com gating de cartucho e cobertura do overlay; depois estender para 16/32 KiB e persistência em mudanças isoladas. A branch deve partir do HEAD atual de `main`, e o port não deve ser publicado como concluído sem os gates Rust/Python no HEAD final.


## 8. Revalidação de escopo após consolidação Rust-only (2026-11-07)

A revisão das PRs #424/#543 permanece uma análise histórica sobre a baseline
`3ca0f648c841bfe7c85a301fb561273e5c62f676`. A `main` atual usada nesta
atualização documental é `1506f93bfa06905085da9a74b64353ff086b96d9`.

- A implementação de produto é Rust-only. Referências históricas a PyO3,
  adaptadores Python e frontend Qt não são requisitos nem destinos de port.
- Não executar nem exigir pytest/Ruff para validar o produto. Os gates
  obrigatórios são os três comandos Cargo definidos no contrato do projeto,
  mais gates Windows específicos quando aplicáveis.
- A auditoria de runtime descrita nas seções anteriores não foi repetida sobre
  a `main` atual; não atribuir status PASS à inspeção do runtime atual.
- O overlay `$FFFC-$FFFF` na SRAM alta permanece BLOCKED até definição de
  vetor reproduzível e decisão de hardware. Não copiar a exclusão encontrada
  no patch #424 sem resolver esse contrato.
- SRAM deve integrar o estado completo serializável do hardware; persistência
  de arquivo continua separada e fora do caminho de barramento.

**Classificação desta atualização:** escopo Rust-only PASS; revisão histórica
#424/#543 PASS no escopo registrado; auditoria do runtime atual NOT RUN;
overlay BLOCKED; implementação NOT STARTED; validação in-game NOT RUN.
