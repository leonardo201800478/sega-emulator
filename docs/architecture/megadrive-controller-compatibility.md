# Compatibilidade de controles do Mega Drive/Genesis

## Política desejada

- Perfil padrão de emulação para Mega Drive/Genesis: **controle de 6 botões**.
- Para ROMs verificadas que apresentem incompatibilidade real, selecionar automaticamente o perfil de 3 botões por meio de um catálogo de exceções.
- Permitir substituição manual por jogo e oferecer diagnóstico explicando qual regra selecionou o perfil.
- Não inferir incompatibilidade apenas porque o jogo não usa X/Y/Z/Mode; a exceção é para comportamento quebrado ou incompatibilidade reproduzível.

Esta política é uma decisão do projeto. A compatibilidade de cada jogo ainda precisa ser validada individualmente; nenhum jogo deve entrar no catálogo definitivo apenas por um relato isolado.

## Identificação da ROM

CRC32 será o identificador de consulta exigido para o catálogo, mas não é globalmente único. Armazenar SHA-1 complementar (e opcionalmente SHA-256 para auditoria) para reduzir ambiguidades e permitir confirmar a imagem testada.

Antes de calcular hashes:

- Ler os bytes efetivamente carregados pelo emulador.
- Remover cabeçalhos externos de contêineres/formato apenas quando o formato os definir; nunca remover bytes por heurística.
- Converter imagens SMD/interleaved para a representação linear da ROM antes de calcular o hash canônico.
- Não remover automaticamente o cabeçalho interno do cartucho Mega Drive; documentar separadamente qualquer normalização que venha a ser comprovada necessária.
- Manter metadados do arquivo original (nome/tamanho/formato) apenas para diagnóstico; nome de arquivo não é chave de compatibilidade.
- Calcular o CRC sobre a representação canônica e registrar algoritmo, tamanho dos bytes processados e procedimento de normalização.

## Esquema proposto do catálogo

Cada entrada de exceção deve conter:

- `system`: `megadrive`;
- `crc32`: checksum hexadecimal calculado sobre bytes canônicos;
- `sha1`: hash complementar da mesma representação;
- `rom_size`: tamanho da representação canônica;
- `title`: nome do jogo;
- `region` e `revision`: quando conhecidos;
- `recommended_controller`: `3-button`;
- `evidence_status`: `reported`, `reproduced` ou `verified`;
- `evidence`: fonte, versão de emulador/hardware, passos para reproduzir e resultado;
- `notes`: comportamento observado e quaisquer condições (por exemplo, estado do botão Mode na inicialização).

Não adicionar CRCs inventados nem preencher valores de checksum a partir de nomes de ROM. O catálogo começa vazio ou com entradas marcadas explicitamente como não verificadas até que hashes e evidências sejam obtidos.

## Resolução de configuração

Ordem de precedência:

1. Substituição manual explícita do usuário.
2. Exceção correspondente por CRC32 e hash complementar, com correspondência inequívoca.
3. Padrão de 6 botões.

Se CRC32 corresponder mas SHA-1/tamanho/metadados entrarem em conflito, não aplicar silenciosamente a exceção; registrar ambiguidade e usar o padrão ou pedir seleção manual. A interface deve mostrar perfil selecionado, origem da regra e a possibilidade de substituição.

## Modelo de entrada de exemplo (não é uma exceção real)

```json
{
  "system": "megadrive",
  "crc32": "00000000",
  "sha1": "0000000000000000000000000000000000000000",
  "rom_size": 0,
  "title": "EXEMPLO — NÃO UTILIZAR",
  "region": null,
  "revision": null,
  "recommended_controller": "3-button",
  "evidence_status": "reported",
  "evidence": [],
  "notes": "Entrada ilustrativa; checksum fictício, não é uma ROM catalogada."
}
```

## Validação das exceções

Para promover uma entrada a `verified`:

1. Identificar dump, região e revisão da ROM legalmente obtida.
2. Calcular CRC32 e SHA-1 com procedimento documentado.
3. Reproduzir a falha com controle de 6 botões no hardware ou em pelo menos uma implementação de referência bem documentada.
4. Confirmar se selecionar o perfil de 3 botões ou manter Mode pressionado durante reset realmente resolve a falha.
5. Testar a entrada catalogada com a mesma imagem e documentar as condições.
6. Confirmar que jogos diferentes/revisões não são afetados pela mesma regra.

## Candidato inicial de pesquisa — não é exceção aprovada

Forgotten Worlds é relatado por fontes comunitárias como incompatível ou problemático com controle de 6 botões; há relatos de que manter Mode pressionado durante a inicialização ajuda em algumas configurações. O comportamento exato varia entre fontes, então o jogo é apenas um candidato à investigação. Nenhum CRC é incluído até que a ROM/revisão específica seja verificada.

## Testes automatizados necessários

- ROM binária e imagem SMD equivalente resultam no mesmo hash canônico após normalização correta.
- Cabeçalho de contêiner removido apenas conforme especificação do formato.
- CRC32 conhecido e SHA-1 conhecido conferem com vetores de teste.
- Perfil padrão é 6 botões quando não há correspondência.
- Exceção verificada seleciona 3 botões.
- Substituição manual prevalece sobre a exceção automática.
- CRC32 duplicado com SHA-1/tamanho divergente gera ambiguidade e não seleciona uma exceção silenciosamente.
- Catálogo inválido ou desconhecido não impede o carregamento da ROM.

## Referências

- Comunidade Mega Drive, relato de compatibilidade de controles: https://www.comunidademegadrive.com.br/2024/02/controle-de-seis-botoes/
- VGDB, lista comunitária de compatibilidade de controles: https://www.vgdb.com.br/mega-drive/jogos/joystick-6-botoes-para-mega-drive/
- Genesis Plus GX, projeto de emulação para comparação de políticas de entrada: https://github.com/ekeeke/Genesis-Plus-GX

Relatos comunitários são fontes de pistas, não comprovação normativa. Entradas definitivas exigem checksum real, revisão identificada e reprodução do comportamento.