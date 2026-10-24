# SG-1000 — marco do primeiro jogo executável

## Objetivo

Construir um caminho de ponta a ponta para executar uma ROM SG-1000 legalmente obtida,
mostrar o vídeo e receber entrada do jogador. O áudio fica explicitamente fora deste marco.

## Componentes incluídos nesta primeira integração

- CPU Z80 existente, executada por instrução.
- ROM de cartucho e RAM interna de 1 KiB, com espelhamento de ROM pequena no adaptador da máquina.
- VDP compatível com TMS9918A: 16 KiB de VRAM, registradores, portas de dados/controle,
  status/VBlank, relógio por linhas e renderização por software de Graphics I, Graphics II,
  texto, multicolor e sprites.
- Portas iniciais de joystick 1/2 (leitura ativa em nível baixo).
- Frontend Pygame opcional, com janela 3x e mapeamento de teclado.
- Modo headless para validar o avanço de quadros com uma ROM sintética.

## Executar no Windows

Instale o pacote com o frontend opcional:

```powershell
python -m pip install -e ".[dev,frontend]"
python -m sega_emulator "C:\caminho\para\jogo.rom"
```

Controles iniciais: setas direcionais, Z/Space para botão 1, X/Enter para botão 2 e Esc para sair.

Teste headless, sem Pygame:

```powershell
python -m sega_emulator "C:\caminho\para\homebrew.rom" --headless-frames 600
```

### ROM homebrew de diagnóstico

Gere uma ROM própria de 8 KiB que configura o VDP e mostra um padrão de teste:

```powershell
python tools/create_sg1000_demo_rom.py --output sg1000-demo.rom
python -m sega_emulator .\sg1000-demo.rom
```

Essa ROM valida o caminho CPU → I/O → VDP → framebuffer; não substitui a execução de um jogo real.

Use `--headless-frames 0` para validar o caminho e o tamanho da ROM sem executar a CPU. Cada quadro executado é limitado por avanço de frame do VDP; a instrução que cruza a borda pode ultrapassar o orçamento nominal por alguns T-states.

Não distribua ROMs comerciais. Use homebrew ou dumps legalmente obtidos.

## Limitações conhecidas

- A CPU ainda retorna T-states nominais por instrução; o VDP avança depois de cada instrução,
  portanto os eventos não estão alinhados a cada T-state nem a cada acesso de barramento.
- A temporização de 228 T-states por linha e 262 linhas é o modelo NTSC inicial; variantes
  de VDP e detalhes de fase devem ser validados contra documentação e testes.
- As regras de sprite, colisão, overflow e os modos gráficos são uma implementação inicial,
  ainda sem comparação diferencial extensa com hardware ou outro VDP de referência.
- A decodificação de I/O, expansões de cartucho e espelhamento de ROM são simplificados.
- Não há PSG nem som; não há ainda sincronização de áudio/vídeo, save states ou debugger.
- Um jogo que dependa de um opcode Z80 ausente ou de comportamento não implementado pode parar
  com erro explícito; isso é um dado útil para a próxima rodada de compatibilidade.

## Critério do marco

O marco não é considerado concluído apenas porque abre uma janela. É necessário:
1. passar nos testes unitários e de integração;
2. executar pelo menos uma ROM real conhecida, sem ROMs empacotadas no repositório;
3. confirmar vídeo e entrada jogável;
4. registrar a ROM usada (nome/hash, sem redistribuir seu conteúdo), problemas e evidências;
5. documentar divergências observadas e transformá-las em regressões.

## Referências técnicas

- [Texas Instruments — TMS9918A/TMS9928A/TMS9929A Video Display Processors Data Manual](https://www.bitsavers.org/components/ti/TMS9900/TMS9918A_TMS9928A_TMS9929A_Video_Display_Processors_Data_Manual_Nov82.pdf): registradores de controle/status, modos, VRAM, sprites e flag de VBlank.
- [TMS9918A programming notes](https://github.com/cbmeeks/TMS9918/blob/master/tms9918a.txt): resumo independente de portas, registradores e modos gráficos.

As fontes fundamentam a estrutura do protótipo, mas não certificam a implementação. As regras de renderização, colisão/overflow, acesso buffered à VRAM e temporização precisam de comparação diferencial.
