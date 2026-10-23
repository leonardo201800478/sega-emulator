# Pesquisa de compatibilidade de hardware Sega — CPU, áudio e revisões

**Data:** 23/10/2026  
**Estado:** pesquisa inicial, com pontos confirmados e questões abertas explicitamente separados.  
**Escopo de implementação atual:** SG-1000; em seguida Master System, Game Gear e o Z80 auxiliar do Mega Drive/Genesis. Mega CD e 32X ficam documentados como arquitetura futura, sem iniciar suas implementações.

## Objetivo e decisão arquitetural

A hipótese de trabalho é compartilhar a maior parte do núcleo de uma família de CPU quando a arquitetura de instruções for compatível, sem presumir que isso torna iguais os barramentos, clocks, sinais, periféricos ou variantes físicas.

**Decisão proposta para a CPU Z80:**
- Manter um núcleo Z80 comum, determinístico e testável.
- Separar o núcleo do perfil de integração de cada sistema: clock, acesso a memória/I/O, linhas de interrupção, reset, contenção/esperas e sincronização com os demais dispositivos.
- Introduzir variantes explícitas apenas quando documentação ou testes reproduzíveis demonstrarem diferença observável (por exemplo, NMOS/CMOS, comportamento de flags não documentadas, interrupções ou ciclos de barramento).
- Não criar quatro implementações independentes do Z80 sem evidência. Uma arquitetura comum com configuração/estratégias pequenas é preferível; a configuração não deve virar um conjunto de condicionais sem rastreabilidade.
- Não declarar precisão ciclo a ciclo enquanto a CPU só contabilizar T-states agregados por instrução.

Esta é uma decisão arquitetural de trabalho, não uma afirmação de que todos os chips instalados nos consoles sejam fisicamente idênticos.

## 1. Z80 entre SG-1000, Master System, Game Gear e Mega Drive

### O que as fontes sustentam

| Sistema | CPU documentada / contexto | Clock de referência | Conclusão para o emulador |
|---|---|---:|---|
| SG-1000 original | Sega especifica Z-80A | ~3,58 MHz | Alvo inicial. O núcleo deve primeiro representar o Zilog Z80A clássico; confirmar exceções físicas por revisão se surgirem evidências. |
| Master System / Mark III | CPU da família Z80; revisões de placa podem usar fornecedores/modelos distintos | ~3,58 MHz na documentação técnica consultada | Compartilhar o núcleo lógico inicialmente. O perfil SMS precisa tratar seu mapa de memória, I/O, VDP e PSG próprios. |
| Game Gear | Manual de hardware da Sega especifica CPU clock de 3,579545 MHz | 3,579545 MHz | Mesmo núcleo de instruções como hipótese inicial; perfil de plataforma separado para memória, VDP, I/O, áudio estéreo e temporização de vídeo portátil. |
| Mega Drive / Genesis | Z80 auxiliar, além do Motorola 68000 principal. Revisões de placa listam Zilog Z0840004PSC e, em Model 2, Toshiba 84C00AU-6 ou Zilog Z84C0006FEC, entre outros | aproximadamente 3,58 MHz | Compartilhar o núcleo Z80 após validar particularidades. O contexto de barramento é radicalmente diferente: arbitragem com o 68000, RAM de som, YM2612/YM3438 e PSG. |

**Fontes:**
- Sega, ficha histórica oficial do SG-1000: https://www.sega.jp/history/hard/sg1000/
- Sega, *Game Gear Hardware Reference Manual*: https://segaretro.org/images/1/16/Sega_Game_Gear_Hardware_Reference_Manual.pdf
- Sega, *Genesis Technical Overview*: https://segaretro.org/images/1/18/GenesisTechnicalOverview.pdf
- Revisões e componentes por placa (fonte secundária; confirmar detalhes em manuais de serviço e fotografias de placa): https://segaretro.org/Sega_Mega_Drive_PCB_revisions
- Master System brasileiro/TecToy, inventário de placa e componentes (fonte de comunidade técnica): https://www.smspower.org/Development/BrazilianMasterSystemTECTOYPOWERBASEPAL-M

### O que ainda não está provado

- Que todas as variantes físicas tenham exatamente o mesmo comportamento em opcodes não documentados, flags, reset, interrupções e temporização de barramento.
- Que toda diferença de compatibilidade entre revisões Master System seja causada pelo processador. Pode decorrer de BIOS, mapper/cartucho, decodificação de memória/I/O, VDP, FM, conectores ou revisões da placa.
- Que uma diferença observada em um emulador seja uma diferença de hardware real. É necessário rastrear a fonte e comparar testes independentes.
- Que o clock nominal, isoladamente, seja suficiente para sincronizar CPU e vídeo/áudio.

