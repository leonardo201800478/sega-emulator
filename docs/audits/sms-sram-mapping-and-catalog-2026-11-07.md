# SMS SRAM — auditoria de mapeamento e identidade de cartuchos

**Data da auditoria:** 2026-11-07  
**PR de implementação:** #643  
**SHA de código analisado:** `80aa2caebb61639b1f018982fb9c162127559eba`  
**Base remota:** `f47f71830344b85f72329376505d400bfbfaae69`  
**Estado:** bit 4 implementado na branch; catálogo e persistência ainda não implementados.

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
- O MAME classifica a presença de bateria quando `feature battery=yes`, mas nem sempre informa capacidade SRAM.
- O DAT No-Intro do Master System não foi encontrado no branch `main` do projeto. As pesquisas do repositório localizaram DATs de 32X e SG-1000, mas não o DAT SMS. Não é possível declarar confronto com um DAT SMS atual do projeto.
- Os arquivos técnicos locais enviados nesta conversa não contêm uma especificação de mapper/cartucho SMS; os documentos Z80 são manuais de CPU.
- SHA-256 não é fornecido pelo MAME `sms.xml`; deverá ser calculado sobre cada dump local exato. Não preencher hashes ausentes com valores inferidos.

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

## 4. Flight of Pigarus — perfil isolado e BLOCKED

MAME `hash/sms.xml` lista três ROMs de Flight of Pigarus com `battery=yes`:

| Variante | Tamanho | CRC32 | SHA-1 |
|---|---:|---|---|
| v1.11 | 262144 | `60B86F18` | `46941B6CA4C38B07FA55987D3F30C249D5674E2A` |
| v1.10 | 262144 | `1950005D` | `3BD03C3BE402B90E61E6BE5B67216C0C6D8056E6` |
| v1.00 | 262144 | `838574D1` | `E53A16F5BF4A2D804B26C359CF3A727C06379E6D` |

O registro MAME confirma bateria, mas não fornece PCB/IC nem protocolo completo. A alegação de memória 256 Kibit/32 KiB utilizáveis não basta para concluir que o protocolo é o Sega mapper padrão. **Estado: BLOCKED** para perfil executável até confirmar independentemente decodificação, capacidade acessível, seleção de bancos e comportamento de leitura/escrita. Não incluir no catálogo genérico.

## 5. RAM volátil — não gerar .srm

| Jogo | Evidência atual | Tratamento |
|---|---|---|
| Desert Speedtrap | Pesquisa histórica do projeto registra RAM sem bateria via Gearsystem; MAME não marca battery | Candidato a RAM volátil; exigir capacidade/protocolo antes de implementar |
| The Flash | Pesquisa histórica do projeto registra RAM sem bateria via Gearsystem; MAME não marca battery | Candidato a RAM volátil; exigir capacidade/protocolo antes de implementar |
| The Terminator (Europe) | MAME lista IC2 `KM62256BLP-10L` (32 KiB SRAM) e não marca battery | RAM sem evidência de bateria; não gerar .srm |
| The Terminator (Brazil) | MAME não informa IC nem battery | UNKNOWN; não herdar perfil europeu sem fingerprint |
| Othello | Não consta como cartucho Master System no MAME `sms.xml`; documentação do projeto o lista no contexto SG-1000 | Não cadastrar como SRAM de cartucho SMS |

A ausência de `battery=yes` não prova ausência de RAM; apenas impede classificar como battery-backed com base nesta fonte.

## 6. Arquitetura e sequência de implementação

- `CartridgeIdentity`: SHA-1 para compatibilidade DAT/oráculos; SHA-256 para identidade local/armazenamento; CRC32 e tamanho como auxiliares; variante regional/revisão explícita.
- `CartridgeHardwareProfile`: mapper, classe de RAM, capacidade, battery-backed, janelas, seleção de bancos, variante de placa, proveniência e confiança.
- `SaveRamStore`: filesystem na camada host, carregamento/validação, metadados versionados, gravação atômica e associação inequívoca por SHA-256.
- Nenhum I/O no caminho CPU/barramento. O runtime recebe bytes de SRAM antes da execução e expõe snapshots/dirty state para a camada host.
- Reset de hardware preserva SRAM. Troca de ROM constrói novo estado de cartucho e salva a SRAM anterior antes de descartá-la.
- Não implementar o catálogo automático até adicionar/identificar o DAT SMS atual do projeto, calcular SHA-256 dos dumps efetivamente suportados e reconciliar variantes.
- Não implementar persistência até auditar o carregador/runtime e o limite host existente; evitar duplicar catálogo ou store.

## 7. Estado desta etapa

| Item | Estado |
|---|---|
| Semântica documentada de `$FFFC` bit 4 | DOCUMENTED |
| Implementação inicial da janela alta na PR #643 | IMPLEMENTED — pendente CI |
| Testes unitários de 8/16/32 KiB e overlay | IMPLEMENTED — pendente execução CI |
| Conflito elétrico com work RAM habilitada | MODELADO conservadoramente; hardware value não reivindicado |
| Catálogo de identidades MAME | REFERÊNCIA DOCUMENTAL, não runtime |
| Comparação com DAT SMS versionado no projeto | BLOCKED — DAT não encontrado |
| SHA-256 das variantes locais | BLOCKED — dumps exatos não fornecidos |
| Flight of Pigarus | BLOCKED |
| SaveRamStore / integração host | NOT IMPLEMENTED |

**Próximo gate:** executar CI no novo HEAD da PR, revisar os testes de janela alta e só então avançar para catálogo. Nenhum merge é autorizado por este documento.

