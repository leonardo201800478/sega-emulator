# Projeto: carregamento nativo de BIOS do Sega Master System

**Data:** 2026-10-29  
**Base observada:** `main` em `0a6e77c2f01b87692accd033525197eb8812707b` (PR #279 integrada)  
**Status:** IMPLEMENTED / CI_PENDING / REAL_BIOS_BOOT_PENDING. O scanner por SHA-256, a configuração persistente da pasta, a seleção regional, o encaminhamento Qt/PyO3 e o boot pelo core Rust estão conectados. A validação com BIOS real permanece dependente de arquivos locais do usuário e teste manual; CI deve ser avaliado no SHA candidato mais recente.

## Objetivo

Adicionar suporte nativo e opcional a BIOS de Master System fornecidas pelo usuário. Os arquivos devem permanecer fora do repositório e da distribuição do emulador. O usuário poderá escolher uma pasta de firmware nas configurações; o emulador identificará os arquivos por SHA-256, sem depender de nomes fixos, e selecionará a BIOS compatível com o perfil regional efetivo.

O SMS deve continuar iniciando sem BIOS quando o modo de boot selecionado permitir isso. A presença de BIOS não deve ser confundida com a configuração PAL/NTSC nem com a região do cartucho.

## Evidência dos três arquivos fornecidos nesta conversa

Os hashes abaixo foram calculados localmente a partir dos arquivos anexados. Os binários não foram adicionados ao Git.

| Arquivo recebido | Tamanho | SHA-256 | Identificação pelo catálogo consultado |
|---|---:|---|---|
| `jp_sms.bin` | 8.192 bytes | `67846e26764bd862f19179294347f7353a4166b62ac4198a5ec32933b7da486e` | Master System Japan |
| `us_sms.bin` | 8.192 bytes | `477617917a12a30f9f43844909dc2de6e6a617430f5c9a36306c86414a670d50` | Master System USA / Europe |
| `eu_sms.bin` | 8.192 bytes | `477617917a12a30f9f43844909dc2de6e6a617430f5c9a36306c86414a670d50` | Master System Europe; mesmo conteúdo de US |

**Resultado importante:** os arquivos US e EU são byte a byte idênticos segundo tamanho e SHA-256; devem ser tratados como uma única imagem de firmware com aliases regionais, não como duas BIOS distintas. O arquivo JP possui hash diferente.

Os rótulos regionais acima são sustentados pela correspondência de SHA-256 no catálogo público RetroBIOS; não inferir uma versão exata além do que o hash/catalogação confirma.

## Referência: abordagem do ares

A documentação pública do perfil ares no catálogo RetroBIOS informa que:
- a identificação de firmware é feita por SHA-256, não pelo nome do arquivo;
- os nomes são definidos pelo usuário;
- a pasta de firmware pode ser configurada e existe uma pasta padrão de dados do usuário;
- os slots de firmware são associados ao sistema/perfil depois da identificação.

Fontes:
- SMS Power, Port $3E: https://www.smspower.org/Development/Port3E
- SMS Power, comportamento das BIOSes: https://www.smspower.org/Development/BIOSes
- SMS Power, formato e códigos regionais do cabeçalho: https://www.smspower.org/Development/ROMHeader
- Charles MacDonald, *SMS/GG hardware notes* (2002), seções 2–4 e 6; cópia de referência: https://github.com/drhelius/Gearsystem/blob/master/skills/gearsystem-debugging/references/smstech.md
- ares upstream: https://github.com/ares-emulator/ares
- perfil ares e referências ao código de varredura por hash: https://abdess.github.io/retrobios/emulators/ares/
- catálogo Sega e hashes de Master System: https://abdess.github.io/retrobios/systems/sega/
- código do core SMS atual no projeto: `native/sms_core/src/machine.rs`, `memory.rs`, `config.rs` e integração da interface.

A pesquisa do catálogo descreve a identificação por conteúdo e os arquivos conhecidos; a implementação precisa verificar diretamente a revisão de código do ares que será usada como referência antes de fechar decisões de arquitetura. Não copiar código do ares.

## Comportamento proposto

### Configuração e diretório

- Criar uma configuração persistente `firmware_directory` definida pelo usuário.
- A pasta é configurada explicitamente pelo usuário nas configurações Qt; o valor vazio significa BIOS não configurada.
- Expor caminho, seletor de pasta, botão de varredura explícita e resultado por arquivo (status, região e prefixo SHA-256).
- Não varrer disco no startup. A varredura ocorre quando solicitada no diálogo ou ao carregar/resetar uma ROM SMS.
- Não manter cache de bytes entre sessões; a seleção revalida tamanho e SHA-256 imediatamente antes de encaminhar os bytes ao core.
- Não baixar BIOS automaticamente.

### Identificação e validação

- Ler arquivos candidatos como dados locais, calcular SHA-256 e comparar com uma lista interna de metadados (hash, sistema, região/alias, tamanho esperado e versão quando conhecida).
- Não confiar em nomes, extensões ou diretórios para validar firmware.
- A BIOS japonesa aceita o hash JP indicado acima.
- A BIOS de exportação US/EU aceita o hash compartilhado US/EU indicado acima.
- Recusar arquivos truncados, vazios, inacessíveis ou de tamanho incompatível, com erro diagnóstico explícito.
- Nunca gravar o conteúdo ou dumps da BIOS em logs; registrar apenas nome/caminho escolhido pelo usuário, tamanho, hash, região identificada e resultado da validação.
- Manter os hashes em testes de metadados, sem armazenar os binários proprietários no repositório.

### Seleção regional

- Separar **território do console**, **padrão de vídeo PAL/NTSC** e **região/identidade da BIOS**; não assumir que esses campos são equivalentes.
- Definir uma política explícita de preferência: região manual do usuário tem prioridade; no modo automático, resolver a região usando os metadados confiáveis disponíveis do cartucho e do sistema, e registrar a decisão.
- Para território japonês, preferir BIOS JP quando instalada; para território export, preferir BIOS US/EU compartilhada.
- Se não houver correspondência, respeitar a política de boot sem BIOS e mostrar estado “não instalada”; não escolher uma BIOS incompatível silenciosamente.
- Para modo automático, ler o nibble alto de `$7FFF` em um cabeçalho `TMR SEGA` localizado em `$7FF0`, `$3FF0` ou `$1FF0`: código 3 seleciona BIOS JP e código 4 seleciona BIOS de exportação US/EU.
- Se o cabeçalho estiver ausente ou tiver código desconhecido, não inferir uma BIOS; preservar boot direto sem firmware. A preferência regional manual tem precedência.
- Quando mais de um arquivo tiver o mesmo hash conhecido, manter a primeira ocorrência determinística selecionável e marcar as demais como `DUPLICATE_HASH`.
- Os códigos do cabeçalho identificam território (Japan/Export), não PAL/NTSC. O padrão de vídeo continua sendo uma escolha separada.

### Integração de hardware

A BIOS deve ser parte do caminho de inicialização do cartucho/máquina, não uma imagem desenhada na interface. A implementação deve:
- definir um objeto de firmware validado (metadados + bytes) fora da ROM do jogo;
- encaminhar a BIOS validada pela fronteira Python/PyO3 ao core Rust sem callbacks por instrução;
- implementar overlay/mapeamento de BIOS e o mecanismo de desabilitação conforme o comportamento documentado do SMS;
- garantir que as leituras da BIOS e a desabilitação afetem o mapa de memória real visto pela CPU;
- manter a ROM de cartucho e seu mapper intactos, e permitir que o jogo passe a ser executado após a BIOS transferir controle ou ser desabilitada;
- limpar/reinicializar estado de overlay e latch no reset, de acordo com o contrato de hardware confirmado;
- evitar duplicar o carregador de ROM ou criar um segundo runtime Python de hardware.

### Semântica de controle validada para esta fatia

- O SMS decodifica escritas pares no intervalo de portas `$00-$3F` como o latch de memória; o core aplica essa máscara, não apenas a porta literal `$3E`.
- Os bits são ativos em nível baixo: bit 7 expansão, bit 6 cartucho, bit 5 card, bit 4 RAM interna, bit 3 BIOS e bit 2 I/O.
- A BIOS fica habilitada no power-on. O core modela o perfil inicial `$F3` (BIOS ativa; cartucho e RAM interna desabilitados) e o perfil pós-BIOS de cartucho `$AB` (BIOS desabilitada; cartucho e RAM interna habilitados).
- O fluxo sintético de aceitação habilita RAM mantendo BIOS, copia um pequeno stub para RAM, desabilita BIOS/habilita cartucho e transfere execução ao endereço zero do cartucho.
- A BIOS de 8 KiB fornecida pelo usuário só ocupa o mapeamento integral de 8 KiB em `$0000-$1FFF`; os bytes restantes não são expostos por esse overlay. O slot do cartucho continua sujeito ao latch de memória.
- A arbitragem elétrica quando BIOS e cartucho são habilitados simultaneamente difere entre SMS e SMS 2 (precedência/AND nas referências consultadas); esse caso não é exercitado pelo fluxo de boot implementado, que mantém os slots mutuamente selecionados. A modelagem explícita dessa sobreposição permanece uma limitação, não um comportamento declarado como validado.

**Não assumir que a BIOS deve estar sempre mapeada ou sempre obrigatória.** O perfil de inicialização e a transferência de controle devem respeitar o latch e a ordem observada nos documentos técnicos citados.

## Estados de diagnóstico propostos

- `NOT_CONFIGURED`: pasta ainda não definida.
- `DIRECTORY_MISSING`: pasta configurada não existe ou não está acessível.
- `NOT_FOUND`: nenhum arquivo corresponde a uma assinatura conhecida.
- `INVALID_FILE`: falha de leitura/tamanho/validação.
- `VALID_JAPAN`: hash japonês validado.
- `VALID_EXPORT_US_EU`: hash compartilhado US/EU validado.
- `DUPLICATE_HASH`: múltiplos nomes, mesmo conteúdo já reconhecido.
- `IN_USE`: firmware selecionado e carregado na máquina.
- `FALLBACK_NO_BIOS`: modo de compatibilidade selecionado sem BIOS.

## Critérios de aceitação

### Validação de metadados e armazenamento
- [ ] Testar hashes exatos dos três arquivos fornecidos por fixtures de teste não versionadas/localmente ou por constantes de metadados; não commitar binários.
- [ ] Confirmar que JP é classificada como Japan e que o hash compartilhado é classificado como US/Europe.
- [ ] Confirmar que nomes arbitrários funcionam e nomes “corretos” com conteúdo incorreto são rejeitados.
- [ ] Testar arquivo ausente, tamanho incorreto, falha de leitura, pasta inexistente, duplicatas e arquivos não relacionados.
- [ ] Garantir que arquivos grandes sejam rejeitados por tamanho antes de ler seu conteúdo integral.
- [ ] Garantir que nenhum conteúdo de BIOS seja incluído em logs ou artefatos CI.

### Integração SMS
- [ ] Definir e testar o mapa integral de 8 KiB da BIOS e sua desabilitação com programa sintético.
- [ ] Testar reset, leitura antes/depois de desabilitar BIOS, transferência ao cartucho e mapper padrão.
- [ ] Testar perfil de power-on `$F3`, perfil de cartucho `$AB` e espelhamento de escritas pares em `$00-$3F`.
- [ ] Executar um programa sintético que copia o stub da BIOS para RAM antes do hand-off.
- [ ] Testar boot sem BIOS para preservar compatibilidade atual.
- [ ] Testar BIOS JP e US/EU em perfis regionais compatíveis.
- [ ] Confirmar o comportamento de jogos com e sem BIOS e evitar regressões nos testes SMS existentes.

### Interface, CI e distribuição
- [ ] Permitir escolher/alterar a pasta; mudanças na BIOS ativa aplicam-se no próximo reset ou carregamento.
- [ ] Exibir resultado da varredura explícita com hash abreviado, região e validade.
- [ ] Registrar carregamento/fallback com evento de diagnóstico sem registrar bytes da BIOS.
- [ ] Garantir que empacotamento/CI não distribua os binários.
- [ ] Executar fmt, clippy, testes Rust, lint/testes Python, build Release/Maturin e testes de integração pertinentes.
- [ ] Validar manualmente no Windows com arquivos locais do usuário.

## Plano de implementação por fatias

1. **Contrato e catálogo:** tipo de metadados/firmware, SHA-256, estados e testes de identificação.
2. **Core Rust:** entrada opcional de BIOS, overlay de memória e mecanismo de desabilitação com testes sintéticos.
3. **Fronteira PyO3 e interface:** pasta configurável, scanner, status e seleção regional explícita.
4. **Integração e validação:** fluxo completo, regressões de boot com e sem BIOS, build empacotado e testes manuais.
5. **Documentação de usuário:** formatos aceitos, diretório padrão, como instalar legalmente arquivos obtidos pelo usuário e diagnóstico de hash.

## Status desta PR

### Fatia 1 — catálogo e scanner Python

- **IMPLEMENTED:** assinaturas SHA-256 para BIOS JP e imagem compartilhada US/EU.
- **IMPLEMENTED:** varredura explícita, nomes arbitrários, tamanho exato de 8 KiB e rejeição por tamanho antes da leitura integral.
- **IMPLEMENTED:** seleção por território e revalidação do arquivo (tamanho + SHA-256) imediatamente antes do carregamento.
- **IMPLEMENTED:** duplicatas determinísticas; primeira ocorrência selecionável e cópias adicionais marcadas como `DUPLICATE_HASH`.
- **IMPLEMENTED:** inferência automática de território a partir dos códigos 3/4 de cabeçalho SMS reconhecido; cabeçalho desconhecido não provoca escolha especulativa.
- **IMPLEMENTED:** testes sintéticos sem binários de BIOS no Git.

### Fatia 2 — boot no core Rust e fronteira PyO3

- **IMPLEMENTED:** BIOS opcional de 8 KiB; mapeamento integral de 8 KiB em `$0000-$1FFF`.
- **IMPLEMENTED:** latch `$3E` com decodificação de portas pares espelhadas em `$00-$3F`.
- **IMPLEMENTED:** perfil de power-on `$F3`, RAM habilitada mantendo BIOS por `$E3`, hand-off para cartucho por `$AB`, e reset coerente com BIOS presente/ausente.
- **IMPLEMENTED:** teste sintético executa cópia do stub BIOS para RAM antes de desabilitar a BIOS e saltar ao cartucho.
- **IMPLEMENTED:** PyO3 mantém compatibilidade dos argumentos antigos e encaminha bytes opcionais sem callbacks por instrução.

### Fatia 3 — Qt/configuração

- **IMPLEMENTED:** preferência persistente `firmware_directory` e seletor de pasta na aba BIOS SMS.
- **IMPLEMENTED:** botão de varredura explícita com status, região e prefixo SHA-256 por arquivo.
- **IMPLEMENTED:** carregamento da BIOS compatível ao abrir/resetar ROM SMS; preferência manual prevalece e modo automático usa cabeçalho confiável.
- **IMPLEMENTED:** fallback para boot direto quando BIOS não está configurada ou não há correspondência segura.
- **IMPLEMENTED:** diagnóstico de BIOS carregada/fallback sem registrar bytes do firmware.

### Validação

- **QUICK_PASS / SHORT_PASS:** pendentes de confirmação no SHA final desta cadeia.
- **CI:** avaliar todos os checks aplicáveis no SHA candidato atual; checks cancelados ou de commits anteriores não contam como PASS.
- **Boot real com BIOS:** PENDING; requer firmware local do usuário e validação visual/manual.
- **TEMPORAL_PASS:** não aplicável; esta fatia não altera o scheduler ciclo a ciclo.