### Política de implementação

1. **Core comum:** registradores, decodificação, semântica de instruções e flags documentadas.
2. **Modelo de CPU selecionável:** apenas para diferenças físicas comprovadas e observáveis.
3. **Adaptador de plataforma:** memória, I/O, interrupções, reset, bus request/acknowledge, waits e eventos externos.
4. **Agendador temporal:** relaciona clocks e eventos do sistema sem executar CPU, VDP e áudio em threads independentes sem sincronização explícita.
5. **Testes compartilhados e específicos:** corpus comum do Z80 mais regressões para cada perfil físico/sistema quando necessárias.

## 2. Z80: por que a precisão é difícil

O manual oficial cobre o conjunto documentado, operações, ciclos nominais e sinais de barramento. Porém, uma implementação de emulador precisa também decidir comportamentos que nem sempre estão completamente especificados para todas as revisões.

### Áreas que exigem pesquisa explícita

- Flags X/Y não documentadas e seus valores derivados de registradores, operandos ou barramento.
- Estado auxiliar WZ/MEMPTR e sua atualização por família de instruções.
- Estado Q e a influência em SCF/CCF e em sequências específicas.
- P/Q e a condição de interrupção após LD A,I / LD A,R.
- Prefixos DD/FD repetidos e combinações DD/FD/CB/ED.
- DAA, instruções de bloco, block I/O e flags pouco documentadas.
- HALT, EI/DI, IM 0/1/2, NMI, reconhecimento de interrupção e limites entre instruções.
- Incremento de R, refresh, estados de espera e sinais M1/MREQ/IORQ/RD/WR.
- Diferenças NMOS/CMOS e implementações compatíveis (Zilog, NEC, Toshiba), sem generalizar uma diferença específica a toda a família.

### Referências e como interpretá-las

- **Manual primário Zilog:** https://www.zilog.com/docs/z80/um0080.pdf
- **SingleStepTests/Z80:** https://github.com/SingleStepTests/z80 — inclui estado inicial/final, WZ, Q, P e opcionalmente ciclos de barramento. O README declara que os testes foram traduzidos do núcleo ares e posteriormente corrigidos; portanto, é valioso, mas não totalmente independente nem infalível.
- **z80test:** https://github.com/raxoft/z80test — inclui testes direcionados a MEMPTR e flags.
- **Z80 from Schematics — diferenças conhecidas:** https://github.com/rejunity/z80-from-schematics/blob/main/docs/known-differences.md — fonte útil porque classifica divergências por flags, MEMPTR, temporização, interrupções e ambiguidades; ainda é preciso ler os detalhes e não tomar toda a implementação como verdade de hardware.
- **MAME:** https://github.com/mamedev/mame/tree/master/src/devices/cpu/z80 — implementação madura que documenta diferenças e limitações. Referência comparativa, não especificação definitiva.
- **Gearsystem:** https://github.com/drhelius/Gearsystem — especialmente relevante pela família SG-1000/SMS/GG; examinar revisão e licença antes de qualquer reutilização.
- **ares:** https://github.com/ares-emulator/ares — referência de arquitetura/semântica; os testes SingleStepTests têm dependência histórica do ares, portanto não devem ser contados como um segundo voto independente.

### Critério de validação

Para cada conclusão, registrar: instrução e sequência mínima; estado inicial; variante do chip assumida; resultado esperado; fonte que justifica o resultado; teste usado; dependências entre fontes; e grau de confiança. Discordância entre duas implementações abre investigação, não autoriza escolher automaticamente uma delas.

## 3. Revisões do Master System e compatibilidade de jogos

A expressão “Master System 1/2” descreve modelos/revisões comerciais e físicas, não duas arquiteturas de CPU formalmente estabelecidas. As fontes consultadas mostram mudanças de placa, conectores, BIOS, VDP/ASIC e presença de FM, além de diferenças regionais. Isso não prova, por si só, duas versões incompatíveis do Z80.

O FM Yamaha YM2413 foi incorporado em modelos japoneses compatíveis com FM; o Mark III originalmente não o integrava, embora houvesse uma unidade FM de expansão. A emulação deve representar a presença/ausência do dispositivo e seu roteamento de áudio no perfil de hardware, em vez de alterar o núcleo Z80 sem evidência.

