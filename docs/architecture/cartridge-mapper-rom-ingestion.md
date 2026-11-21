# Refinamento da arquitetura de cartuchos, mappers e ingestão de ROMs

**Estado:** proposta arquitetural documental (SH-0); não altera o runtime.  
**Baseline:** `main` `94949c680e973c889580588686503c575dbc5c5b`.  
**Escopo imediato:** contratos transversais de mídia/cartucho, ingestão, identificação e seleção de mapper para os sistemas 8-bit já contemplados no roadmap.  
**Escopo futuro:** registrar limites para Mega Drive, Sega CD e 32X sem antecipar implementação 16-bit.

## 1. Objetivo

Estabelecer uma arquitetura única, extensível e baseada em evidências para carregar ROMs, identificar o hardware do cartucho e rotear transações de memória sem confundir:

- formato do arquivo com hardware do cartucho;
- identidade do jogo com identificação elétrica da placa;
- mapper com RAM persistente, periférico ou coprocessador;
- leitura de ROM com decodificação de escrita em registradores;
- identificação do mapper com boot ou compatibilidade funcional.

O objetivo é evitar seleção especulativa de mapper, falhas de boot, perda de estado em snapshots e duplicação de lógica entre sistemas.

Este documento complementa a auditoria existente [SMS — compatibilidade de mappers de cartucho](../audits/sms-cartridge-mapper-compatibility.md). Em caso de divergência sobre mappers SMS, a auditoria específica e suas evidências por perfil prevalecem.

## 2. Princípios normativos

1. **Evidência antes de inferência:** mapper desconhecido permanece `UNKNOWN` ou `AMBIGUOUS`; não aplicar fallback silencioso para Sega mapper.
2. **Identidade não é nome:** nome do arquivo, título, publisher, região e extensão não identificam hardware.
3. **Tamanho não identifica mapper:** tamanho pode eliminar hipóteses incompatíveis, mas não comprova circuito ou protocolo.
4. **Read decode e write decode são independentes:** uma região que fornece bytes de ROM em leitura pode decodificar writes para registradores.
5. **Um único dono temporal:** efeitos de barramento e de mapper permanecem ordenados pelo runtime Rust e pelo `MachineScheduler`; nenhum relógio paralelo.
6. **Separação de dispositivos:** banking, SRAM, EEPROM, Flash, periféricos e coprocessadores são componentes distintos, ainda que coexistam no mesmo cartucho.
7. **Estado completo:** reset, snapshot/restore e persistência devem incluir todos os latches e estados mutáveis necessários à retomada determinística.
8. **Compatibilidade declarada por evidência:** `MAPPER_IDENTIFIED`, `BOOT_CONFIRMED` e `GAMEPLAY_SMOKE` são estados diferentes.

## 3. Modelo em camadas

O fluxo conceitual deve separar cinco responsabilidades:

| Camada | Responsabilidade | Não deve fazer |
|---|---|---|
| Ingestão de mídia | Ler arquivo/arquivo compactado e reconhecer formato de dump | Escolher mapper pelo nome ou extensão |
| Normalização | Remover header de dumper ou de-interleave somente após validação estrutural | Alterar silenciosamente os bytes de origem |
| Identidade e evidência | Calcular tamanho, CRC32, SHA-1 quando necessário e SHA-256; consultar catálogo versionado | Tratar CRC ou título como prova isolada |
| Perfil de cartucho | Descrever mapa, mapper, dispositivos, decodificadores e evidências | Executar instruções ou avançar tempo |
| Mapper e dispositivos | Responder a transações de barramento e manter estado | Controlar CPU, Scheduler, VDP ou GUI |

### 3.1 Entidades conceituais

- **MediaImage:** bytes originais, formato detectado, transformações aplicadas e identidade calculada.
- **CartridgeIdentity:** tamanho normalizado, hashes e referências de catálogo.
- **CartridgeProfile:** associação versionada entre identidade/evidência e hardware esperado.
- **MapperProfile:** geometria de janelas, bancos, registradores, write-decode, estado inicial e regras de leitura.
- **CartridgeDevice:** SRAM, EEPROM, Flash, RTC, portas extras ou coprocessador, quando comprovados.
- **MapperState:** bancos selecionados, latches, flags, estado de dispositivos e dados necessários ao snapshot.
- **DetectionResult:** candidato, confiança, proveniência, conflitos e limitações.

