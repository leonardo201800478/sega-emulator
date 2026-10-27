# Contrato global de drivers de entrada, áudio e vídeo

## Objetivo

O menu Qt **Drivers** concentra a seleção das APIs do host. As preferências são globais, persistentes e independentes do console selecionado. Cada máquina Sega deve consumir os mesmos contratos de fronteira, sem acoplar o núcleo de hardware a APIs de sistema operacional.

## Input: API direta e snapshot normalizado

Fluxo obrigatório:

1. O frontend consulta a API escolhida na fronteira de entrada do frame.
2. O adaptador normaliza botões/eixos para um snapshot lógico por jogador.
3. O adaptador da máquina traduz o snapshot para os registradores/portas próprios do console.
4. A CPU e os dispositivos emulados leem apenas o estado latched da máquina.

Não chamar XInput, SDL, DirectInput ou outra API de host por instrução, acesso a byte, callback de barramento ou dentro do núcleo Rust. O estado deve ser transferido em lote e com ordenação determinística.

### APIs implementadas no frontend SG-1000

- `auto`: XInput quando há controle XInput conectado; SDL3 como fallback.
- `xinput`: consulta direta à API XInput do Windows, sem fallback implícito para SDL3.
- `sdl3`: consulta SDL3 Gamepad explicitamente, sem priorizar XInput.

A escolha de input é global, mas o mapeamento final de botões para portas é responsabilidade do adaptador de cada console. Controles de desconexão devem ser tratados no limite do frame, não dentro do núcleo de instruções.

## Áudio

- `wasapi`: saída compartilhada do Windows por PortAudio/sounddevice.
- `disabled`: não abre dispositivo de saída; a máquina ainda pode gerar amostras para diagnóstico/testes.

O núcleo emite eventos/amostras determinísticos em lote. A API de áudio do host só consome o buffer já produzido. Clock, divisor, temporização digital e estado do chip não podem depender da latência do dispositivo de saída.

## Vídeo

- `auto`: usa Vulkan/Qt Quick quando inicializado; se falhar, apresenta por Qt Raster.
- `vulkan`: solicita explicitamente a apresentação Vulkan; se não estiver disponível, a seleção é recusada.
- `raster`: apresenta o framebuffer via Qt Raster.

A API de apresentação não pode alterar o framebuffer ou o estado do VDP. A renderização do hardware pertence ao núcleo; Vulkan/Raster apenas apresenta o quadro concluído.

## Consoles futuros

SG-1000 é o primeiro adaptador. Master System/Game Gear, Mega Drive/Genesis, Sega CD e 32X devem reutilizar as mesmas preferências globais e a fronteira de snapshots, adicionando apenas os mapeamentos de controles e os requisitos de saída específicos de cada máquina. Não anunciar uma API como disponível num console até que o adaptador correspondente e seus testes de integração existam.

## Runtime de emulação

Qt, execução headless e frontend Pygame usam somente o runtime Rust. A implementação Python de referência fica disponível no código de desenvolvimento para testes diferenciais, mas não pode ser selecionada como runtime de produção nem receber fallback silencioso. A extensão Rust ausente ou uma instrução não suportada deve produzir um erro explícito.

## Validação necessária

- Testar as políticas `auto`, `xinput` e `sdl3` com adaptadores falsos e, no Windows, controles reais.
- Testar a persistência das seleções e a migração de valores antigos.
- Testar áudio habilitado/desabilitado, falha ao abrir o dispositivo e continuidade de vídeo/input.
- Testar a seleção raster após cada atualização de framebuffer e a indisponibilidade de Vulkan.
- Validar o contrato para cada console antes de habilitar drivers específicos nele.