Referências:
- Visão de arquitetura SMS, com discussão de FM: https://cesarmiquel.github.io/assets/files/2025-06-09/sega-master-system-architecture.pdf
- Pesquisa de placas TecToy: https://www.smspower.org/Development/BrazilianMasterSystemTECTOYPOWERBASEPAL-M
- Comparação de modelos/revisões (fonte secundária): https://consolemods.org/wiki/Master_System:Master_System_Model_Differences

**Questão aberta — jogo Street Fighter da TecToy:** a alegação de compatibilidade restrita a revisões mais novas deve ser verificada com a identificação exata do título/ROM, mapper, BIOS, placa e sintomas. Não atribuir a causa ao Z80 até eliminar incompatibilidades de mapeamento, RAM, I/O e periféricos.

## 4. Áudio: chips e variantes não devem ser confundidos

A fidelidade sonora provavelmente exigirá um trabalho de pesquisa e validação tão rigoroso quanto a CPU, porque “SN76489 compatível” não garante sequência de ruído, divisores, comportamento de latch, reset ou mistura idênticos.

### SN76489 / família PSG

A documentação técnica da família SMS indica que o PSG de SG-series original e os clones posteriores usados por Master System, Game Gear e Mega Drive não devem ser presumidos idênticos. Devem ser pesquisados e testados, no mínimo:
- largura e realimentação do LFSR de ruído;
- modos de ruído periódico e branco;
- divisores e valores de frequência extremos;
- latch/data writes e bits ignorados;
- sequência e estado inicial do ruído após reset;
- amplitude por canal, mistura mono/estéreo e filtros analógicos da placa.

Referência de desenvolvimento: https://www.smspower.org/Development/SN76489

Não usar uma descrição de uma variante como prova para todas as outras. O comportamento do PSG precisa de perfis e testes próprios quando houver diferença demonstrada.

### FM e áudio dos sistemas posteriores

- **Master System japonês/Mark III:** YM2413 quando o hardware/modelo oferece FM; ausência nos perfis que não têm o chip.
- **Mega Drive Model 1/Model 2:** o hardware pode usar YM2612 discreto ou uma implementação baseada em YM3438 integrada em ASIC, conforme a revisão. Isto não deve ser reduzido a “o Mega Drive tem dois chips de áudio”: há diferenças entre chip discreto, integração, amplificação e circuito analógico, e a classificação precisa acompanhar a placa-alvo escolhida.
- **Mega CD:** acrescenta CPU secundária 68000, PCM estéreo de 8 canais documentado em manuais do sistema, além da reprodução de áudio de CD; sincronização e mixagem com o áudio do Mega Drive devem ser modeladas.
- **32X:** acrescenta áudio estéreo PWM próprio; deve ser mixado com a saída do Mega Drive/Mega CD na cadeia correta, não substituído por uma única saída de áudio genérica.

Fontes:
- SN76489 e variantes: https://www.smspower.org/Development/SN76489
- Revisões de placa Mega Drive (fonte secundária, útil para identificar chips físicos): https://segaretro.org/Sega_Mega_Drive_PCB_revisions
- Manual técnico do Mega CD (cópia digital de manual de serviço): https://segaretro.org/images/5/51/Mega-CD_Hardware_Manual.pdf
- Manual de hardware 32X: https://consolemods.org/wiki/images/e/e9/32X_Hardware_Manual_1994_Sega_text.pdf

## 5. M68K, Mega CD e 32X: esclarecimentos para o roadmap

### Motorola 68000

O Mega Drive/Genesis usa um Motorola 68000/derivado como CPU principal, nominalmente em torno de 7,67 MHz. O Mega CD tem seu próprio 68000 secundário, especificado no manual do Mega CD a 12,5 MHz. São CPUs da mesma família ISA, mas com clocks, memórias, barramentos, prioridades e papéis diferentes. Devemos compartilhar um core 68000 apenas se sua semântica e interface forem adequadas; cada sistema necessita de integração temporal e de barramento própria.

O núcleo M68K também tem problemas de precisão em emulação: não basta implementar a ISA. Exceções, flags, acesso desalinhado, ordenação de barramento, ciclos de instrução e diferenças entre 68000 e derivados precisam de investigação e testes. Não devemos inferir que sua lacuna seja igual à do Z80, nem quantificá-la sem uma auditoria separada.

Referência primária: *M68000 8-/16-/32-Bit Microprocessors User’s Manual*: https://www.bitsavers.org/components/motorola/68000/68000/M68000UM_AD_M68000_Microprocessor_Users_Manual_Rev8_1993.pdf

### Mega CD

O manual técnico Sega descreve o 68000 principal do Mega Drive a 7,67 MHz e o CPU secundário do Mega CD a 12,5 MHz. O Mega CD não é “um segundo clock” do mesmo processador: é outro processador com memória e responsabilidades próprias, sincronizado com o sistema hospedeiro por interfaces de hardware.