Os nomes são contratos conceituais, não exigência de que essas structs sejam introduzidas imediatamente. A implementação deve reutilizar os tipos e crates existentes após inventário.

## 4. Ingestão e normalização de ROMs

### 4.1 Preservação da origem

O loader deve preservar a imagem original e produzir uma representação normalizada separada quando uma transformação for necessária. Diagnósticos devem registrar:

- formato de entrada e membro escolhido em arquivo compactado;
- tamanho original e tamanho normalizado;
- transformação aplicada e justificativa;
- hashes da imagem original e da imagem normalizada;
- avisos de padding, overdump, truncamento ou estrutura inconsistente.

A extensão do arquivo é apenas uma pista. Não basta para concluir que um arquivo é SMD, interleaved ou contém header de dumper.

### 4.2 Regras de transformação

- Remover header de 512 bytes apenas quando tamanho, estrutura e assinatura/contexto forem consistentes com o formato reconhecido.
- Executar de-interleave SMD somente após validar a organização dos blocos e a coerência do resultado.
- Não arredondar o tamanho lógico da ROM para potência de dois nem preencher automaticamente com `0xFF` como se esses bytes existissem no cartucho.
- Se o hardware exigir espelhamento, open bus ou comportamento de endereço fora da imagem, essa política pertence ao perfil de barramento/mapper, não a uma mutação genérica do arquivo.
- Padding de overdump deve ser preservado como metadado e investigado; não remover conteúdo sem critério verificável.

### 4.3 Checksums

Checksum inválido não deve, por si só, impedir o carregamento: ROM hacks, traduções e homebrews podem não recalculá-lo. O checksum continua útil como diagnóstico e metadado. A decisão de aceitar ou rejeitar uma imagem deve considerar formato, integridade estrutural e limites do sistema, não uma regra universal de checksum.

## 5. Identidade e detecção

### 5.1 Fingerprints

Manter, quando aplicável:

- tamanho exato da imagem original e normalizada;
- CRC32 para interoperabilidade com DATs e catálogos legados;
- SHA-1 quando uma fonte de catálogo o utilizar;
- SHA-256 como fingerprint local preferencial;
- metadados de cabeçalho, região declarada e offsets relevantes;
- versão do catálogo, fonte, data/revisão e nível de confiança por campo.

CRC32 isolado não deve ser tratado como identidade criptográfica. Nome e título nunca são chave suficiente.

### 5.2 Política de resolução

Ordem de resolução:

1. Identificar plataforma e formato com evidência suficiente.
2. Calcular fingerprints e consultar o catálogo de perfis.
3. Aplicar somente correspondências inequívocas segundo os critérios definidos pelo registro.
4. Verificar conflitos entre tamanho, hashes, variante e metadados.
5. Retornar `UNKNOWN` ou `AMBIGUOUS` quando a evidência não for suficiente.
6. Permitir override manual explícito e diagnosticável, sem alterar automaticamente o catálogo global.
7. Registrar separadamente o perfil selecionado, a origem da seleção e o resultado de boot/teste.

Heurísticas de análise estática ou trace podem gerar candidatos e evidências auxiliares. Não podem trocar automaticamente o mapper ativo durante a execução sem um contrato explícito, reproduzível e validado.

### 5.3 Detecção por comportamento

A observação de writes em execução é útil para diagnóstico: endereço, dado, instante de barramento, mapper candidato e efeito observado. Entretanto, procurar bytes de opcode em uma janela fixa da ROM não comprova que o jogo executará aquela instrução nem que a escrita pertence ao mapper. Código pode estar em outros bancos, usar outras instruções, saltos indiretos ou conter bytes de dados semelhantes a opcodes.

A detecção dinâmica deve permanecer uma ferramenta de investigação, não um mecanismo especulativo de reconfiguração do hardware.

## 6. Contrato do barramento e do mapper

O barramento deve encaminhar ao perfil selecionado as transações relevantes com endereço, direção, dado e contexto temporal necessário. A decisão sobre a transação pertence ao decodificador do hardware.

Separar explicitamente:

