# Modelo PSG SN76489AN do SG-1000

## Escopo

`SN76489` representa somente o perfil SN76489AN escolhido para o SG-1000:
três canais de tom, um canal de ruído, registradores latch/data e saída digital
mono. Não deve ser reutilizado automaticamente como perfil universal do Master
System, Game Gear ou Mega Drive: variantes Sega podem diferir no tamanho/taps do
LFSR e o Game Gear também tem roteamento estéreo.

## Contratos de API

- `write(byte)` decodifica os bytes latch/data e atualiza os registradores.
- `advance_clocks(n)` avança o estado lógico a partir do clock de entrada e
  enfileira as amostras resultantes.
- `drain_samples(limit)` consome FIFO somente as amostras enfileiradas por
  `advance_clocks`.
- `render(n)` também avança o mesmo estado lógico, mas retorna diretamente
  exatamente n amostras; elas não são duplicadas no buffer de `drain_samples`.
- O buffer de `drain_samples` tem capacidade de um segundo. Se encher, descarta
  as amostras mais antigas e incrementa `dropped_samples`; consumidores devem
  drenar com regularidade e observar esse contador.
- `reset()` limpa registradores, fases temporais, buffers e contador de perdas.

A taxa de amostragem é uma propriedade da saída digital, não o clock do console.
O clock lógico do chip permanece independente da frequência de amostragem.

## Modelo implementado

- Clock de entrada padrão: 3.579.545 Hz; divisor interno: 16.
- Três divisores de tom de 10 bits e quatro atenuadores de 4 bits.
- Tabela de atenuação nominal com passos próximos de 2 dB; nível 15 é silêncio.
- LFSR de 15 bits, seed 0x4000, com realimentação dos bits 0/1 no modo branco.
- Modos de ruído fixos e modo sincronizado ao divisor do terceiro tom.
- Fase de divisor, fase de amostragem e contadores preservados entre chamadas,
  tornando o avanço em blocos determinístico.
- O divisor efetivo de ruído é atualizado ao escrever o controle de ruído ou o
  período do terceiro tom, e não recalculado em cada tick.

## Limites conhecidos — não confundir com precisão total

1. **Fase de I/O:** a máquina avança o PSG depois de cada instrução Z80 usando a
   contagem nominal de T-states. Escritas em portas ainda não são posicionadas
   no T-state exato dentro da instrução; efeitos transitórios entre os bytes de
   um write de frequência podem, portanto, divergir do hardware.
2. **Saída analógica:** a mistura é uma aproximação digital normalizada. Não
   modela impedâncias, resposta de frequência, filtro/estágio analógico, ganho
   específico da placa nem características elétricas do chip.
3. **Períodos 0/1:** o perfil atual conserva o comportamento constante-alto que
   o projeto já assumia para o SG-1000. Fontes técnicas divergem sobre certos
   chips/revisões; a escolha ainda precisa ser validada contra uma referência
   independente do SG-1000 ou hardware real.
4. **Amostragem:** as amostras são tomadas nos ticks internos do modelo. Isso
   não substitui um integrador de banda limitada nem uma validação espectral;
   aliasing e transientes analógicos não são considerados resolvidos.
5. **Referência externa:** os testes cobrem protocolo, determinismo, fases,
   buffer, frequência aproximada e estados do ruído. Ainda não equivalem a uma
   suíte de áudio diferencial contra hardware ou a uma captura de referência.

## Validação e referências

As fontes abaixo são usadas para orientar a variante e os limites; elas não
substituem testes de regressão do próprio projeto:

- [Texas Instruments SN76489AN datasheet](https://map.grauw.nl/resources/sound/texas_instruments_sn76489an.pdf)
  — atenuação nominal e características elétricas.
- [SMS Power: SN76489](https://www.smspower.org/Development/SN76489?from=Development.PSG)
  — notas técnicas sobre registradores e variantes.
- [MAME historic SN76496 implementation](https://github.com/mamedev/historic-mame/blob/master/src/emu/sound/sn76496.c)
  — comparação de variantes TI/Sega; código de terceiros é referência técnica,
  não código reutilizado nesta implementação.
- [RTL de referência SN76489](https://github.com/dnotq/sn76489_audio/blob/master/rtl/sn76489_audio.vhd)
  — implementação independente de LFSR de 15 bits e divisores de ruído.

## Próximas condições para elevar a fidelidade

- Introduzir eventos de I/O com timestamp em T-state e validar a ordem dos
  efeitos CPU/PSG sem quebrar o caminho determinístico.
- Fixar sequências de saída esperadas para tom, ruído periódico e branco a partir
  de uma referência independente documentada.
- Comparar frequência, duty cycle, transientes e espectro em capturas de
  referência; registrar a incerteza por revisão de hardware.
- Medir o caminho de áudio em Windows real e validar a conversão para PCM do
  frontend separadamente do núcleo do PSG.
