# SMS1 315-5124 — registro de evidências temporais

**Escopo:** somente ASIC SMS1 315-5124, inicialmente Mode 4, perfil 192 linhas.  
**Estado:** inventário de fontes e bloqueios; não é uma especificação temporal completa.  
**Precisão cycle-accurate:** NÃO DECLARADA.  
**Regra:** uma constante existente em emulador não se torna fato de hardware por repetição.

## 1. Identidade e separação de variantes

| Item | Evidência | Estado |
|---|---|---|
| SMS1 corresponde ao ASIC 315-5124; SMS2 ao 315-5246 | SMS Power, SMS1/SMS2 VDPs; Charles MacDonald VDP notes | DOCUMENTED |
| SMS1 preserva efeitos de alguns bits legados dos registradores na geração de endereços VRAM | SMS Power, VDP Registers; coberto funcionalmente nas PRs #368/#370 | FUNCTIONAL COVERAGE |
| SMS2 suporta modos de 224/240 linhas; não transportar esses modos automaticamente ao SMS1 | SMS Power, SMS1/SMS2 VDPs; MAME variant split | DOCUMENTED |
| Região NTSC/PAL é dimensão separada da revisão do ASIC | arquitetura do projeto + fontes regionais | REQUIRED |

Fontes:
- https://www.smspower.org/Development/SMS1SMS2VDPs
- https://www.smspower.org/Development/VDPRegisters
- Charles MacDonald, Sega Master System VDP documentation (2000–2002): https://github.com/franckverrot/EmulationResources/blob/master/consoles/sms-gg/Sega%20Master%20System%20VDP%20documentation.txt

## 2. Evidência sobre V Counter e geometria vertical

### 2.1 Valores do V Counter

A página SMS Power Scanline Counter e as notas de Charles MacDonald descrevem:
- V Counter acessível pela porta $7E;
- leitura livre durante a varredura;
- contagem ativa iniciando em 0 na primeira linha exibida;
- NTSC 256x192: sequência publicada 00–DA, D5–FF;
- PAL 256x192: sequência publicada 00–F2, BA–FF;
- NTSC possui 262 scanlines por frame e PAL 313;
- o valor do contador muda em pontos específicos entre linhas; as sequências são expressas em hexadecimal.

Fontes:
- https://www.smspower.org/Development/ScanlineCounter
- Charles MacDonald VDP documentation, seção 11 — Display timing.

### 2.2 Segmentos verticais publicados

As notas de Charles MacDonald listam para 256x192:
- NTSC: 192 active, 24 bottom border, 3 bottom blanking, 3 vertical blanking, 13 top blanking, 27 top border.
- PAL: 192 active, 48 bottom border, 3 bottom blanking, 3 vertical blanking, 13 top blanking, 54 top border.

**Limite de proveniência:** o próprio documento informa que a maior parte da geometria veio originalmente de dados do TMS9918 e de um Genesis NTSC e que os resultados foram verificados com um SMS2 NTSC equipado com chave PAL/NTSC. Isso não constitui validação direta do ASIC SMS1 315-5124. Usar como referência forte de pesquisa para perfil SMS, mas não promover automaticamente a fase/segmentação a fato exclusivo do SMS1.

### 2.3 O que não está definido

- A sequência de valores não fixa o timestamp sublinha da troca de V Counter.
- A descrição por linhas não determina a fase exata de incremento na fronteira entre linhas.
- O início absoluto da área ativa no ASIC SMS1 precisa de corroboration específica.
- O instante exato em que VBlank status e VINT se tornam observáveis não é dedutível apenas da contagem de linhas.

## 3. Evidência sobre H Counter e latch

As notas de Charles MacDonald afirmam que:
- o H Counter progride com o dot clock;
- o V Counter avança ao final de cada scanline;
- o V Counter pode ser lido livremente;
- o H Counter é capturado por mudança de estado do pino TH de uma das portas de controle, associado ao uso de lightgun, e permanece congelado até nova transição de TH.

Fonte: Charles MacDonald VDP documentation, seção 11 — Display timing.

