> **Atualização de estado (2026-10-27):** o hardware SG-1000 ativo é Rust-only. A implementação Python antiga e ferramentas/testes que dependiam dela estão em `legacy/python_reference/`; não fazem parte do runtime e os comandos históricos abaixo não são caminhos de execução ativos.

# TMS9918A — modelo, evidências e plano de fidelidade

## Escopo real desta etapa

O núcleo ativo `Tms9918A` está em Rust (`native/sg1000_core/src/vdp.rs`) e mantém 16 KiB de VRAM, oito registradores
de controle, registrador de status, portas com aliases, buffer de leitura,
endereçamento de 14 bits, quatro modos gráficos documentados, sprites e um
relógio raster nominal. A imagem RGB24 é construída linha a linha e publicada
quando começa VBlank, após a última linha visível; isso não é uma implementação
ciclo a ciclo nem uma simulação elétrica do VDP.

Esta etapa endurece a semântica de sprites e estado:
- os quatro primeiros sprites ativos por linha são os únicos desenhados;
- o primeiro sprite opaco na ordem da tabela mantém a prioridade visual;
- pixels de padrão sobrepostos geram colisão mesmo quando a cor é zero ou a
  coordenada horizontal está fora da área visível;
- o quinto sprite registra seu índice no status quando o flag de quadro F está
  zerado; o overflow é avaliado durante o avanço raster, e não somente quando
  a imagem completa é renderizada;
- reset explícito limpa registradores e relógio, preservando VRAM por padrão.
  Limpar VRAM é uma opção do emulador, não uma afirmação sobre valores de power-on.

## Registradores e interfaces

| Registrador | Função modelada | Pendências relevantes |
|---|---|---|
| R0 | M3 e controle de vídeo externo | Comportamentos de entrada externa não modelados |
| R1 | seleção de VRAM, blank/display, interrupção, modos M1/M2, tamanho e magnificação de sprites | Validar combinações híbridas de modo e máscaras de bits não usados |
| R2 | base da tabela de nomes | Confirmar comportamento de todos os bits em revisões de chip |
| R3 | base da tabela de cores | Cobrir todos os alinhamentos e efeitos de máscara |
| R4 | base do gerador de padrões | Cobrir aliases e todos os modos |
| R5 | base da tabela de atributos de sprites | Cobrir tabelas sobrepostas, terminador e wrap |
| R6 | base do gerador de padrões de sprites | Cobrir endereçamento de sprites 8×8/16×16 |
| R7 | cor de texto e backdrop | A paleta RGB atual é aproximada, não uma reprodução analógica |

As portas de dados e controle/status são decodificadas com a máscara de hardware
adotada pela integração SG-1000. O status possui F (VBlank/interrupt pending),
5S (overflow do quinto sprite), C (coincidência) e o índice do sprite quando 5S
está ativo. Ler status limpa os flags e cancela uma primeira metade de comando
de controle pendente.

## Modos de vídeo

A seleção de modo usa os bits M1/M2/M3 documentados:
- Graphics I: tabela de nomes, padrões e cores por grupo de oito caracteres.
- Graphics II: padrões e cores por seção vertical, permitindo bitmap colorido.
- Multicolor: blocos de cor de 4×4 pixels lógicos.
- Text: 40 colunas por 24 linhas, glifos de 6×8 e sem sprites.

As combinações híbridas não documentadas ainda caem no caminho de Graphics I;
esse comportamento é provisório até haver teste de silício ou referência
independente que justifique o comportamento específico.

## Relógio e sincronização

O relógio atual avança por T-states nominais do Z80, usando 228 T-states por
linha e 262 linhas por quadro na configuração padrão. O flag F é levantado na
fronteira da linha 192. A composição de fundo e sprites acontece na fronteira de cada linha e usa o
estado de registradores/VRAM naquele instante. Os pixels de linhas já emitidas
não são retroativamente alterados por escritas posteriores. A imagem completa
é publicada quando começa VBlank, após a última linha visível.

Isso melhora o instante de atualização dos flags, mas **não** simula fetches de
VRAM por dot, arbitragem, contenção, latência de barramento, pipeline de pixels
ou variações analógicas. Não anunciar precisão ciclo a ciclo.

## Plano de validação e implementação

### Fase 1 — semântica do VDP e regressões
- [x] VRAM de 16 KiB, wrap e buffer de leitura.
- [x] Protocolo de escrita em registradores e reset do latch ao ler status.
- [x] Modos Graphics I, Graphics II, Multicolor e Text.
- [x] Sprites 8×8/16×16, magnificação, deslocamento horizontal, prioridade,
      colisão e limite de quatro por linha.
- [x] Avaliação raster dos flags de sprite.
- [ ] Testes diferenciais por registrador/porta contra uma referência confiável.
- [ ] Cobertura exaustiva de endereços de tabela, valores extremos e tabelas
      sobrepostas.

### Fase 2 — correção funcional do renderizador
- [ ] Testes dourados de cada modo com VRAM sintética e hashes de framebuffer.
- [ ] Confirmar regras de color zero, backdrop, bordas e modos híbridos.
- [ ] Testar sprites parcialmente/totalmente fora da área visível, magnificação,
      prioridade, colisão e overflow com vetores independentes.
- [ ] Comparar imagens de ROM homebrew com ares/MAME usando a mesma cena e
      resolução, documentando diferenças de captura e paleta.

### Fase 3 — temporização raster
- [x] Renderizar linhas incrementalmente e preservar pixels já emitidos.
- [x] Publicar o framebuffer no início de VBlank, quando as linhas visíveis estão completas.
- [x] Comparar o renderizador incremental e o renderizador de referência nos quatro modos.
- [ ] Modelar eventos de VBlank, status e interrupção no instante correto.
- [ ] Determinar temporizações por região/revisão com evidência; não inferir
      PAL/NTSC somente a partir de frequência de apresentação.
- [ ] Acrescentar testes que leiam status antes, durante e depois de VBlank.

### Fase 4 — precisão de barramento e hardware
- [ ] Modelar ciclos de acesso à VRAM e arbitragem/contenda quando aplicável.
- [ ] Separar TMS9918A NTSC de TMS9929A/TMS9929A PAL quando as diferenças
      estiverem documentadas e representadas por testes.
- [ ] Verificar comportamento de power-on/reset e bits não documentados.
- [ ] Usar testes de hardware ou traces de emuladores de referência com versão
      identificada; separar fatos documentados de inferências.

## Critérios de aceitação

Nenhuma etapa é considerada completa apenas porque a interface inicia ou
produz uma imagem. Cada fase precisa de testes automatizados, regressões
determinísticas e comparação documentada. Os resultados devem informar os casos
testados, as referências e os comportamentos ainda não confirmados.

## Fontes técnicas

- Texas Instruments, *TMS9918A/TMS9928A/TMS9929A Video Display Processors Data
  Manual* (November 1982), especialmente §§2.2–2.4 e a descrição de sprites:
  https://computers.baffa.tec.br/pages/datasheet/TMS9918A_TMS9928A_TMS9929A_Video_Display_Processors_Data_Manual_Nov82.pdf
- Notas técnicas comunitárias sobre o TMS9918A, usadas como referência cruzada,
  não como substituto do manual:
  https://github.com/cbmeeks/TMS9918/blob/master/tms9918a.txt
- MAME — drivers de sistemas que usam a família TMS9918/9928, para conferir a
  integração e os aliases de portas específicos de cada sistema:
  https://github.com/mamedev/mame/blob/master/src/mame/msx/msx.cpp

As referências independentes são ferramentas de comparação, não prova automática
de que este modelo já tenha equivalência com elas.
