# Projeto: carregamento nativo de BIOS do Sega Master System

**Data:** 2026-10-29  
**Base observada:** `main` em `fad306e7461878201fe7dcf447ffe7e81e417ff3`  
**Status:** CORE_BOOT_IMPLEMENTED / FRONTEND_INTEGRATION_PENDING. O core Rust já recebe BIOS opcional, aplica overlay de 1 KiB, decodifica o latch `$3E` para BIOS/cartucho/RAM e permite boot direto sem BIOS. A seleção persistente pela interface Qt ainda está pendente.

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
- ares upstream: https://github.com/ares-emulator/ares
- perfil ares e referências ao código de varredura por hash: https://abdess.github.io/retrobios/emulators/ares/
- catálogo Sega e hashes de Master System: https://abdess.github.io/retrobios/systems/sega/
- código do core SMS atual no projeto: `native/sms_core/src/machine.rs`, `memory.rs`, `config.rs` e integração da interface.

A pesquisa do catálogo descreve a identificação por conteúdo e os arquivos conhecidos; a implementação precisa verificar diretamente a revisão de código do ares que será usada como referência antes de fechar decisões de arquitetura. Não copiar código do ares.

## Comportamento proposto

### Configuração e diretório

- Criar uma configuração persistente `firmware_directory` definida pelo usuário.
- Usar um diretório padrão dentro da área de dados/configuração do usuário, nunca dentro da pasta do repositório nem de uma pasta de ROMs.
- Expor na interface: caminho atual, selecionar pasta, varrer novamente e mostrar status por BIOS.
- Não fazer varreduras de disco inesperadas no startup; a primeira configuração deve ser explícita e o cache deve ser invalidado quando o caminho/arquivos mudarem.
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
- Quando mais de um arquivo tiver o mesmo hash conhecido, tratá-los como duplicatas da mesma imagem e não como conflito.
- A política para títulos japoneses/coreanos, títulos PAL de Austrália/Coreia e cabeçalhos ausentes precisa ser confirmada contra comportamento real do ares e testes antes de ser congelada.

### Integração de hardware

A BIOS deve ser parte do caminho de inicialização do cartucho/máquina, não uma imagem desenhada na interface. A implementação deve:
- definir um objeto de firmware validado (metadados + bytes) fora da ROM do jogo;
- encaminhar a BIOS validada pela fronteira Python/PyO3 ao core Rust sem callbacks por instrução;
- implementar overlay/mapeamento de BIOS e o mecanismo de desabilitação conforme o comportamento documentado do SMS;
- garantir que as leituras da BIOS e a desabilitação afetem o mapa de memória real visto pela CPU;
- manter a ROM de cartucho e seu mapper intactos, e permitir que o jogo passe a ser executado após a BIOS transferir controle ou ser desabilitada;
- limpar/reinicializar estado de overlay e latch no reset, de acordo com o contrato de hardware confirmado;
- evitar duplicar o carregador de ROM ou criar um segundo runtime Python de hardware.

**Não assumir que a BIOS deve estar sempre mapeada ou sempre obrigatória.** A semântica do registrador de controle e o fluxo de desabilitação precisam ser conferidos em documentação técnica e código independente antes da implementação.

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
- [ ] Testar arquivo ausente, tamanho incorreto, leitura negada, pasta inexistente, duplicatas e arquivos não relacionados.
- [ ] Garantir que nenhum conteúdo de BIOS seja incluído em logs ou artefatos CI.

### Integração SMS
- [ ] Definir e testar o mapa de memória da BIOS e seu overlay/desabilitação com programa sintético.
- [ ] Testar reset, leitura antes/depois de desabilitar BIOS, transferência ao cartucho e mapper padrão.
- [ ] Testar boot sem BIOS para preservar compatibilidade atual.
- [ ] Testar BIOS JP e US/EU em perfis regionais compatíveis.
- [ ] Confirmar o comportamento de jogos com e sem BIOS e evitar regressões nos testes SMS existentes.

### Interface, CI e distribuição
- [ ] Permitir escolher/alterar pasta e revalidar sem reinício quando seguro; mudanças de BIOS ativa devem ter política explícita de reset/reload.
- [ ] Exibir hash abreviado, região, validade e se está em uso.
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

- **IMPLEMENTED:** metadados SHA-256 para JP e imagem compartilhada US/EU.
- **IMPLEMENTED:** scanner explícito de diretório, sem varredura automática no startup.
- **IMPLEMENTED:** nomes/extensões arbitrários, validação de 8 KiB, diagnóstico de leitura/tamanho e identificação por conteúdo.
- **IMPLEMENTED:** detecção de múltiplos arquivos com o mesmo hash conhecido.
- **IMPLEMENTED:** testes sintéticos que substituem o digest; nenhum binário de BIOS é armazenado no Git.
- **QUICK_PASS / SHORT_PASS / LONG_PASS:** NOT RUN — ainda não há execução confirmada no SHA candidato.
- **CI:** NOT RUN / sem checks publicados para o SHA candidato consultado.

### Fatias pendentes

- **INTEGRATION_PENDING:** SIM.
- **Core Rust:** BIOS opcional, overlay de memória e latch de controle `$3E` ainda não implementados.
- **PyO3/Qt:** caminho de firmware persistente, seleção regional e apresentação de diagnóstico ainda não implementados.
- **Boot real com BIOS:** PENDING; requer validação local no Windows com arquivos do usuário.
- **Compatibilidade sem BIOS:** precisa de regressão automatizada após integração do core.
- **Distribuição de BIOS pelo projeto:** proibida/não pretendida.
- **TEMPORAL_PASS:** não aplicável a esta funcionalidade.