**Risco de variante:** a mesma seção adverte que a geometria horizontal detalhada foi baseada em documentação TMS9918 e testes num Genesis NTSC e que um SMS real pode diferir. Portanto, não importar para SMS1 a tabela de 342 pixels, divisões, offsets ou ranges H Counter sem medição SMS1 específica.

A página SMS Power GG Official Docs publica uma relação de dots/contagens/CPU clocks para Game Gear. É uma variante distinta e não deve ser transplantada para SMS1:
- https://www.smspower.org/Development/GGOfficialDocs

### Bloqueios H Counter

- Escala e fase exatas do H Counter no 315-5124: NEEDS SMS1 TRACE.
- Momento e polaridade da captura TH no SMS1: NEEDS SMS1 TRACE.
- Se a leitura sem latch tem comportamento adicional em revisões SMS1: UNKNOWN.
- Relação H Counter ↔ Z80 T-state em cada região: UNKNOWN.

## 4. Evidência funcional de avaliação de sprites

A página SMS Power Sprites descreve:
- até 64 sprites no total e até 8 sprites por linha;
- avaliação da SAT durante uma scanline para compor o buffer da próxima linha;
- avaliação interrompida quando 64 entradas foram examinadas, 8 entradas foram preenchidas ou Y=$D0 (208) é encontrado no modo 192 linhas;
- OVERFLOW quando mais sprites elegíveis excedem as 8 posições, independente de X ou do padrão/pixels;
- o X atua como contador decrementado conforme o H Counter avança; ao expirar, bitplanes são deslocados para produzir pixels;
- prioridade pela ordem no buffer; colisão exige pixels opacos sobrepostos.

Fonte:
- https://www.smspower.org/Development/Sprites

As regras funcionais acima estão cobertas parcialmente pelas PRs #369 e #372. A fonte não fornece por si só o slot exato de cada leitura SAT, fetch de pattern, carregamento do shifter ou mudança de pixel em cada dot.

### Bloqueios temporais de sprites

- Início/fim de SAT evaluation: UNKNOWN.
- Ordem e timestamp dos fetches SAT/pattern: UNKNOWN.
- Momento de carregar e deslocar o sprite shifter: UNKNOWN.
- Efeito de escrita concorrente na SAT/VRAM durante avaliação: UNKNOWN.
- Momento exato em que OVERFLOW/COLLISION se tornam observáveis na porta de status: NEEDS TRACE.

## 5. Referência secundária: MAME 315_5124

Arquivo consultado:
- https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp
- Licença declarada no arquivo: BSD-3-Clause.
- Identificação no código: dispositivos 315-5124, 315-5246 e 315-5377 separados.

Observações úteis para pesquisa diferencial:
- O código mantém perfis de timing de frame separados para NTSC/PAL e 192/224/240 linhas.
- A tabela line_315_5124 contém pontos de eventos horizontais; o próprio comentário marca o ponto NMI como not verified.
- DISPLAY_DISABLED_HPOS também está explicitamente marcado not verified.
- A rotina de V Counter aplica um ajuste de uma linha antes de VCOUNT_CHANGE_HPOS.
- A rotina H Counter usa a posição horizontal anterior e uma transformação de contagem/divisão.
- O arquivo lista como pendentes mudanças no meio da scanline e restrições de acesso VRAM/CRAM.

**Classificação:** IMPLEMENTATION_REFERENCE / SECONDARY. O MAME é útil para localizar hipóteses, divergências e testes; não é oráculo independente de hardware. Constantes marcadas como não verificadas permanecem bloqueadas e não devem ser copiadas para produção.

## 6. Matriz de confiança por comportamento

