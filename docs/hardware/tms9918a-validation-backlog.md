# TMS9918A — backlog consolidado de validação

## Objetivo e estado-base

Este documento consolida as pendências técnicas remanescentes das antigas PRs #26, #27, #33 e #35 após comparar suas alterações com o código atual de `main`. Não é uma declaração de conformidade completa com o hardware.

O código já contém VRAM de 16 KiB, portas e buffer de leitura, quatro modos gráficos, composição incremental por scanline, publicação de framebuffer no início de VBlank, sprites 8×8/16×16, magnificação, early clock, prioridade, colisão e status de overflow. Os testes sintéticos existentes cobrem parte significativa desses caminhos.

A correção incluída nesta consolidação valida a seleção de bytes de padrão no modo Multicolor por coordenada vertical global: a seleção do padrão deve avançar entre as faixas de caracteres, não reiniciar a cada oito linhas. Também acrescenta regressões para primeiro overflow na ordem raster e para flags de colisão limpas e levantadas novamente em linhas subsequentes.

## Itens concluídos por esta PR

- [x] Consolidar a lista de lacunas das PRs #26, #27, #33 e #35 num único documento atual.
- [x] Testar e corrigir a seleção de padrão do modo Multicolor entre linhas de caracteres, nos caminhos incremental e de quadro completo.
- [x] Cobrir a escolha do índice do quinto sprite quando um overflow posterior na tabela ocorre numa linha raster anterior.
- [x] Cobrir a limpeza do flag de colisão por leitura de status e sua possível reativação numa linha posterior.
- [x] Manter explícito que a temporização atual é nominal e não ciclo a ciclo.

## Backlog restante

### P0 — regressões funcionais do VDP

- [ ] Cobrir a matriz de máscaras e endereços-base de R2–R6, incluindo tabelas sobrepostas e limites de VRAM.
- [ ] Cobrir sequências de portas alternando leitura, escrita, escrita de registrador, leitura de status e comandos de controle interrompidos.
- [ ] Validar o prefetch do buffer de leitura e o seu comportamento depois de escritas e reset contra o manual primário.
- [ ] Expandir os testes de Graphics I, Graphics II, Multicolor e Text para limites, valores extremos, backdrop/cor zero e padrões nas fronteiras das tabelas.
- [ ] Cobrir sprites 8×8 e 16×16 em todos os quadrantes, magnificação, early clock, clipping horizontal/vertical, terminador Y=0xD0 e endereços de padrão próximos do wrap.
- [ ] Verificar o limite de quatro sprites por linha, a contagem de sprites transparentes e o índice de overflow com vetores independentes.
- [ ] Testar a relação entre leitura do status, flags F/5S/C e o estado de interrupção em sequências de várias linhas.

### P1 — validação diferencial e evidência

- [ ] Definir cenas sintéticas determinísticas por modo, com configuração de registradores e VRAM documentada.
- [ ] Comparar framebuffers com uma versão identificada de ares ou MAME; usar a mesma cena, estado inicial, região e frame.
- [ ] Gerar hashes de framebuffers e relatório de pixels divergentes; não depender somente de screenshots visualmente semelhantes.
- [ ] Comparar o valor e o instante observável de mudanças de status, o buffer de leitura, endereço interno e saída de interrupção.
- [ ] Registrar versão/commit da referência, configuração, hashes dos artefatos e passos de reprodução.
- [ ] Separar divergências de paleta/conversão RGB de divergências funcionais de endereçamento ou geometria.

### P1 — integração CPU–VDP e temporização raster

- [ ] Confirmar por testes de integração que os T-states retornados pelo Z80 avançam o VDP uma única vez, sem omissões ou dupla contagem.
- [ ] Testar leituras de status imediatamente antes, na fronteira e depois de VBlank.
- [ ] Validar o momento de ativação da interrupção e a resposta observável da CPU.
- [ ] Definir regressão reproduzível para um caso de sprites móveis, capturando estado de CPU, tabela de atributos e VRAM ao longo de quadros.
- [ ] Testar efeitos de escritas de registradores/VRAM entre scanlines; não assumir que o modelo por linha reproduz efeitos dentro de uma linha.
- [ ] Confirmar temporizações regionais/revisões apenas quando houver documentação ou traces que sustentem os valores.

### P2 — precisão de hardware

- [ ] Especificar um scheduler determinístico comum antes de introduzir eventos de dot ou acesso de barramento.
- [ ] Modelar fetches de VRAM, pipeline de pixels, contenção e arbitragem somente após identificar comportamentos observáveis que exijam esses mecanismos.
- [ ] Separar variantes TMS9918A/TMS9928A/TMS9929A e regiões de vídeo conforme diferenças comprovadas, sem generalizar valores entre chips.
- [ ] Investigar bits reservados, estados de power-on/reset e combinações híbridas de modo com fontes primárias e regressões.
- [ ] Separar geração digital de pixels da conversão RGB e de possíveis efeitos analógicos; não tratar a paleta atual como saída elétrica de televisão.

### P2 — desempenho

- [ ] Fazer profiling separado de CPU, avanço raster, renderização de fundo, sprites, conversão RGB e apresentação.
- [ ] Otimizar somente gargalos medidos e preservar hashes de framebuffer e determinismo antes/depois.

## Critérios para encerrar o backlog

Uma área só pode ser marcada como concluída quando:

1. houver testes reproduzíveis para os comportamentos documentados;
2. expectativas críticas estiverem fundamentadas em manual primário, teste independente ou trace identificado;
3. os checks automatizados estiverem aprovados;
4. as limitações ainda não comprovadas forem registradas;
5. não houver afirmação de precisão ciclo a ciclo sem um modelo temporal e testes específicos que a demonstrem.

## Referências técnicas

- Texas Instruments, *TMS9918A/TMS9928A/TMS9929A Video Display Processors Data Manual* (1982): https://computers.baffa.tec.br/pages/datasheet/TMS9918A_TMS9928A_TMS9929A_Video_Display_Processors_Data_Manual_Nov82.pdf
- Notas técnicas complementares do TMS9918: https://github.com/cbmeeks/TMS9918/blob/master/tms9918a.txt
- MAME, referência de integração da família TMS9918: https://github.com/mamedev/mame

Fontes comunitárias e emuladores de referência servem para comparação cruzada; não substituem automaticamente o manual primário nem comprovam por si só equivalência do modelo.
