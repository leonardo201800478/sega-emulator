# SMS — Mapper estendido de 4 MiB para homebrew

Data: 2026-11-06

## Problema observado

O caminho de carregamento SMS aceitava até 1 MiB. A ROM Flashback SMS 0.0.7 fornecida para validação tem 4 MiB, alinhamento de 16 KiB e assinatura `TMR SEGA` no offset `0x7FF0`; o carregador e o núcleo rejeitavam o arquivo antes da execução.

## Frente 1 — engenharia reversa do port Flashback

Repositório do projeto: https://github.com/haroldo-ok/flashback-sms

Evidências encontradas:

- O README descreve a imagem de 4.096 KiB e o uso de bancos para dados do jogo.
- `src/main.c` mantém o índice de banco em `unsigned char s_bank` e usa `SMS_mapROMBank(s_bank)`.
- `tools/smstest/smstest.c` implementa o alvo de 4 MiB, 256 bancos de 16 KiB e registradores de mapper em `0xFFFC..0xFFFF` com valores de 8 bits.
- O README do port alerta que a simulação de jogo opera aproximadamente a um terço da velocidade original por custo da lógica Z80 do próprio port.

A última observação não é prova de FPS baixo no emulador. A taxa de frames do VDP deve ser medida separadamente da frequência com que a lógica de jogo atualiza o estado.

## Frente 2 — documentação de hardware

- SMS Power, [Mappers](https://www.smspower.org/Development/Mappers): o mapper Sega convencional documentado tem até seis bits de banco significativos entre as revisões conhecidas, equivalente a 1 MiB.
- SMS Power, [discussão de mapper customizado](https://www.smspower.org/forums/17547-MakingASEGAcardAndMapperThisTimeFullyOpen): descreve uma implementação customizável com linhas de endereço suficientes para chegar a 4 MiB; a placa inicial construída no tópico estava limitada a 512 KiB.

Conclusão documental: 4 MiB não deve ser tratado como capacidade garantida de todo cartucho Sega original. O suporte precisa ser um perfil estendido/customizado, separado do mapper padrão.

## Frente 3 — modelo implementado

O perfil `SegaExtended`:

- É selecionado para ROMs acima de 1 MiB, alinhadas a 16 KiB e com assinatura `TMR SEGA`.
- Preserva os registradores de banco `0xFFFD..0xFFFF` e seus oito bits.
- Mantém o primeiro 1 KiB da janela inicial fixo, como no perfil Sega.
- Permite até 256 bancos de 16 KiB (4 MiB).
- Rejeita imagens acima de 4 MiB sem truncamento.
- Não altera o limite de 1 MiB do perfil Sega padrão.
- Não altera CPU, Scheduler, T-states, VDP, PSG ou apresentação.

ROMs sem a assinatura esperada não recebem automaticamente o perfil estendido.

## Validação

Testes sintéticos cobrem seleção do mapper, bancos `0x80` e `0xFF`, preservação da janela fixa e rejeição acima de 4 MiB. Testes Python cobrem carregamento de ROM 4 MiB solta e dentro de ZIP.

Ainda é necessário executar Flashback 0.0.7 em Windows com a build integrada e registrar:

- FPS do VDP;
- frames publicados e apresentados pelo D3D12;
- erros do worker Rust;
- atividade e erros da sessão WASAPI/XAudio2.

Nenhuma ROM foi adicionada ao repositório.
