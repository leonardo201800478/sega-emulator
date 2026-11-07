# SMS SRAM — auditoria de mapeamento e identidade de cartuchos

**Data da auditoria:** 2026-11-07
**PR de implementação:** #643
**SHA de código auditado nesta revisão:** `e6a0384036ee3b13aa45324904988219d41fa6e0`
**Base remota:** `f47f71830344b85f72329376505d400bfbfaae69`
**Estado:** mapeamento, catálogo inicial, store host-side e integração do carregador implementados na PR; validação CI do HEAD final ainda pendente.

## 1. Evidências e proveniência

### Fontes primárias e independentes

1. SMS Power — [Mappers](https://www.smspower.org/Development/Mappers): bits 4, 3 e 2 de `$FFFC`, janela alta e janela do slot 2; registers write-only e RAM subjacente.
2. SMS Power — [Memory Map](https://www.smspower.org/Development/MemoryMap): mapa `$C000-$DFFF` e espelho `$E000-$FFFF`; sobreposição dos registradores.
3. SMS Power — [RAM](https://www.smspower.org/Development/RAM): 8 KiB de work RAM, espelhamento, porta `$3E` bit 4 active-low para habilitação da RAM interna; desabilitar para evitar contenção com RAM externa.
4. SMS Power Forums — [Official SMS documentation of mapper register $FFFC](https://www.smspower.org/forums/6807-OffficalSMSDocumentationOfMapperRegisterFffc): transcrição do manual oficial Sega e explicação de Charles MacDonald sobre bit 4 e contenção de barramento.
5. SMS Plus GX — [source/sms.c](https://github.com/ekeeke/smsplus-gx/blob/master/source/sms.c), função `mapper_16k_w`: remapeamento de slot 2 via bit 3, página SRAM via bit 2 e janela alta via bit 4; as escritas em `$FFFC-$FFFF` são primeiro decodificadas pelo mapper e depois gravadas na memória mapeada.
6. MAME — [hash/sms.xml](https://github.com/mamedev/mame/blob/master/hash/sms.xml), branch `master`, blob SHA `5410f81bc8e1900375d076f2f7ee42ee852c2c48`, consultado em 2026-11-07. Fornece entradas de software, tamanho, CRC32, SHA-1, placa/IC quando conhecido e feature `battery`.

### Limites das evidências

- A página Mappers é a especificação de referência para a função dos bits.
- SMS Plus GX é uma implementação comparativa; não é substituto de documentação elétrica.
- O MAME usa `feature battery=yes` como indicador de memória persistente no software list; isso não prova, isoladamente, a existência de bateria física. Flight of Pigarus é o contraexemplo documentado: usa FeRAM sem bateria.
- O DAT No-Intro SMS versionado no próprio projeto foi localizado: `dats/Sega - Master System - Mark III (Parent-Clone) (20261013-064924).dat`, versão `20261013-064924`, blob SHA `9528da45f7e341ff642164ddbd58f93dfc353338`.
- O DAT do projeto foi confrontado por SHA-1, CRC32 e tamanho com MAME `hash/sms.xml`, blob SHA `5410f81bc8e1900375d076f2f7ee42ee852c2c48`. Os fingerprints coincidem nas variantes comuns, protótipos e variantes regionais cobertas por ambas as bases. Exceções sem par MAME são explicitamente mantidas separadas.
- O DAT contém MD5 além de SHA-1/CRC32/tamanho. O catálogo deverá usar SHA-1 para correspondência DAT e calcular SHA-256 diretamente dos bytes carregados para identidade de armazenamento; não é necessário inventar SHA-256 ausente no DAT.
- Os arquivos técnicos locais enviados nesta conversa não contêm uma especificação de mapper/cartucho SMS; os documentos Z80 são manuais de CPU. A semântica de mapper foi obtida das referências SMS Power e da implementação comparativa SMS Plus GX.

## 2. Matriz de conformidade do Sega mapper

| Endereço/controle | Condição | Leitura | Escrita / efeito no mapper | Efeito em SRAM | Efeito em work RAM | Teste |
|---|---|---|---|---|---|---|
| `$FFFC` | Cartucho habilitado (`$3E` bit 6 = 0) | Retorna a memória subjacente, não o latch | Atualiza controle SRAM e bank-shift | Se bit 4 já estiver ativo e janela alta selecionada, a escrita alcança a SRAM após o remapeamento; bit 3 pode selecionar slot 2 | Se RAM interna habilitada, escrita atualiza o espelho interno | `mapper_register_overlay_reads_and_writes_the_selected_high_window_ram`; `mapper_register_writes_also_update_the_ram_mirror` |
| `$FFFD` | Cartucho habilitado | Retorna memória subjacente | Atualiza banco do slot 0 | Se janela alta estiver mapeada, escrita também chega ao offset correspondente da SRAM | Se work RAM habilitada, atualiza byte espelhado | `mapper_register_overlay_reads_and_writes_the_selected_high_window_ram`; `maps_fixed_first_kib_and_three_banked_slots` |
| `$FFFE` | Cartucho habilitado | Retorna memória subjacente | Atualiza banco do slot 1 | Se janela alta estiver mapeada, escrita também chega à SRAM | Se work RAM habilitada, atualiza byte espelhado | `mapper_register_overlay_reads_and_writes_the_selected_high_window_ram` |
| `$FFFF` | Cartucho habilitado | Retorna memória subjacente | Atualiza banco do slot 2, salvo quando SRAM ocupa o slot 2 | Se bit 3 = 1, SRAM responde em slot 2; se bit 4 = 1, janela alta também usa SRAM | Se work RAM habilitada, atualiza byte espelhado | `mapper_register_overlay_reads_and_writes_the_selected_high_window_ram`; testes de bancos |
| `$8000-$BFFF` | Sega mapper, bit 3 de `$FFFC` = 1 | SRAM em vez de ROM | Escritas na SRAM | Bit 2 escolhe banco de 16 KiB; 8 KiB espelha; 16 KiB faz alias do banco inexistente; 32 KiB seleciona duas páginas | Sem efeito na work RAM | testes existentes de 8/16/32 KiB |
| `$C000-$FFFF` | `$3E` bit 4 = 0 | Work RAM interna | Escritas na work RAM | SRAM externa não é exposta pela janela alta | 8 KiB espelhados em toda a faixa de 16 KiB | `mirrors_work_ram_across_the_upper_address_window` |
| `$C000-$FFFF` | `$3E` bit 4 = 1, cartucho habilitado, Sega mapper, `$FFFC` bit 4 = 1 | SRAM externa, janela de 16 KiB | Escritas na SRAM externa | Sempre expõe os primeiros 16 KiB: 8 KiB espelha; 16 KiB ocupa a janela; 32 KiB expõe a página baixa | Work RAM interna desabilitada e preservada, não apagada | `mapper_bit_four_maps_cartridge_sram_into_high_window_when_system_ram_is_disabled`; `mapper_bit_four_high_window_mirrors_eight_kib_and_uses_lower_page_of_32k` |
| `$C000-$FFFF` | `$3E` bit 4 = 1 e `$FFFC` bit 4 = 0 | Open bus | Sem escrita em work RAM ou SRAM | SRAM não selecionada | Work RAM interna desabilitada | `memory_control_port_can_disable_and_reenable_work_ram` |
| Qualquer endereço de cartucho | `$3E` bit 6 = 1 | Cartucho desconectado; BIOS segue seu próprio bit de habilitação | Escritas não alteram mapper/SRAM | Não acessível | RAM interna continua regida pelo bit 4 de `$3E` | `disabled_cartridge_slot_does_not_receive_mapper_register_writes`; `high_window_sram_is_not_visible_when_system_ram_or_cartridge_is_disabled` |
| `$0000-$1FFF` | BIOS presente e `$3E` bit 3 = 0 | BIOS responde | Escritas não alteram mapper do cartucho quando slot está desabilitado | Não acessível | Work RAM independente | `bios_maps_all_eight_kib_and_can_be_disabled`; `disabled_cartridge_slot_does_not_receive_mapper_register_writes` |
| `$FFF8-$FFFB` | SegaScope/3D glasses, se presente | Dispositivo específico sobrepõe a RAM espelhada | Controle de periférico | Não é SRAM mapper | Região também coincide com espelho de RAM | Ainda não há teste de SegaScope neste core |

### Decodificação explícita de `$FFFC`

| Bit | Função documentada | Decodificação |
|---:|---|---|
| 7 | ROM write enable (dev boards) | Não implementado nesta tarefa; fora dos cartuchos comerciais listados |
| 6-5 | Não utilizados | Preservados no latch |
| 4 | RAM externa em `$C000-$FFFF` | Mapeia os primeiros 16 KiB da SRAM na janela alta; requer work RAM interna desabilitada via `$3E` bit 4 para evitar contenção |
| 3 | RAM externa em `$8000-$BFFF` | Substitui ROM do slot 2 |
| 2 | Seleção de banco SRAM | Seleciona o banco de 16 KiB somente no slot 2; não altera a página da janela alta |
| 1-0 | Bank shift | Função especial de revisão do mapper; não é seleção de SRAM |

### Prioridade e contenção

A combinação `$FFFC` bit 4 = 1 com `$3E` bit 4 = 0 deixa work RAM interna e RAM externa simultaneamente selecionadas, condição que SMS Power recomenda evitar por contenção elétrica. O modelo atual mantém work RAM como resposta enquanto a RAM interna está habilitada e só expõe SRAM externa depois que a RAM interna é desabilitada. Isso é uma política determinística conservadora; não afirma reproduzir o valor elétrico de um barramento em contenção.

As leituras em `$FFFC-$FFFF` não retornam registradores. Retornam a memória efetivamente selecionada na janela alta: work RAM interna quando habilitada; SRAM externa quando a janela alta está selecionada e a work RAM interna está desabilitada; open bus quando nenhuma delas responde. As escritas continuam atualizando os registradores de mapper e também a memória subjacente selecionada.

## 3. Perfis SRAM — candidatos de identidade do MAME

O MAME atual lista os seguintes registros com `battery=yes`. Os hashes abaixo são SHA-1 e CRC32 da ROM exata indicada pelo próprio MAME. Tamanho está em bytes. A lista é uma referência de identificação, não ainda o catálogo automático do projeto.

| Jogo / variante MAME | Tamanho | CRC32 | SHA-1 | Evidência MAME |
|---|---:|---|---|---|
| Golden Axe Warrior (Europe, USA, Brazil) | 262144 | `C7DED988` | `FBDA0486B393708A89756BB57D116AD6007484E4` | battery=yes; PCB 171-5602-B |
| Golfamania (Europe, Brazil) | 262144 | `48651325` | `D0B964DD7CD8CCDD730DE4D8E4BB2E87BEA7686E` | battery=yes; IC1 MPR-13144 |
| Golfamania (prototype) | 262144 | `5DABFDC3` | `DA88DC3E84DAA2F8B8D803B00A13B5FB3185D8C5` | battery=yes; prototype separado |
| Miracle Warriors (Europe, USA, Brazil) | 262144 | `0E333B6E` | `F952406BCA4918EE91A89B27E949E224EAE96D85` | battery=yes |
| Miracle Warriors (prototype) | 262144 | `301A59AA` | `5F1CFC974A8FFC965F18216A89472FD1A980D7CB` | battery=yes; prototype separado |
| Monopoly (Europe) | 131072 | `026D94A4` | `6BAD7176011DD4BBD007498D167399DAACDE173D` | battery=yes |
| Monopoly (USA) | 131072 | `69538469` | `8DD7BB4F666F70F7F57687823D0068C9100AF8E5` | battery=yes |
| Monopoly (USA, prototype 1) | 131072 | `E0D1049B` | `6D100D43CDDFB761D3560786C26E345873452A4B` | battery=yes; prototype separado |
| Monopoly (USA, prototype alt) | 131072 | `7E9D87FC` | `68830FB46A010CF2F2D04BDC69E3759F6D42B8DE` | battery=yes; prototype separado |
| Doki Doki Penguin Land (Japan) | 131072 | `2BCDB8FA` | `C01CF44EEE335D509DC20A165ADD8514E7FBB7C4` | battery=yes; variante japonesa |
| Doki Doki Penguin Land (Japan, prototype) | 131072 | `56BD2455` | `C56417EFD78A332A35E4D0E87C7B13C20DA85391` | battery=yes; prototype separado |
| Penguin Land (Europe, USA) | 131072 | `F97E9875` | `8762239C339A084DFB8443CC38515301476BDE28` | battery=yes; IC1 315-5235 |
| Phantasy Star (Europe, USA, v3) | 524288 | `00BEF1D7` | `07FCF297BE4F4C9D92CD3F119A7AC48467E06838` | battery=yes; SRAM IC3 MB8464A-15LL-SK |
| Phantasy Star (Europe, USA, v2) | 524288 | `E4A65E79` | `257CA76EBCD54C75A414CA7CE968FA59EA42F150` | battery=yes |
| Phantasy Star (Brazil) | 524288 | `75971BEF` | `FD8DAD6ACB6FA75DC8E9BBAEA2A7E9FD486FC2DD` | battery=yes; Tec Toy |
| Phantasy Star (Japan) | 524288 | `6605D36A` | `C9A40DDD217C58DDDCD6B5C0FE66C3A50D3E68E4` | battery=yes |
| Phantasy Star (Japan, Mega Drive/Saturn Collection CD) | 524288 | `07301F83` | `B3AE447DC739256616B44CBD77CB903C9F19E980` | battery=yes; distinct repack |
| Phantasy Star (Korea) | 524288 | `747E83B5` | `52B2AA52A1C96E15869498A8E42B074705070007` | battery=yes; regional variant |
| Ultima IV (Europe, Brazil) | 524288 | `B52D60C8` | `A90E21E5961BCF2E10B715A009C04E7C2017A3B1` | battery=yes; IC1 MPR-13135 |
| Ultima IV (Europe, prototype) | 524288 | `DE9F8517` | `BB1AE06B62A9F7D3259C51EEE4CFDED781EB5D30` | battery=yes; prototype separado |
| Ys (Japan) | 262144 | `32759751` | `614B589080B732E17CC0D253E17216A72A268955` | battery=yes |
| Ys - The Vanished Omens (Europe, USA, Brazil) | 262144 | `B33E2827` | `E73E836C353543E9F48315410B0D72278899FF59` | battery=yes; PCB/IC não identificados |

**Importante:** `battery=yes` prova que o registro MAME está classificado como cartucho com bateria; não prova, isoladamente, que todos os registros usem exatamente 8 KiB. A capacidade de 8 KiB está documentada no material histórico do projeto como configuração usual, e Phantasy Star v3 identifica explicitamente uma SRAM MB8464A de 8K×8. Para os demais registros, exigir confirmação da placa/IC ou de outra fonte independente antes de fixar a capacidade por variante.

**Variantes não incluídas automaticamente:** demos/protótipos sem `battery=yes`, como Ys USA demo, permanecem fora do perfil persistente. Não herdar o perfil do título comercial por semelhança de nome.

### Confronto DAT do projeto ↔ MAME

O DAT SMS versionado no projeto está na versão `20261013-064924`. MAME `hash/sms.xml` foi consultado no blob `5410f81bc8e1900375d076f2f7ee42ee852c2c48`. Os registros com mesmo SHA-1 têm também tamanho e CRC32 coincidentes.

O MAME informa `dataarea ram size=8192` e `feature battery=yes` para os cartuchos comerciais das oito famílias, confirmando 8 KiB de RAM persistente no modelo do software list. Phantasy Star v3 identifica ainda o IC `MB8464A-15LL-SK`, uma SRAM 8K×8. As entradas protótipo que também aparecem no DAT têm fingerprints próprios e não devem herdar identidade por nome.

| Família / variante | Tamanho | CRC32 | SHA-1 | MAME |
|---|---:|---|---|---|
| Golden Axe Warrior (USA, Europe, Brazil) | 262144 | `C7DED988` | `FBDA0486B393708A89756BB57D116AD6007484E4` | battery=yes; RAM 8192 |
| Golfamania (Europe, Brazil) | 262144 | `48651325` | `D0B964DD7CD8CCDD730DE4D8E4BB2E87BEA7686E` | battery=yes; RAM 8192 |
| Golfamania (Beta) | 262144 | `5DABFDC3` | `DA88DC3E84DAA2F8B8D803B00A13B5FB3185D8C5` | battery=yes; RAM 8192 |
| Super Golf (Europe) (Beta) | 262144 | `7F7B568D` | `EBE733EA693FA46EDDE43260FDB963CAB71317E1` | battery=yes; RAM 8192; protótipo distinto |
| Miracle Warriors (USA, Europe, Brazil) | 262144 | `0E333B6E` | `F952406BCA4918EE91A89B27E949E224EAE96D85` | battery=yes; RAM 8192 |
| Haja no Fuuin (Japan) | 262144 | `B9FDF6D9` | `46A032004D49FEC58099AA6BF0DD796997E95142` | battery=yes; RAM 8192; variante japonesa distinta |
| Miracle Warriors (Beta) | 262144 | `301A59AA` | `5F1CFC974A8FFC965F18216A89472FD1A980D7CB` | battery=yes; RAM 8192; protótipo distinto |
| Monopoly (USA, Europe) | 131072 | `69538469` | `8DD7BB4F666F70F7F57687823D0068C9100AF8E5` | battery=yes; RAM 8192 |
| Monopoly (USA, Europe) (Rev A) | 131072 | `026D94A4` | `6BAD7176011DD4BBD007498D167399DAACDE173D` | battery=yes; RAM 8192 |
| Monopoly (Beta 1) | 131072 | `7E9D87FC` | `68830FB46A010CF2F2D04BDC69E3759F6D42B8DE` | battery=yes; RAM 8192; protótipo distinto |
| Monopoly (Beta 2) | 131072 | `E0D1049B` | `6D100D43CDDFB761D3560786C26E345873452A4B` | battery=yes; RAM 8192; protótipo distinto |
| Penguin Land (USA, Europe) | 131072 | `F97E9875` | `8762239C339A084DFB8443CC38515301476BDE28` | battery=yes; RAM 8192 |
| Doki Doki Penguin Land (Japan) | 131072 | `2BCDB8FA` | `C01CF44EEE335D509DC20A165ADD8514E7FBB7C4` | battery=yes; RAM 8192 |
| Doki Doki Penguin Land (Japan) (Beta) | 131072 | `56BD2455` | `C56417EFD78A332A35E4D0E87C7B13C20DA85391` | battery=yes; RAM 8192; protótipo distinto |
| Phantasy Star (USA, Europe) (Rev A) | 524288 | `00BEF1D7` | `07FCF297BE4F4C9D92CD3F119A7AC48467E06838` | battery=yes; RAM 8192; IC SRAM identificado |
| Phantasy Star (USA, Europe) | 524288 | `E4A65E79` | `257CA76EBCD54C75A414CA7CE968FA59EA42F150` | battery=yes; RAM 8192 |
| Phantasy Star (Brazil) | 524288 | `75971BEF` | `FD8DAD6ACB6FA75DC8E9BBAEA2A7E9FD486FC2DD` | battery=yes; RAM 8192 |
| Phantasy Star (Japan) | 524288 | `6605D36A` | `C9A40DDD217C58DDDCD6B5C0FE66C3A50D3E68E4` | battery=yes; RAM 8192 |
| Phantasy Star (Korea) (Unl) | 524288 | `747E83B5` | `52B2AA52A1C96E15869498A8E42B074705070007` | battery=yes; RAM 8192 |
| Phantasy Star (Japan) (Rev 1) | 524288 | `07301F83` | `B3AE447DC739256616B44CBD77CB903C9F19E980` | battery=yes; RAM 8192; MAME dump status baddump |
| Ultima IV - Quest of the Avatar (Europe, Brazil) | 524288 | `B52D60C8` | `A90E21E5961BCF2E10B715A009C04E7C2017A3B1` | battery=yes; RAM 8192 |
| Ultima IV (Beta) | 524288 | `DE9F8517` | `BB1AE06B62A9F7D3259C51EEE4CFDED781EB5D30` | battery=yes; RAM 8192; protótipo distinto |
| Ys (Japan) | 262144 | `32759751` | `614B589080B732E17CC0D253E17216A72A268955` | battery=yes; RAM 8192 |
| Ys - The Vanished Omens (USA, Europe, Brazil) | 262144 | `B33E2827` | `E73E836C353543E9F48315410B0D72278899FF59` | battery=yes; RAM 8192 |

**Variantes separadas, sem associação automática:**
- `Astro Adventure (Taiwan) (En) (Unl)` está no DAT (SHA-1 `E3DA56644C098D07947B1FFD9465002616A69D30`) mas não tem entrada MAME correspondente nem evidência de SRAM/bateria; não atribuir perfil.
- `Phantasy Star (World) (En/Ja) (Sega Ages)` são imagens de 540672/565248 bytes sem par MAME; não herdar perfil das ROMs de 512 KiB.
- `Ys - The Vanished Omens (USA, Europe) (Ja) (Demo)` tem SHA-1 `B70757876E4FF592692EB0EE1C51C571B31541A9`; MAME não marca battery nem área RAM. Não atribuir perfil persistente.
- Não foi encontrada variante Beta de Golden Axe Warrior; não inventar.

## 4. Flight of Pigarus — perfil isolado e BLOCKED

O DAT SMS do projeto lista as mesmas três ROMs; SHA-1, CRC32 e tamanho coincidem com MAME. O MAME lista `dataarea ram size=32768` e `battery=yes` para as três variantes:

| Variante | Tamanho | CRC32 | SHA-1 | MAME |
|---|---:|---|---|---|
| v1.11 | 262144 | `60B86F18` | `46941B6CA4C38B07FA55987D3F30C249D5674E2A` | RAM 32768; dump digital não verificado contra cartucho físico |
| v1.10 | 262144 | `1950005D` | `3BD03C3BE402B90E61E6BE5B67216C0C6D8056E6` | RAM 32768 |
| v1.00 | 262144 | `838574D1` | `E53A16F5BF4A2D804B26C359CF3A727C06379E6D` | RAM 32768 |

A evidência independente agora confirma que o jogo utiliza duas páginas lógicas de 16 KiB: o autor Kagesan afirma que os 15.000 frames de replay cabem em uma página de 16 KiB e que o high score comprimido é armazenado na outra; Ichigo, responsável pela PCB, afirma que precisou alterar mapper e SRAM para disponibilizar duas páginas. Isso sustenta 32 KiB de armazenamento acessível pelo software, mas não revela os endereços de decodificação nem prova que o circuito é idêntico ao Sega mapper padrão.

A PCB usa FeRAM Lapis MR48V256 de 256 Kibit (32 KiB), segundo a nota de hardware no MAME e a declaração do fabricante da placa. FeRAM é não volátil e não exige bateria; portanto, o campo MAME `battery=yes` não deve ser interpretado aqui como prova de bateria física. O próprio MAME registra que o dump digital v1.11 não foi verificado contra o cartucho físico e questiona se os 32 KiB são integralmente acessíveis.

**Estado: BLOCKED** para perfil executável até confirmar por documentação elétrica, disassembly ou trace: decodificação de endereço, mecanismo exato de seleção das duas páginas, comportamento de leitura/escrita e equivalência entre dump digital e PCB. Registrar provisoriamente como `NONVOLATILE_FRAM`, não como SRAM battery-backed, e não incluir no perfil genérico.

Evidências:
- Kagesan, desenvolvedor — [Flight of Pigarus development Q&A](https://www.smspower.org/forums/17278-FlightOfPigarusDevelopmentQA), mensagem de 04/12/2018: 16 KiB para replay e outra página para high score.
- Ichigo, designer da PCB — [Flight of Pigarus discussion](https://www.smspower.org/forums/17274-HomebrewGameFlightOfPigarus?start=50), mensagem de 02/01/2019: necessidade de mapper e SRAM com duas páginas.
- 2Minds — [Flight of Pigarus physical production](https://www.smspower.org/forums/18169-2MindsStudioPublisher): FeRAM 256 Kibit sem bateria.
- MAME `hash/sms.xml`, entrada `pigarus`: `dataarea ram size=32768`, parte Lapis MR48V256 e ressalva sobre o dump digital.

## 5. RAM volátil — não gerar .srm

| Jogo / variante | Identidade DAT SMS do projeto | Evidência MAME | Tratamento |
|---|---|---|---|
| Desert Speedtrap (Europe, Brazil) | 262144; CRC `B137007A`; SHA-1 `60E2B6EC69D73DD73C1EF846634942C81800655B` | `dataarea ram size=8192`; sem `battery=yes` | RAM volátil 8 KiB candidata; não gerar .srm |
| Flash, The (Europe, Brazil) | 262144; CRC `BE31D63F`; SHA-1 `1732B4C13FD00DD5EFC5BF1CCB1AB6ED3889C8BA` | `dataarea ram size=8192`; sem `battery=yes` | RAM volátil 8 KiB candidata; não gerar .srm |
| Terminator, The (Europe) | 262144; CRC `EDC5C012`; SHA-1 `EAF733D385E61526B90C1B194BF605078D43E2D3` | PCB 171-6339A; IC2 `KM62256BLP-10L`; `dataarea ram size=32768`; sem `battery=yes` | RAM volátil 32 KiB candidata; não gerar .srm |
| Terminator, The (Brazil) | 262144; CRC `E3D5CE9A`; SHA-1 `BEA0A1D0271FAA2273E4CF1E968241D018C81DA1` | `dataarea ram size=32768`; sem IC/PCB detalhados | RAM volátil 32 KiB indicada pelo MAME; manter identidade e proveniência separadas |
| Othello | Sem entrada no DAT SMS do projeto | Não consta como cartucho SMS no MAME; documentação do projeto o lista no contexto SG-1000 | Não cadastrar como SRAM de cartucho SMS |

A ausência de `battery=yes` não prova ausência de RAM: MAME lista RAM de 8 KiB em Desert Speedtrap/Flash, The e RAM de 32 KiB em The Terminator sem flag de persistência. Da mesma forma, `battery=yes` é um marcador do software list MAME para dados persistentes e não prova sozinho que a PCB possua uma bateria física; Flight of Pigarus é o contraexemplo documentado, com FeRAM.

## 6. Arquitetura implementada na PR #643

### `CartridgeIdentity` — `native/media_runtime/src/identity.rs`

- Calcula SHA-1, SHA-256, CRC32 e tamanho diretamente dos bytes exatos da ROM já descompactada/carregada.
- SHA-1 seleciona a entrada do DAT No-Intro; CRC32 e tamanho são verificações auxiliares obrigatórias.
- SHA-256 é a chave estável para o caminho do arquivo `.srm`.
- A entrada canônica e o nome da variante permanecem associados ao `CartridgeMatch` retornado pelo catálogo (game + ROM DAT); não são inferidos pelo nome do arquivo local.

### `CartridgeHardwareProfile` — `native/media_runtime/src/profile.rs`

- Separa mapper, classe de RAM (`VolatileSram`, `BatteryBackedSram`, `FeRam`), capacidade, mapeamento, janela, tamanho de banco, espelhamento, estado de evidência e proveniência.
- Somente perfis `Verified`, com mapper/mapeamento conhecidos e capacidade positiva, podem configurar o runtime.
- Protótipos e variantes sem prova suficiente ficam em `EvidenceOnly`; Flight of Pigarus permanece `Blocked`.

### `CartridgeCatalog` — `native/media_runtime/src/catalog.rs`

- Usa o DAT SMS versionado no projeto (`20261013-064924`) e associa perfis por SHA-1 exato.
- Valida CRC32 e tamanho antes de aceitar a correspondência.
- Recusa SHA-1 duplicado no DAT e perfis duplicados para a mesma identidade, em vez de escolher silenciosamente a primeira entrada.
- Os oito títulos comerciais pedidos têm perfis persistentes verificados de 8 KiB; variantes adicionais só são ativadas quando há identidade exata e evidência suficiente.
- Desert Speedtrap e The Flash têm perfil volátil de 8 KiB; The Terminator (32 KiB) permanece `EvidenceOnly` por falta de mapeamento confirmado. Othello não recebe perfil SMS.

### `SaveRamStore` — `native/save_runtime/src/lib.rs`

- I/O exclusivamente host-side; nenhuma operação de arquivo foi adicionada ao caminho CPU, mapper, barramento ou scheduler.
- Arquivo `.srm` associado ao SHA-256 da ROM, com cabeçalho versionado, identidade SHA-1/SHA-256/CRC32/tamanho, tipo/capacidade de RAM e checksums CRC32/SHA-256 do payload.
- Valida assinatura, versão, flags, identidade, tipo de memória, capacidade, tamanho exato do payload e integridade.
- Grava em arquivo temporário exclusivo no mesmo diretório, executa `sync_all` e substitui o destino por rename.
- Recusa gravação de RAM volátil e perfis não executáveis.

### Integração no carregador/runtime

- `native/sega_app/src/app.rs` identifica a ROM, consulta o catálogo, carrega o save antes de iniciar o worker e salva o cartucho anterior antes da troca.
- `rust/runtime/sms-runtime/src/worker.rs` recebe o perfil e bytes iniciais antes de sinalizar prontidão; expõe snapshot por comando coarse-grained.
- O host pausa o worker antes de exportar a SRAM para persistência, evitando uma escrita de frame entre o snapshot e a troca/parada.
- Reset reconstrói a máquina preservando os bytes de RAM do cartucho.
- Stop, fechamento da máquina, troca de ROM e encerramento controlado passam pelo store; RAM volátil nunca cria `.srm`.

### Limites ainda não comprovados

- A identidade SHA-256 é calculada sobre os bytes efetivamente carregados, mas os dumps comerciais correspondentes não foram fornecidos nesta sessão; não há confronto independente de SHA-256 dos oito títulos.
- Ainda faltam testes de integração ponta a ponta para troca de ROM, parada/encerramento, reabertura do processo e execução contínua versus blocos.
- A atomicidade de rename precisa passar no workflow Windows do HEAD final; não declarar persistência durável/atômica validada até esse gate.
- Não há validação em hardware físico nem gameplay com ROMs legais de teste nesta etapa.

## 7. Estado desta etapa

| Item | Estado |
|---|---|
| Semântica documentada de `$FFFC` bits 2/3/4 e janela alta | PASS documental — SMS Power + SMS Plus GX confrontados |
| Matriz de prioridade, overlay e portas | IMPLEMENTADA; validação final de CI ainda pendente |
| Mapeamento SRAM 8/16/32 KiB e work RAM independente | IMPLEMENTADO; suíte do HEAD final ainda NOT RUN |
| Catálogo SHA-1 + validação CRC32/tamanho + SHA-256 de armazenamento | IMPLEMENTADO; testes de colisão adicionados, CI NOT RUN |
| Oito famílias comerciais persistentes 8 KiB | PASS no confronto DAT: oito SHA-1 presentes no DAT atual com CRC32/tamanho correspondentes |
| Variantes regionais/revisões/protótipos | Parcial: perfis exatos; variantes sem evidência executável permanecem EvidenceOnly |
| RAM volátil | IMPLEMENTADA para Desert Speedtrap/The Flash; The Terminator EvidenceOnly; Othello sem perfil SMS |
| Flight of Pigarus | BLOCKED — FeRAM 32 KiB provável; decodificação e seleção de páginas não confirmadas |
| SaveRamStore e ciclo host | IMPLEMENTAÇÃO INICIAL presente; CI e integração ponta a ponta pendentes |
| SHA-256 de dumps comerciais | BLOCKED — ROMs exatas não foram disponibilizadas para auditoria independente |
| Testes locais | NOT RUN — ambiente desta sessão sem acesso de rede para clonar o workspace |
| CI do HEAD `e6a0384036ee3b13aa45324904988219d41fa6e0` | NOT RUN — não foi criada execução de workflow para os commits via API |
| CI do HEAD anterior `27b534acc2e6f1179d72a6fe857ba899a4e0e6aa` | FAIL — erros de imports em testes do worker e rustfmt Windows; correções publicadas depois |

**Próximo gate:** obter CI 100% verde no HEAD final; executar integração host/runtime para troca, stop, shutdown e reopen; validar os hashes SHA-256 com dumps exatos; manter Flight of Pigarus bloqueado até evidência elétrica independente. Nenhum merge é autorizado por este documento.