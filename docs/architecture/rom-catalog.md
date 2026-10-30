# Catálogo de ROMs e mídias de disco

## Objetivo

O serviço `sega_emulator.rom_catalog.RomCatalog` lê os arquivos XML DAT da pasta
raiz `dats/` e permite identificar imagens por conteúdo, sem depender do nome
do arquivo. Ele é uma camada de metadados, independente do runtime de hardware.
Uma identificação do catálogo não significa que o sistema correspondente já pode
ser executado pelo emulador.

## Correspondência por conteúdo

- O índice é carregado sob demanda e mantido em memória.
- A correspondência tenta SHA-1, depois MD5 e, por último, CRC-32 combinado com
  tamanho. CRC-32 é uma alternativa de menor força e não deve ser usado como prova
  criptográfica de identidade.
- A região vem das entradas `release region` presentes no DAT. Se uma entrada
  tiver várias regiões ou várias correspondências, o resultado preserva todas
  as regiões encontradas; não escolhe arbitrariamente uma.
- ROMs ausentes do DAT retornam `not_catalogued`. Hacks e homebrew exigirão
  analisadores estruturais específicos por sistema ou seleção manual; o catálogo
  não deduz região pelo nome do arquivo.
- `identify_path` calcula hashes por blocos de 1 MiB e pode levar tempo em
  imagens de CD grandes. O chamador deve executá-lo fora do caminho de renderização.

## Integração com metadados técnicos do Master System

O DAT No-Intro continua sendo a fonte de identidade e regiões de release; não é
uma fonte suficiente para mapper, SRAM, FM, periféricos ou requisitos de BIOS.
Essas características serão descritas por um catálogo técnico SMS separado,
versionado e associado a identidades exatas, com proveniência e confiança por
campo. A arquitetura e a política de ROM não identificada estão em
[SMS ROM metadata, compatibilidade de hardware e OSD](sms-rom-metadata-and-osd.md).

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
