# Auditoria integral do VDP TMS9918A — plano de execução

**Estado:** auditoria iniciada; este documento define a matriz de investigação. Não representa aprovação de fidelidade nem conclusão da auditoria.

## Objetivo

Auditar o VDP da primeira máquina SG-1000 de ponta a ponta, identificando divergências funcionais, temporais, de integração e de desempenho, com evidências reproduzíveis. Não alterar o comportamento do VDP sem um teste que reproduza o problema e uma referência rastreável.

## Base conhecida em `main`

A documentação existente (`docs/hardware/tms9918a-model.md` e `docs/hardware/tms9918a-validation-backlog.md`) descreve um modelo Python com 16 KiB de VRAM, oito registradores, buffer de leitura, quatro modos de vídeo, sprites e avanço raster nominal. O modelo usa 228 T-states por linha e 262 linhas por quadro, com VBlank na fronteira da linha 192. A própria documentação declara que não há fetches de VRAM por dot, arbitragem, contenção nem pipeline de pixels em nível de ciclo.

Esses números são a configuração modelada atual, não uma afirmação de que todas as variantes/revisões do hardware tenham esses valores. A paleta RGB é aproximada.

## Frentes da auditoria

### A. Interface de registradores, portas e VRAM
- [ ] Mapear o caminho de código real e as interfaces públicas do VDP e do barramento SG-1000.
- [ ] Verificar wrap de endereço de 14 bits, limites de VRAM e máscaras dos registradores R0–R7.
- [ ] Testar latch de dois bytes, comandos interrompidos, aliases de porta e leitura de status durante sequência de controle incompleta.
- [ ] Validar prefetch/buffer de leitura, incremento de endereço, escritas após leitura, reset e tabelas sobrepostas.
- [ ] Cobrir bits reservados e combinações inválidas sem presumir comportamento não documentado.

### B. Modos de vídeo e composição
- [ ] Construir estados sintéticos determinísticos para Graphics I, Graphics II, Multicolor e Text.
- [ ] Validar cálculos de bases, máscaras, alinhamentos, wrap e fronteiras das tabelas de nomes, padrões e cores.
- [ ] Testar cor zero, backdrop, bordas, clipping, coordenadas negativas/fora da tela e dimensões do framebuffer.
- [ ] Separar divergência lógica de pixels de divergência de paleta/conversão RGB.
- [ ] Verificar modos híbridos e documentar como hipótese todo comportamento sem evidência suficiente.

### C. Sprites e status
- [ ] Cobrir terminador Y=0xD0, wrap de Y, sprites parcialmente/totalmente fora da área visível e deslocamento Early Clock.
- [ ] Testar tamanhos 8×8/16×16, magnificação, padrões e máscaras em todos os quadrantes.
- [ ] Confirmar prioridade, transparência, colisão, contagem de sprites e índice do quinto sprite.
- [ ] Verificar quando F, 5S e C são definidos/limpos e o efeito de ler status.
- [ ] Comparar os resultados com vetores independentes derivados do manual ou de traces identificados.

### D. Raster, VBlank, interrupções e integração CPU–VDP
- [ ] Testar status imediatamente antes, na fronteira e depois de VBlank.
- [ ] Verificar que T-states avançam o VDP exatamente uma vez, sem omissões nem dupla contagem.
- [ ] Testar a ativação da interrupção, habilitação por R1 e resposta observável da CPU.
- [ ] Capturar regressões de escritas de VRAM/registradores entre scanlines e documentar a limitação intra-scanline.
- [ ] Não declarar precisão ciclo a ciclo até haver scheduler explícito e testes de eventos de dot/barramento.

### E. Diferencial contra referência
- [ ] Fixar versão/commit do Ares ou MAME e registrar configuração e região.
- [ ] Gerar cenas de referência reproduzíveis por modo, incluindo estado inicial completo.
- [ ] Comparar framebuffer por hash e relatório de pixels divergentes; não depender só de inspeção visual.
- [ ] Comparar leituras de status, buffer de leitura e eventos observáveis, não somente a imagem.
- [ ] Garantir que as capturas usem a mesma ROM/revisão, frame e sequência de entradas; preservar ROMs comerciais fora do repositório.

### F. Desempenho e determinismo
- [ ] Medir CPU, avanço raster, composição de fundo, sprites, conversão RGB e apresentação separadamente.
- [ ] Registrar hashes de framebuffer por quadro e verificar determinismo entre execuções.
- [ ] Estabelecer benchmark antes de qualquer otimização e comparar apenas no mesmo hardware/configuração.
- [ ] Não somar tempos inclusivos de funções aninhadas como se fossem custos independentes.

## Critérios de aceitação da auditoria

1. Cada item concluído aponta para teste, evidência e fonte/referência.
2. Cada divergência inclui reprodução mínima, estado inicial, resultado esperado/obtido e hash quando aplicável.
3. Correções de código são pequenas, acompanhadas de testes de regressão e executadas separadamente por família de comportamento.
4. Os checks relevantes de pytest, Ruff e CI são observados e seus resultados reportados sem inferência.
5. A documentação distingue fatos do manual, comportamento observado em referência, inferências e incertezas.
6. Não há alegação de conformidade total, equivalência com hardware ou precisão ciclo a ciclo sem evidência específica.

## Fontes primárias e referências cruzadas

- Texas Instruments, *TMS9918A/TMS9928A/TMS9929A Video Display Processors Data Manual* (1982): https://computers.baffa.tec.br/pages/datasheet/TMS9918A_TMS9928A_TMS9929A_Video_Display_Processors_Data_Manual_Nov82.pdf
- Notas técnicas complementares: https://github.com/cbmeeks/TMS9918/blob/master/tms9918a.txt
- MAME: https://github.com/mamedev/mame
- Ares: https://github.com/ares-emulator/ares

As referências cruzadas não substituem a fundamentação por comportamento documentado ou teste reproduzível. A auditoria deve registrar o commit exato de cada referência usada.
