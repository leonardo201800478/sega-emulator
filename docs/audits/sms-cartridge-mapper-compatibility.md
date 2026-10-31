# SMS — compatibilidade de mappers de cartucho e prevenção de falhas de boot

**Estado:** pesquisa documental e catalogação inicial; runtime não alterado.  
**Prioridade:** P0 para fechamento da compatibilidade de carregamento SMS.  
**Escopo:** Codemasters, ports MSX/coreanos, mappers coreanos, multicarts e identificação por fingerprints.  
**Regra:** não selecionar mapper por nome, região, tamanho ou suposição de família. Usar identidade verificável + perfil explícito; desconhecido deve permanecer UNKNOWN e não cair silenciosamente no mapper Sega.

## 1. Objetivo e risco

Um cartucho SMS não é necessariamente um conjunto de ROM banks de 16 KiB controlado por \`0xFFFC–0xFFFF\`. Existem placas que decodificam escritas em endereços que a CPU normalmente lê como ROM, mapeamentos em janelas de 8 KiB, bancos fixos/variáveis específicos, multicarts e circuitos com comportamento adicional.

Se o barramento tratar toda escrita para uma faixa de ROM como uma operação sem efeito, um mapper que usa escrita endereçada nessa faixa nunca receberá o comando de troca de banco. O jogo pode iniciar parcialmente, mas travar ao carregar níveis, menus ou sub-jogos.

A correção arquitetural não é “mudar o SMS para slots de 8 KiB” globalmente. É selecionar um **CartProfile/MapperProfile** explícito por imagem/placa e deixar o mapper interceptar as transações que o hardware realmente decodifica.

## 2. Evidência e confiança

- **DOCUMENTADO:** comportamento descrito na documentação técnica SMS Power ou identificado em catálogo de código-fonte público de emulador.
- **CORROBORADO POR CATÁLOGO:** ROM identificada por CRC32 em mais de uma base técnica, mas a implementação física ainda exige conferência.
- **PENDENTE:** nome de jogo ou alegação recebida sem confirmação suficiente de mapper/fingerprint.
- **NÃO CONCLUIR:** afirmação geral que as fontes contradizem ou não sustentam.

As tabelas abaixo não substituem análise dos dumps locais. CRC32 é útil para selecionar uma entrada de catálogo, mas não é identidade criptográfica: deve ser acompanhado de tamanho e SHA-256 do dump examinado, quando disponível.

## 3. Mappers e decodificação confirmados

### 3.1 Sega mapper (baseline, não universal)

O mapper Sega padrão controla três janelas de ROM de 16 KiB por escritas em \`0xFFFD\`, \`0xFFFE\` e \`0xFFFF\`; \`0xFFFC\` controla RAM e funções auxiliares. O primeiro 1 KiB do slot 0 (\`0x0000–0x03FF\`) permanece fixo para preservar vetores de interrupção, enquanto o restante do slot 0 pode ser paginado. A BIOS export assume esse mapper para checksum de ROMs acima de 48 KiB.

Não usar a frase “o primeiro banco é sempre estático” como regra global do SMS. Ela é válida apenas em parte para o Sega mapper: o primeiro 1 KiB é preservado; há mapper Codemasters e multicarts com outras regras.

### 3.2 Codemasters mapper

**DOCUMENTADO — SMS Power, Mappers:**

- Três slots de 16 KiB.
- Registradores decodificados em toda a faixa de cada slot; endereços canônicos: \`0x0000\`, \`0x4000\`, \`0x8000\`.
- Escritas em outros endereços da mesma janela são mirrors válidos.
- Diferentemente do Sega mapper, o primeiro 1 KiB do slot 0 não é protegido.
- Estado inicial documentado: bancos 0, 1 e 0 nos slots 0, 1 e 2.
- Há um caso especial de RAM em Ernie Els Golf: valor escrito em \`0x4000\` com bit 7 setado habilita RAM no segmento superior de 8 KiB do slot 2.

A camada de barramento precisa encaminhar writes às janelas de cartucho para o mapper selecionado, mesmo quando a memória lida naquele endereço é ROM. O mapper decide se o endereço/espelho é registrador e se a escrita tem efeito. Não descartar a escrita antes dessa decodificação.

**Jogos listados com evidência de Codemasters mapper no Genesis Plus GX:**

| Jogo / variante | CRC32 | Estado |
|---|---:|---|
| Excellent Dizzy Collection, The (Europe) (Proto) | \`8813514B\` | CORROBORADO POR CATÁLOGO |
| Dinobasher - Starring Bignose the Caveman (SMS Proto) | \`EA5C3A6F\` | CORROBORADO POR CATÁLOGO |

O material recebido também lista o protótipo Excellent Dizzy com idiomas En/Fr/De/Es/It; a correspondência pelo CRC acima deve ser confirmada com o dump local antes de atribuir o perfil. Não selecionar Codemasters mapper apenas porque a publisher é Codemasters: a seleção deve vir do fingerprint/perfil, pois outros cartuchos da empresa podem usar hardware distinto ou variantes SMS/GG.

### 3.3 MSX 8 KiB mapper / Nemesis variant

**DOCUMENTADO — SMS Power, Mappers:**

O MSX mapper possui registradores de seleção nos endereços \`0x0000–0x0003\`, com quatro janelas de 8 KiB:

| Write address | Janela selecionada |
|---|---|
| \`0x0000\` | \`0x8000–0x9FFF\` |
| \`0x0001\` | \`0xA000–0xBFFF\` |
| \`0x0002\` | \`0x4000–0x5FFF\` |
| \`0x0003\` | \`0x6000–0x7FFF\` |

A primeira janela \`0x0000–0x3FFF\` permanece fixa no MSX mapper. O Nemesis variant difere porque fixa o último banco de 8 KiB em \`0x0000–0x1FFF\` e o segundo banco em \`0x2000–0x3FFF\`. Não colapsar ambos em um perfil sem essa diferença.

**Jogo confirmado:**

| Jogo | CRC32 | Perfil apontado pelo catálogo |
|---|---:|---|
| Cyborg Z (Korea) | \`77EFE84A\` | MSX 8 KiB |

O SMS Power identifica Cyborg Z como port MSX e o catálogo Genesis Plus GX o classifica como MSX 8K. Isso sustenta o perfil, não a afirmação genérica de que todos os ports coreanos usam o mesmo mapper.

### 3.4 Korean mapper simples (não é MSX mapper)

O “Korean mapper” descrito separadamente pelo SMS Power é mais simples: slots 0 e 1 fixos nos bancos 0 e 1; o slot 2 (\`0x8000–0xBFFF\`) é selecionado por um byte escrito em \`0xA000\`.

**Jogo confirmado:**

| Jogo | CRC32 | Perfil apontado pelo catálogo |
|---|---:|---|
| Dallyeora Pigu-Wang (Korea) (Unl) / Run! Dodgeball King | \`89B79E77\` | Korean mapper simples |

Portanto, Dallyeora Pigu-Wang não deve ser agrupado automaticamente com o MSX 8K mapper de Cyborg Z. São mecanismos diferentes, com endereços de controle diferentes.

### 3.5 Janggun mapper e Jang Pung — não confundir

O **Janggun mapper** documentado pelo SMS Power oferece uma janela fixa de 16 KiB em \`0x0000–0x3FFF\` e quatro janelas selecionáveis de 8 KiB em \`0x4000–0x5FFF\`, \`0x6000–0x7FFF\`, \`0x8000–0x9FFF\` e \`0xA000–0xBFFF\`. Escritas em \`0xFFFE\` e \`0xFFFF\` também selecionam pares de janelas de 16 KiB e ativam a lógica de bit reversal documentada. Os writes nesses dois endereços também aparecem na RAM de sistema.

Entretanto, **Jang Pung II e Jang Pung 3 não devem ser classificados como Janggun mapper só pelo nome**. O catálogo Genesis Plus GX classifica as versões SMS listadas como \`KOREA_16K_V1\`, com fingerprints próprios:

| Jogo | CRC32 | Perfil do catálogo |
|---|---:|---|
| Jang Pung II (Korea) (SMS) | \`929222C4\` | Korea 16K V1 |
| Jang Pung 3 (Korea) (SMS) | \`18FB98A3\` | Korea 16K V1 |

A variante SMS-GG de Jang Pung II aparece separadamente com CRC \`76C5BDFB\` e perfil \`KOREA_16K_V2\`; não reutilizar automaticamente o perfil da versão SMS.

**LACUNA:** obter a especificação detalhada da família Korea 16K V1/V2, endereços de registrador, largura/espelhamento e estado inicial. Até isso ser confirmado, preservar os nomes de perfil do oráculo como aliases de catalogação, não como especificação elétrica completa.

### 3.6 Multicarts — cada placa tem seu mapper

Os multicarts não formam uma família elétrica única. A seleção de jogo pode afetar todos os bancos, inclusive o bloco inicial, e pode incluir registradores de menu, valores codificados, trampolines em RAM e bancos de 8 KiB/16 KiB. O mapper deve implementar o circuito/algoritmo observado, não apenas trocar uma ROM lógica inteira por um índice de menu.

| Jogo | CRC32 | Perfil encontrado | Observação |
|---|---:|---|---|
| 4 PAK All Action (Australia) | \`A67F2A5C\` | Multi 16 KiB | Não é o mesmo perfil de 128 Hap |
| 128 Hap (Korea) | \`BA5EC0E3\` | Multi 4×8 KiB | SMS Power descreve protocolo de escrita em \`0x2000\` com XOR \`0x1F\` |
| Game Mo-eumjip 188 Hap [v0] (Korea) | \`380D7400\` | Multi 4×8 KiB | Usa o mesmo mapper \`0x2000 xor 0x1F\` de 128 Hap, mas menu/layout diferente |
| Game Mo-eumjip 188 Hap [v1] (Korea) | \`C76601E0\` | Multi 4×8 KiB | Variante distinta, mesmo perfil de mapper catalogado |
| 20 em 1 (Brazil) | \`F0F35C22\` | UNKNOWN — investigar | O catálogo MAME confirma tamanho/CRC, mas as fontes consultadas não bastam para fixar o mapper. Não assumir Multi 16K/4×8K nem Sega por inferência. |

A pesquisa de SMS Power para 188 Hap registra explicitamente o mapper \`0x2000 xor 0x1F\`, os valores de menu \`0x1F\` e \`0x1B\`, páginas-base e records de trainer; isso não é equivalente a um simples registrador em \`0x3FFE\`. O texto recebido sobre \`0x3FFE\` não deve ser usado como regra genérica de multicart.

O catálogo Genesis Plus GX separa \`4-Pak All Action\` como \`MULTI_16K\` e \`128 Hap\`/188 Hap como \`MULTI_4x8K\`. Logo, uma única implementação genérica para todos os multicarts seria insuficiente.

### 3.7 E.I. — Exa Innova: exceção à classificação recebida

**Não confirmado como mapper especial.** O dump listado de E.I. - Exa Innova tem 32 KiB e CRC32 \`DD74BCF1\`. As fontes consultadas descrevem-no como port coreano de um jogo MSX, mas não demonstram que use o MSX 8K mapper do cartucho. Com 32 KiB, pode ser imagem linear sem paging; isso deve ser verificado pelo dump/PCB, não inferido apenas pela origem do software.

Classificação provisória: \`UNKNOWN / NO BANKING ASSUMPTION\`; não forçar MSX 8K. Antes de iniciar o jogo, o loader pode permitir o perfil linear padrão para ROMs de 32 KiB, mas a matriz deve registrar essa decisão como hipótese e confirmar o boot/gameplay com evidência.

## 4. Política de identificação e seleção

### 4.1 Fingerprints

A identificação deve preservar e calcular:
- tamanho exato do arquivo;
- CRC32 para compatibilidade com catálogos legados;
- SHA-1 quando o DAT/oráculo o fornecer;
- SHA-256 como fingerprint local forte;
- cabeçalho/offsets e presença de padding/overdump;
- origem, revisão, variante SMS/GG e estado de integridade.

O registry consulta uma tabela versionada de perfis. O match deve usar a combinação definida no manifesto (preferencialmente SHA-256; CRC32 + tamanho quando essa for a única referência disponível), nunca só o nome do arquivo.

### 4.2 Decisão fail-closed

1. Identificar plataforma e formato sem modificar o dump.
2. Consultar fingerprints conhecidos.
3. Resolver um MapperProfile explícito e sua versão.
4. Se houver match inequívoco, instanciar o mapper e seu estado inicial documentado.
5. Se houver múltiplos matches ou identidade insuficiente, marcar \`AMBIGUOUS\`/\`UNKNOWN\`; não escolher Sega mapper silenciosamente.
6. Permitir seleção manual explícita como override de sessão, registrando-a no diagnóstico; não reescrever o catálogo global automaticamente.
7. Não inferir mapper por região, publisher, extensão, tamanho ou nome de jogo.

### 4.3 Contrato do barramento e mapper

O contrato de memória deve encaminhar ao mapper as transações de leitura/escrita com endereço, dado e contexto suficiente para o mapper aplicar a decodificação real. O caminho de write não pode rejeitar uma transação apenas porque o caminho de read naquele endereço entrega bytes de ROM.

Separar:
- **Read decode:** qual ROM/RAM responde a uma leitura em determinado endereço.
- **Write decode:** quais registradores/latches respondem a uma escrita no endereço, inclusive em faixas de leitura ROM.
- **Bank mapping:** tradução da janela Z80 para offset físico/virtual da mídia.
- **Reset state:** bancos/latches definidos por power-on/reset da placa.
- **Persistence:** RAM/SRAM/EEPROM, se presente, sem misturar com ROM banking.

O mapper não deve possuir a CPU nem o VDP, e não deve introduzir clocks paralelos. Writes e efeitos observáveis permanecem ordenados pela timeline Rust.

## 5. Critérios de teste para evitar falha de boot

### QUICK_PASS por mapper

- Vetores de reset e estado inicial dos registradores/bancos.
- Leituras no início/fim de cada janela, incluindo wrap e limites da ROM.
- Writes em todos os endereços canônicos e mirrors documentados.
- Escritas em endereço de leitura ROM são encaminhadas corretamente.
- Primeiro 1 KiB do slot 0 protegido apenas nos perfis em que isso é verdadeiro.
- Mapeamentos de 8 KiB e 16 KiB não se sobrepõem incorretamente.
- Banco inválido, ROM não potência de dois e tamanho parcial têm comportamento explícito, sem index panic ou wrap inventado.
- RAM de cartucho, se existente, não é confundida com Work RAM do console.

### SHORT_PASS por jogo

Para cada ROM legal de teste disponível localmente:
- registrar tamanho, CRC32, SHA-1 (se aplicável) e SHA-256;
- confirmar perfil selecionado e estado inicial;
- executar reset a frio e observar o primeiro fetch;
- capturar writes aos registradores de mapper e mudanças de banco;
- confirmar boot, tela/menu e ao menos uma transição que force troca de banco (nível, sub-jogo, menu);
- comparar traces com Ares/MAME/Genesis Plus GX, registrando versão e divergências.

### Matriz de compatibilidade

Registrar em \`docs/compat/\` (ou caminho equivalente existente) os campos:
\`title\`, \`variant\`, \`size\`, \`crc32\`, \`sha1\`, \`sha256\`, \`mapper_profile\`, \`profile_evidence\`, \`status\`, \`scenario\`, \`candidate_sha\`, \`limitations\`.

Estados de jogo não podem ser promovidos apenas por identificação do mapper:
- \`MAPPER_IDENTIFIED\` não significa boot confirmado;
- \`BOOT_CONFIRMED\` não significa gameplay validado;
- \`GAMEPLAY_SMOKE\` não significa regressão longa;
- \`VALIDATED\` exige os critérios definidos no projeto.

## 6. Lacunas priorizadas

| ID | Prioridade | Lacuna | Encerramento exigido |
|---|---|---|---|
| MAP-01 | P0 | Inventário real dos mappers já existentes no runtime/branch | código, testes e estado atual mapeados; sem duplicar mapper existente |
| MAP-02 | P0 | Write decode em regiões ROM | teste prova que writes chegam ao mapper antes de serem descartados |
| MAP-03 | P0 | Codemasters mapper | reset, mirrors, três slots e proteção de vetores testados |
| MAP-04 | P0 | MSX 8K + Nemesis | quatro registradores, slots fixos e diferença Nemesis testados |
| MAP-05 | P0 | Korean mapper simples | seleção do slot 2 por \`0xA000\` reproduzida |
| MAP-06 | P1 | Korea 16K V1/V2 | especificação elétrica e diferença entre variantes obtidas |
| MAP-07 | P1 | Multicart Multi 16K / Multi 4×8K | circuitos/protocolos separados e transição de sub-jogo validada |
| MAP-08 | P1 | HiCOM / Hwasung / outros perfis especiais | inventário e fingerprints completos; sem agrupar por publisher |
| MAP-09 | P1 | 20 em 1 (Brasil) | mapper confirmado por fonte independente/trace |
| MAP-10 | P1 | E.I. Exa Innova | confirmar se é ROM linear 32 KiB ou requer mapper |
| MAP-11 | P1 | Catálogo de fingerprints | SHA-256 + CRC/tamanho, procedência, revisão e versão do catálogo |
| MAP-12 | P1 | Testes de boot e transição | SHORT_PASS reproduzível no SHA candidato |

## 7. Decisões arquiteturais

- Não criar um “Korean Mapper” monolítico. MSX 8K, Korean simples, Korea 16K V1/V2, Janggun, HiCOM e mappers de multicart são perfis distintos até que equivalência elétrica seja demonstrada.
- Não criar um único “Multicart Mapper” que apenas troca o jogo inteiro. Implementar cada protocolo de placa, com estado persistível.
- Não usar tamanho/país/nome como seleção de mapper.
- Não manter fallback Sega para identidade desconhecida.
- O carregador pode identificar e propor o perfil; o mapper Rust executa a lógica; o barramento SMS resolve reads/writes; o scheduler Rust ordena os efeitos.
- Nenhuma alteração desta pesquisa muda o runtime ou declara os jogos como bootados.

## 8. Referências

1. SMS Power!, [Mappers — Development](https://www.smspower.org/Development/Mappers): Sega, Codemasters, Korean, MSX/Nemesis e Janggun mappers.
2. SMS Power!, [Game Mo-eumjip 188 Hap — discussão técnica](https://www.smspower.org/forums/18978-25thAnniversary3Prototypes1Unreleased188In1Multicart2BrasilianVariants): protocolo \`0x2000 xor 0x1F\`, menu e trainers do 188 Hap.
3. Genesis Plus GX, [sms_cart.c](https://github.com/ekeeke/Genesis-Plus-GX/blob/master/core/cart_hw/sms_cart.c): catálogo de CRCs e classificação de Codemasters, MSX, Korea 16K, multicarts, HiCOM e outros perfis. Usado como oráculo comparativo, não como prova final de hardware.
4. MAME, [hash/sms.xml](https://github.com/mamedev/mame/blob/master/hash/sms.xml): metadados de dumps, CRC/SHA-1 e identificação de placas/slots quando disponível.
5. SMS Power!, [Cyborg Z](https://www.smspower.org/Games/CyborgZ-SMS): CRC32 e classificação do jogo como port MSX.
6. No-Intro SMS DAT e catálogo Genesis Plus GX: identidade por CRC32 e classificação de Dallyeora Pigu-Wang; a documentação SMS Power descreve separadamente o Korean mapper simples.
7. SMS Power!, [E.I. - Exa Innova](https://www.smspower.org/Games/EIExaInnova-SMS): dump de 32 KiB e CRC32; não demonstra mapper especial.
8. SMS Power!, [20 em 1](https://www.smspower.org/Games/20Em1-SMS): dump brasileiro de 256 KiB e CRC32; mapper permanece pendente.

**Conclusão:** o risco de boot por mapper é real e documentado. A classificação exata por jogo exige fingerprints e perfis distintos. As alegações iniciais foram preservadas como ponto de partida, mas corrigidas onde as fontes técnicas mostram mecanismos diferentes ou não sustentam um mapper especial.
