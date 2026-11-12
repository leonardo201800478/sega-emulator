# Estudo de reconstrução PSG — H2

**Escopo:** SN76489 compartilhado por SG-1000 e Master System.  
**Implementação:** protótipo exclusivamente de teste; nenhuma mudança na síntese de produção.  
**Referência histórica:** f47f71830344b85f72329376505d400bfbfaae69.  
**Baseline de integração:** b375d4bb85149678b82ab39a497177d1a1cd2319.

## 1. Hipótese

A interpolação linear entre duas saídas FIR atenua energia crescente nas bandas altas e suaviza transições que o baseline histórico preservava. A hipótese H2 é que a reconstrução das transições digitais por eventos com timestamp subamostral e resposta de passo band-limited reduz esse erro sem alterar clock, contadores, registradores ou LFSR do chip.

O protótipo recebe eventos (clock, delta_amplitude). O timestamp é convertido para a posição fracionária na linha de saída:

\[
q = \frac{t_{PSG}\,f_s}{f_{clock}}
\]

A saída é reconstruída pela soma das respostas de passo deslocadas:

\[
y[n] = y_0 + \sum_k \Delta A_k\,S(n+1-q_k-G)
\]

O termo n+1 representa a convenção da saída PSG: a primeira amostra é emitida no primeiro cruzamento do acumulador de amostragem, não no instante inicial da máquina. O protótipo mantém a fase fracionária explicitamente e não altera o MachineScheduler.

## 2. Kernel experimental

| Parâmetro | Valor |
|---|---:|
| Taxa de saída | 48.000 Hz |
| Janela | Kaiser |
| Beta | 7,85726 |
| Largura total | 65 taps equivalentes |
| Meia largura | 32 amostras |
| Oversampling da tabela integrada | 1.024 fases por amostra |
| Cutoff | 0,46 ciclos/amostra |
| Compensação fracionária experimental | -0,75 amostra |
| Atraso de grupo do protótipo | 31,25 amostras |

A resposta de passo é obtida integrando numericamente o kernel sinc limitado por janela. O protótipo é independente do código Gearsystem/Blip_Buffer; somente os princípios de timestamp, delta de amplitude e reconstrução band-limited são usados como referência conceitual.

A compensação fracionária é um parâmetro experimental de alinhamento com o sample grid histórico, não uma propriedade do SN76489. Deve permanecer explícita nos relatórios e não pode ser confundida com clock de hardware.

## 3. Gates matemáticos e temporais

- Varredura de todos os períodos válidos 2–1023 e cálculo da frequência/alias de saída.
- Períodos 0 e 1 preservados como casos especiais de saída constante.
- Ganho DC unitário e resposta finita.
- Resposta de passo com overshoot, undershoot e settling medidos.
- Resposta contínua avaliada no limite de banda passante e em Nyquist.
- Eventos com timestamp inteiro em clocks PSG e posição subamostral racional.
- Captura segmentada e contínua comparadas bit a bit.
- Protótipo isolado do caminho de produção.

## 4. Transientes

A captura utiliza os mesmos eventos de registradores da referência A/B: alteração de volume, mudança do divisor de ruído, mudança de período, alteração de volume de outro canal, reset do LFSR e mudança final de volume do ruído.

O relatório compara janelas pré-evento, início, região assentada, pico e diferença entre amostras adjacentes. A resposta de passo do kernel também é medida independentemente, para que ringing do kernel não seja confundido com transientes específicos do PSG.

## 5. Tons e aliasing

O teste percorre integralmente os períodos 2–1023. WAVs de checkpoints são exportados para períodos 2, 3, 4, 5, 6, 7, 9, 14, 28, 56, 112, 255, 512 e 1023.

Os períodos 0 e 1 são testados separadamente e não são tratados como frequências oscilantes. A frequência teórica para período válido p é:

\[
f = \frac{f_{clock}}{32p}
\]

A frequência acima de Nyquist é dobrada para a banda observável antes da análise da resposta de reconstrução.

## 6. Ruído e LFSR

| Perfil | Largura | White taps | Período branco | Período periódico |
|---|---:|---|---:|---:|
| SMS | 16 bits | 0 e 3 | 57.337 | 16 |
| SG-1000 | 15 bits | 0 e 1 | 32.767 | 15 |

Os testes verificam estados, contagem de bits, retorno ao seed e o ciclo periódico. Capturas WAV separadas cobrem ruído periódico e branco para cada perfil. O trace integral também cobre a seleção de ruído derivada do terceiro tom e reset durante a execução.

A periodicidade SMS de 57.337 é característica documentada no SMS Power; não deve ser substituída por um LFSR maximal genérico de 65.535 estados.

## 7. Gearsystem como referência conceitual

Na revisão de referência indicada, Gearsystem utiliza timestamps de transição e deltas de amplitude com Blip_Buffer. O cabeçalho define qualidade 12 como boa qualidade, qualidade 8 como média e qualidade 16 como alta. O mecanismo de offset_resampled distribui a contribuição do evento em múltiplos taps conforme a fase subamostral.

O estudo atual implementa um modelo independente, com kernel integrado em ponto flutuante e parâmetros próprios. Nenhum código Gearsystem é incorporado. A referência é conceitual, não um oráculo absoluto.

## 8. Decisão arquitetural

O protótipo event-based é candidato técnico para uma etapa posterior de produção, não uma substituição aprovada. A síntese atual permanece intacta até que os artefatos CI sejam inspecionados, os resultados por perfil e período sejam aceitos, a resposta de ruído e transientes seja aprovada e exista avaliação auditiva A/B controlada.

H2 fecha a investigação e a seleção experimental. A promoção do candidato à síntese de produção não é parte desta alteração.