- **Read decode:** qual dispositivo responde a uma leitura e qual valor é dirigido ao barramento.
- **Write decode:** quais registradores/latches/dispositivos reconhecem uma escrita, inclusive em faixas normalmente lidas como ROM.
- **Bank translation:** conversão da janela lógica para offset físico/virtual da mídia.
- **Unmapped behavior:** open bus, pull-up ou valor definido pelo hardware específico; não generalizar uma constante para todos os consoles.
- **Reset/power-on:** valores iniciais e condições de habilitação por perfil.
- **Persistence:** exportação/importação dos dados persistentes, sem misturá-los com o estado transitório de banking.

Uma escrita não pode ser descartada apenas porque o caminho de leitura do mesmo endereço retorna ROM. O mapper também não deve aplicar efeitos fora da ordem causal estabelecida pelo Scheduler.

## 7. Escopo por sistema

### 7.1 SG-1000

O escopo atual do projeto é o console SG-1000. SC-3000 permanece fora do conjunto de plataformas-alvo, conforme `docs/project-scope.md`.

Para SG-1000, começar pelo mapa já implementado e validado. Expansões de RAM, cartuchos especiais e bancos devem ser tratados como perfis de cartucho específicos, com evidência por placa/jogo. Não assumir que toda ROM de 48 KiB, RAM adicional ou escrita em endereço baixo representa um mesmo mapper.

Dahjee Type A/Type B e multicarts devem permanecer perfis separados até que endereços, espelhamentos, estado inicial e write-decode sejam confirmados por documentação e testes reproduzíveis. A correção de mappings já integrada não deve ser reaberta sem defeito ou regressão demonstrável.

### 7.2 Master System

A auditoria `docs/audits/sms-cartridge-mapper-compatibility.md` é a referência específica existente e deve continuar sendo a fonte de verdade para os perfis documentados.

Preservar como perfis distintos, sem agrupamento genérico:

- Sega mapper;
- Codemasters;
- MSX 8 KiB e variante Nemesis;
- Korean mapper simples;
- Korea 16K V1/V2, enquanto aliases de catálogo até que a especificação elétrica esteja completa;
- Janggun;
- multicarts com protocolos diferentes;
- perfis desconhecidos ou ainda não comprovados.

O primeiro 1 KiB fixo do Sega mapper não é uma regra global para todos os mappers SMS. Escritas em janelas de ROM precisam chegar ao write-decode do mapper selecionado. O catálogo deve distinguir identificação por fingerprint, boot e gameplay.

O perfil Sega estendido de 4 MiB já possui histórico próprio de implementação e não deve ser confundido com o Sega mapper convencional nem generalizado para qualquer ROM grande.

### 7.3 Game Gear

Game Gear é sistema próprio no roadmap. Compartilhar componentes comprovadamente comuns com SMS não autoriza assumir equivalência total de memória, cartucho, save, portas, CRAM ou comportamento de boot.

Perfis de mapper e dispositivos GG devem ser identificados por evidência específica da variante GG. A compatibilidade SMS não é prova automática de compatibilidade GG.

### 7.4 Mega Drive, Sega CD e 32X — fronteira futura

Essas plataformas permanecem fora da implementação imediata, conforme a ordem normativa do roadmap. As notas do documento de referência do usuário devem ser tratadas como hipóteses de pesquisa, não como especificação de hardware aprovada.

Correções obrigatórias para qualquer etapa futura:

- ROM maior que 4 MiB não comprova, isoladamente, o uso do SSF2 mapper.
- Registros SSF2 devem ser selecionados por evidência de protocolo/placa, não apenas por tamanho.
- SRAM, EEPROM, Flash, J-Cart, SVP e proteções são dispositivos ou circuitos distintos; não reduzi-los a um mapper genérico.
- Sega CD é um subsistema com CPUs, Gate Array e memórias compartilhadas; não é um mapper de cartucho.
- 32X é um subsistema com SH-2, vídeo, barramento e arbitragem; não é apenas uma janela de ROM paginada.
- Mapeamentos, registradores, tamanhos, lanes e semântica de open bus devem ser auditados em documentação específica antes de qualquer implementação.

## 8. Persistência, snapshots e overrides

RAM de cartucho e dispositivos seriais devem possuir contratos separados:

