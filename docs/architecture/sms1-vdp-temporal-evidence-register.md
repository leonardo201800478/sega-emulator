# SMS1 315-5124 — registro de evidências temporais

**Escopo:** somente ASIC SMS1 315-5124, inicialmente Mode 4, perfil 192 linhas.  
**Estado:** inventário de fontes e bloqueios; não é uma especificação temporal completa.  
**Precisão cycle-accurate:** NÃO DECLARADA.  
**Regra:** uma constante existente em emulador não se torna fato de hardware por repetição.

## 1. Identidade e separação de variantes

| Item | Evidência | Estado |
|---|---|---|
| SMS1 corresponde ao ASIC 315-5124; SMS2 ao 315-5246 | SMS Power, SMS1/SMS2 VDPs | DOCUMENTED |
| SMS1 preserva efeitos de alguns bits legados dos registradores na geração de endereços VRAM | SMS Power, VDP Registers; coberto funcionalmente nas PRs #368/#370 | FUNCTIONAL COVERAGE |
| SMS2 suporta modos de 224/240 linhas; não transportar esses modos automaticamente ao SMS1 | SMS Power, SMS1/SMS2 VDPs | DOCUMENTED |
| Região NTSC/PAL é dimensão separada da revisão do ASIC | arquitetura do projeto + fontes regionais | REQUIRED |

Fontes:
- https://www.smspower.org/Development/SMS1SMS2VDPs
- https://www.smspower.org/Development/VDPRegisters

## 2. Evidência publicada sobre V Counter

A página SMS Power Scanline Counter declara que:
- o V Counter é lido pela porta $7E;
- durante a área ativa começa em 0 na primeira linha exibida e incrementa ao longo da área ativa;
- a sequência posterior depende do padrão de vídeo e do modo;
- em 256x192: NTSC apresenta 00-DA, D5-FF; PAL apresenta 00-F2, BA-FF;
- NTSC 240 linhas não produz saída válida; os modos PAL descritos totalizam 313 linhas progressivas.

Fonte:
- https://www.smspower.org/Development/ScanlineCounter

**O que esta fonte NÃO fixa:** timestamp de cada transição dentro da linha, instante exato do latch do valor lido, fase de mudança no início/fim da área ativa, ou relação com o clock Z80. A sequência de valores, sozinha, não define uma timeline em MachineTime.

### Uso permitido nesta fase

- Criar vetores de teste para a sequência de valores quando a posição física da linha e o início da área ativa estiverem definidos por evidência suficiente.
- Separar o perfil NTSC 192 do PAL 192.
- Não inferir offsets absolutos de linha apenas pela ordem dos valores.

### Bloqueios

- ACTIVE_DISPLAY_START_LINE absoluto por perfil: NEEDS CORROBORATION.
- Fase sublinha em que a porta $7E troca de valor: UNKNOWN.
- Efeito de leitura exatamente na borda da transição: UNKNOWN.

## 3. Evidência funcional de avaliação de sprites

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

## 4. Referência secundária: MAME 315_5124

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

## 5. Matriz de confiança por comportamento

| Comportamento | Evidência atual | Confiança | Ação |
|---|---|---|---|
| V Counter $7E e sequência 192 NTSC/PAL | SMS Power Scanline Counter | DOCUMENTED, sem fase sublinha | Preparar vetores; bloquear integração temporal |
| Diferenças 315-5124/315-5246 | SMS Power SMS1/SMS2 | DOCUMENTED | Manter profiles separados |
| 8 sprites/linha e regras de elegibilidade | SMS Power Sprites | DOCUMENTED funcional | Cobertura funcional já iniciada |
| Sprite SAT avaliada para próxima linha | SMS Power Sprites | DOCUMENTED em granularidade de scanline | Não inferir dot slots |
| H Counter e conversão de hpos | MAME + documentos de variantes GG | SECONDARY / VARIANT RISK | Bloqueado para SMS1 até corroborar |
| Ticks por linha, divisor Z80↔VDP | não há fonte SMS1 primária suficiente neste registro | UNKNOWN | Não implementar |
| VRAM/CRAM contention e WAIT | MAME declara TODO para constraints | UNKNOWN | Não implementar |
| Prioridade de eventos simultâneos CPU/VDP | sem evidência | UNKNOWN | Não definir prioridade |
| IRQ horizontal/vertical em sublinha | MAME possui timers; fase exata sem oráculo | SECONDARY | Exigir traces |
| Latch de status HBlank | transcrição dos docs oficiais SMS em SMS Power | DOCUMENTED, timestamp exato pendente | Testar transição e leitura |

## 6. Referências que NÃO podem ser transplantadas diretamente

A página SMS Power GG Official Docs descreve um H Counter com detalhes de dots, contagens e clock CPU para Game Gear. Isso não prova automaticamente o mesmo phase/encoding para o ASIC SMS1 315-5124. Manter como fonte de variante separada:
- https://www.smspower.org/Development/GGOfficialDocs

A implementação antiga SmsVdp do projeto pode ser consultada para inventário de funcionalidades e regressões, mas não é autoridade de timing nem expected-value oracle para Sms1Vdp.

O VDP do SG-1000/TMS9918A também não é fonte para as peculiaridades temporais do SMS1.

## 7. Plano de investigação antes de implementar timing

1. Extrair e registrar as seções de VDP dos documentos oficiais e notas de Charles MacDonald, com página/seção e revisão.
2. Obter traces independentes de leituras repetidas de $7E/$7F ao redor de bordas de linha, início/fim da área ativa e frame wrap; distinguir NTSC e PAL.
3. Obter traces de R10 line counter, HINT, VINT, status e IRQ, incluindo escritas imediatamente antes/depois dos pontos de reload.
4. Investigar fetch/contensão por testes sintéticos de escrita/leitura VRAM em fases distintas e por ROMs de diagnóstico conhecidas.
5. Comparar MAME/ARES/MEKA/Emulicious como implementações diferenciais, registrando divergências; nenhum emulador sozinho é hardware oracle.
6. Somente então definir perfil de MachineTime, ClockDomain, eventos e prioridades para SMS1 no único MachineScheduler.

## 8. Gates de implementação

- **T0 — evidence inventory:** IMPLEMENTED quando todas as incógnitas relevantes estiverem listadas com fonte e teste proposto.
- **T1 — counter traces:** implementar V/H counter somente após sequência e fases relevantes corroboradas.
- **T2 — line/frame progression:** integrar linha/quadro ao Scheduler sem pressupor ticks por linha ainda desconhecidos.
- **T3 — IRQ/status:** introduzir eventos somente com timestamp/ordem comprovados.
- **T4 — fetch pipeline/VRAM arbitration:** modelar slots e WAIT somente com evidência rastreável.
- **T5 — save-state/replay:** incluir todos os estados em voo e provar replay/chunk invariance.

Nenhuma etapa deste documento declara QUICK_PASS, SHORT_PASS, LONG_PASS, INTEGRATION_PENDING resolvido ou TEMPORAL_PASS. Os testes funcionais já existentes continuam classificados separadamente.