Manual: https://segaretro.org/images/5/51/Mega-CD_Hardware_Manual.pdf

### 32X

O 32X contém dois Hitachi SH-2 de 32 bits a aproximadamente 23 MHz, usualmente descritos como CPUs master/slave. São dois núcleos da mesma família, mas sua sincronização, arbitragem, caches, memória compartilhada, framebuffers e comunicação entre processadores precisam ser emulados explicitamente. A documentação indica que a unidade também tem geração de áudio PWM estéreo e uma camada de vídeo própria, além do vídeo do Mega Drive.

Manual de hardware Sega (1994): https://consolemods.org/wiki/images/e/e9/32X_Hardware_Manual_1994_Sega_text.pdf  
Manual de serviço: https://retrotechwiki.com/Sega_Genesis_32X_/_Mega_Drive_32X_Service_Manual

## 6. Revisões do Mega Drive/Genesis: como representar V1 e V2

O rótulo Model 1/Model 2 não deve ser confundido com uma simples revisão de CPU. Há alterações em placa, ASICs, áudio, amplificação, vídeo e integração de componentes. Em algumas placas Model 1 o YM2612 é discreto; em modelos posteriores há integração baseada em YM3438 em ASIC. Mesmo dentro de Model 1 ou Model 2 existem revisões adicionais.

Para o escopo futuro, criar dois perfis principais somente depois de definir quais placas concretas eles representam. Cada perfil precisa listar:
- revisão/placa de referência e região;
- CPU principal e CPU Z80;
- VDP/ASIC e variante de áudio;
- clock e padrão de vídeo;
- caminhos analógicos/saídas de áudio;
- diferenças observáveis em software;
- evidências, fotos/manual de serviço e testes de regressão.

Não assumir que “Model 1” e “Model 2” cobrem todas as diferenças possíveis. As revisões internas devem ser opcionais se houver impacto de compatibilidade demonstrável.

## 7. Ordem de trabalho recomendada

1. **SG-1000 Z80:** especificação do Z80A, cobertura documentada, flags, instruções, interrupções e ciclos; resolver as divergências conhecidas com evidência.
2. **SG-1000 bus/memória:** confirmar decodificação e comportamento do hardware além da RAM espelhada já documentada.
3. **SG-1000 VDP e PSG:** documentar TMS9918A e variante do PSG realmente usada, incluindo eventos e portas.
4. **Perfil Master System:** identificar diferenças reais de Z80, VDP, PSG, memória, BIOS e FM; não bifurcar o core sem necessidade.
5. **Perfil Game Gear:** acrescentar particularidades de tela, registradores, estéreo, portas e temporização.
6. **Z80 do Mega Drive:** integrar o mesmo core ao subsistema de áudio/bus request, validando o acesso concorrente ao barramento e à RAM de som.
7. **Auditoria M68K separada:** só quando o roadmap alcançar Mega Drive; usar testes específicos do 68000 e documentar divergências por família de instrução.
8. **Mega CD e 32X:** depois, introduzir os processadores e domínios de clock adicionais, áudio PCM/PWM e composição de vídeo.

## 8. Critérios de qualidade da pesquisa

Uma afirmação só passa a requisito de implementação quando:
- tem uma fonte identificável, com revisão/página/seção quando possível;
- o escopo da fonte corresponde à máquina/variante discutida;
- fontes derivadas de um mesmo núcleo são identificadas como dependentes;
- incertezas e conflitos são preservados no texto;
- existe um plano de validação que não pressupõe o resultado;
- a mudança de código aponta para o requisito e inclui regressão.

### Estado desta pesquisa

**Confirmado por fonte:** especificações nominais básicas do SG-1000; clock de CPU do Game Gear; existência de diferentes fabricantes/modelos de Z80 em revisões do Mega Drive; diferenças de hardware/áudio entre revisões de Mega Drive; 68000 separado no Mega CD; dois SH-2 no 32X; FM YM2413 em modelos japoneses compatíveis.

**Hipótese de arquitetura:** compartilhar o core Z80 e separar adaptadores de plataforma e variantes observáveis. Deve ser validada ao longo da implementação.

**Não confirmado ainda:** equivalência física completa de todas as CPUs Z80; causa exata de incompatibilidades atribuídas às revisões do Master System/TecToy; lista exaustiva de diferenças entre Model 1 e Model 2; perfil analógico e digital completo de cada PSG/YM; grau comparativo de lacunas de precisão do M68K.