| Comportamento | Evidência atual | Confiança | Ação |
|---|---|---|---|
| V Counter $7E e sequência 192 NTSC/PAL | SMS Power + Charles MacDonald notes | DOCUMENTED, phase SMS1 pendente | Preparar vetores; corroborar ASIC |
| Geometria de linhas NTSC/PAL 192 | Charles MacDonald, validado no documento contra SMS2 | STRONG SECONDARY / SMS1 NOT VERIFIED | Não fixar profile SMS1 sem trace |
| H Counter captura por TH | Charles MacDonald notes | DOCUMENTED, SMS1 corroboration needed | Investigar TH latch e leituras |
| Escala horizontal 342 dots / tabela de H values | fonte declara TMS9918 + Genesis e avisa que SMS pode diferir | NOT SMS1 EVIDENCE | Não implementar |
| Diferenças 315-5124/315-5246 | SMS Power SMS1/SMS2 | DOCUMENTED | Manter profiles separados |
| 8 sprites/linha e regras de elegibilidade | SMS Power Sprites | DOCUMENTED funcional | Cobertura funcional já iniciada |
| Sprite SAT avaliada para próxima linha | SMS Power Sprites | DOCUMENTED em granularidade de scanline | Não inferir dot slots |
| Ticks por linha, divisor Z80↔VDP | sem fonte SMS1 primária suficiente | UNKNOWN | Não implementar |
| VRAM/CRAM contention e WAIT | MAME declara TODO para constraints | UNKNOWN | Não implementar |
| Prioridade de eventos simultâneos CPU/VDP | sem evidência | UNKNOWN | Não definir prioridade |
| IRQ horizontal/vertical em sublinha | MAME possui timers; fonte Charles lista evento sublinha como desconhecido | SECONDARY / UNKNOWN PHASE | Exigir traces |
| Timestamp de VCounter increment | Charles lista evento sublinha como não conhecido | UNKNOWN | Exigir trace |

## 7. Referências que NÃO podem ser transplantadas diretamente

- GG Official Docs: H Counter/Game Gear clocks não provam a mesma fase/encoding no 315-5124.
- TMS9918A: a semelhança arquitetural não prova as peculiaridades SMS1.
- Genesis VDP: parte da geometria H Counter das notas Charles veio de testes Genesis; não é oráculo SMS1.
- SmsVdp legado do projeto: pode ser consultado para inventário de funcionalidades e regressões, mas não é autoridade de timing nem expected-value oracle para Sms1Vdp.
- MAME: implementação diferencial útil, não substitui trace físico.

## 8. Plano de investigação antes de implementar timing

1. Extrair e registrar as seções de VDP dos documentos oficiais e notas de Charles MacDonald, com página/seção, revisão e variante efetivamente testada.
2. Obter traces SMS1 de leituras $7E e captura TH/$7F em bordas de linha, início/fim da área ativa e frame wrap; distinguir NTSC e PAL.
3. Obter traces de R10 line counter, HINT, VINT, status e IRQ, incluindo escritas imediatamente antes/depois dos pontos de reload.
4. Investigar fetch/contensão por testes sintéticos de escrita/leitura VRAM em fases distintas e por ROMs de diagnóstico conhecidas.
5. Comparar MAME/ARES/MEKA/Emulicious como implementações diferenciais, registrando divergências; nenhum emulador sozinho é hardware oracle.
6. Somente então definir perfil de MachineTime, ClockDomain, eventos e prioridades para SMS1 no único MachineScheduler.

## 9. Gates de implementação

- **T0 — evidence inventory:** IMPLEMENTED quando todas as incógnitas relevantes estiverem listadas com fonte e teste proposto.
- **T1 — counter traces:** implementar V/H counter somente após sequência e fases relevantes corroboradas.
- **T2 — line/frame progression:** integrar linha/quadro ao Scheduler sem pressupor ticks por linha ainda desconhecidos.
- **T3 — IRQ/status:** introduzir eventos somente com timestamp/ordem comprovados.
- **T4 — fetch pipeline/VRAM arbitration:** modelar slots e WAIT somente com evidência rastreável.
- **T5 — save-state/replay:** incluir todos os estados em voo e provar replay/chunk invariance.

Nenhuma etapa deste documento declara QUICK_PASS, SHORT_PASS, LONG_PASS, INTEGRATION_PENDING resolvido ou TEMPORAL_PASS. Os testes funcionais já existentes continuam classificados separadamente.
