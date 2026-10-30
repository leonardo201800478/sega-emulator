# Catálogo de ROMs e mídias de disco

## Objetivo

O serviço `sega_emulator.rom_catalog.RomCatalog` lê os arquivos XML DAT versionados
em `dats/` no checkout do repositório. Na distribuição setuptools, os mesmos DATs
são instalados em `share/sega-emulator/dats`, usado como fallback quando a pasta
do checkout não existe. A identificação é feita pelo conteúdo, sem depender do
nome do arquivo. O catálogo é uma camada de metadados, independente do runtime.
Uma identificação do catálogo não significa que o sistema correspondente já pode
ser executado pelo emulador.

## Correspondência por conteúdo

- O índice é carregado sob demanda e mantido em memória.
- A correspondência tenta SHA-1, depois MD5 e, por último, CRC-32 combinado com
  tamanho. CRC-32 é uma alternativa de menor força e não deve ser usado como prova
  criptográfica de identidade.
- A região vem primeiro das entradas `release region` presentes no DAT. Quando
  essas tags não existem, o parser extrai apenas rótulos reconhecidos de território
  em grupos entre parênteses do nome No-Intro, como `(Japan, Europe)`. Rótulos
  desconhecidos permanecem desconhecidos; não são inferidos pelo nome do arquivo.
- Se uma entrada tiver várias regiões ou várias correspondências, o resultado
  preserva todas as regiões encontradas; não escolhe arbitrariamente uma.
- ROMs ausentes do DAT retornam `not_catalogued`. Hacks e homebrew exigirão
  analisadores estruturais específicos por sistema ou seleção manual; o catálogo
  não deduz região pelo nome do arquivo.
- `identify_path` calcula hashes por blocos de 1 MiB e pode levar tempo em
  imagens de CD grandes. O chamador deve executá-lo fora do caminho de renderização.

## Mídias Sega CD

`inspect_disc_media` classifica ISO, CUE/BIN e CHD como mídias Sega CD e
verifica se os arquivos referenciados por um CUE existem. Um BIN isolado não é
assumido como disco, pois também pode ser uma ROM de cartucho. A inspeção não
decodifica CHD, não monta imagens e não valida a estrutura física/TOC completa.

A biblioteca de ROM existente já lista `.iso`, `.cue`, `.chd` e `.bin` como
candidatos para a biblioteca visual. Isso não equivale a suportar o carregamento
ou a execução desses formatos. A extensão de runtime e a integração da interface
serão etapas separadas, mantendo explícita a ausência de um runtime Sega CD.

## Testes

Os testes usam DATs sintéticos e pequenas imagens inventadas. Nenhuma ROM comercial
ou imagem de disco está incluída nos testes ou versionada por esta alteração.