- capacidade física e janela mapeada são propriedades distintas;
- SRAM paralela não é equivalente a EEPROM serial ou Flash;
- protocolo, tamanho, organização, write-protect e estado de comunicação precisam ser específicos do dispositivo;
- dados persistentes são exportados/importados por uma fronteira de serviço, sem acesso direto da GUI ao hardware;
- snapshots devem preservar latches e estado transitório do protocolo, além dos dados necessários à retomada;
- reset de CPU, reset de máquina e power cycle não devem ser tratados como eventos equivalentes sem evidência do hardware.

Overrides manuais devem ser explícitos, reversíveis e vinculados à identidade da imagem. Devem indicar se são apenas de sessão ou persistentes. Um override não pode reescrever silenciosamente o catálogo técnico.

## 9. Plano de refinamento e gates

| Gate | Entrega | Critério de saída |
|---|---|---|
| MAP-0 | Inventário do código e dos perfis já existentes | Localizar tipos, decodificadores, testes e consumidores; não duplicar implementação |
| MAP-1 | Contrato comum de identidade/perfil | Distinguir mídia, identidade, perfil, mapper e dispositivos |
| MAP-2 | Auditoria de write-decode | Provar que writes em janelas de ROM chegam ao mapper correto |
| MAP-3 | Matriz de estado e reset | Estado inicial, reset, snapshot/restore e persistência documentados por perfil |
| MAP-4 | Matriz de testes SMS | QUICK_PASS por mapper e SHORT_PASS por jogo com fingerprints e traces |
| MAP-5 | Perfis SG-1000 e GG | Somente após evidência específica e preservando os sistemas já validados |
| MAP-6 | Loader robusto | Testes de formatos, arquivos compactados, normalização, limites e diagnósticos |
| MAP-7 | Extensão 16-bit | Iniciar somente quando a ordem de plataformas do roadmap permitir |

### 9.1 QUICK_PASS por perfil

- Estado inicial e reset.
- Limites de cada janela e tradução para offsets.
- Endereços canônicos e mirrors documentados.
- Read-decode e write-decode independentes.
- Primeiro 1 KiB fixo somente nos perfis em que isso é comprovado.
- ROM parcial, bancos fora do limite e wrap com comportamento explícito.
- RAM/dispositivos não confundidos com RAM interna do console.
- Snapshot/restore e execução particionada sem duplicar efeitos.

### 9.2 SHORT_PASS por imagem

Registrar fingerprint, perfil selecionado, proveniência, trace de writes, primeiro fetch, boot, tela/menu e ao menos uma transição que exercite banking. Comparações com Ares, MAME ou Genesis Plus GX são evidência comparativa, não prova física isolada. Registrar versão do oráculo e divergências.

Estados de aceite permanecem distintos: `MAPPER_IDENTIFIED`, `BOOT_CONFIRMED`, `GAMEPLAY_SMOKE` e `VALIDATED`.

## 10. Limites desta PR

Esta proposta não:

- altera código Rust, CPU, barramento, Scheduler, VDP, áudio ou GUI;
- adiciona mappers, dispositivos ou detecção heurística ao runtime;
- declara suporte a Mega Drive, Sega CD ou 32X;
- altera ou substitui a auditoria SMS existente;
- inclui ROMs, BIOS, dumps comerciais ou dados protegidos;
- declara boot, gameplay, precisão temporal ou compatibilidade funcional.

A próxima etapa de implementação depende do MAP-0: inventariar o estado real da `main` e reconciliar cada perfil com os tipos e testes existentes. Não iniciar uma reescrita geral de mappers antes desse inventário.

## 11. Referências internas

- [Escopo e roadmap normativos](../project-scope.md)
- [Arquitetura geral](../architecture.md)
- [SMS — compatibilidade de mappers de cartucho](../audits/sms-cartridge-mapper-compatibility.md)
- [Documentação do SMS Power — Mappers](https://www.smspower.org/Development/Mappers)
- [SMS Power — Memory Map](https://www.smspower.org/Development/MemoryMap)

**Conclusão:** o projeto deve tratar cartucho como uma composição de mídia, identidade, perfil de hardware, mapper e dispositivos. A detecção precisa ser conservadora, o barramento precisa distinguir leitura de escrita e toda mudança de estado deve permanecer determinística e pertencente ao runtime Rust.
