# Auditoria da fronteira SMS → D3D12

Data: 2026-11-05  
Base examinada: `feat/sms-temporal-qt-control-20261105`  
Escopo: caminho de frame entre a máquina SMS temporal, o worker Rust e o host de vídeo D3D12.

## Evidências do código

1. `SmsTemporalMachine::framebuffer()` retorna uma fatia imutável de bytes RGB24 do VDP. O teste `step_frame_returns_a_complete_sms1_rgb24_frame` confirma um frame de 256 × 192 pixels, totalizando 147.456 bytes.
2. `SmsTemporalWorker::execute_frame()` avança a máquina até completar um frame, atualiza telemetria e drena/processa amostras de áudio. Atualmente não copia nem publica o framebuffer para o runtime de vídeo.
3. A classe PyO3 `NativeSMSTemporalMachine` não expõe framebuffer nem pixels, o que preserva a fronteira arquitetural desejada.
4. `sega-video-runtime::VideoFrame` é o contrato de frame imutável, com sequência, dimensões, formato, stride e armazenamento compartilhado. Os formatos existentes são RGB565 little-endian e RGBA8888; RGB24 ainda não é aceito diretamente.
5. `D3d12Host::present_frame()` consome `VideoFrame` e exige que as dimensões coincidam com o swap chain. A conversão para RGBA8888 ocorre no host Rust.
6. `D3d12Session` possui worker de apresentação próprio e uma fila FIFO limitada sem perda. A fila aplica backpressure no ponto de submissão do frame completo.
7. O runtime também contém `FrameMailbox` e `D3d12Host::present_latest_from_mailbox()`, mas a sessão gerenciada `D3d12Session` não usa esse mailbox. Portanto, o comportamento padrão da sessão atual não é Mailbox.
8. A sessão atual cria e possui sua própria janela Win32 no worker D3D12. Não foi identificada nesta fronteira uma API para receber um HWND de viewport Qt existente.

## Lacunas confirmadas

- Não existe ligação entre o framebuffer RGB24 do SMS e `VideoFrame`.
- O worker temporal não possui nem controla uma sessão D3D12.
- Não existe API coarse-grained de ciclo de vida de vídeo no binding PyO3.
- A fila FIFO atual não satisfaz, por si só, a política Mailbox prevista no contrato da interface.
- A sessão D3D12 atual abre janela Win32 própria; a integração como viewport hospedado pela janela Qt ainda não está demonstrada.

## Direção técnica para a próxima entrega

1. Manter o framebuffer dentro do worker Rust. Criar o `VideoFrame` no limite de frame completo, nunca durante dispatch de instrução ou evento do Scheduler.
2. Preservar sequência monotônica e dimensões/metadados explícitos. Para RGB24, escolher conscientemente entre adicionar um formato RGB888 ao contrato e conversor D3D12, ou converter para RGBA8888 no lado Rust antes da submissão. Não converter pixels em Python.
3. Implementar uma sessão de apresentação com política Mailbox explícita (substituição somente de frames ainda não apresentados), mantendo contadores de frames substituídos. Isso não pode descartar ciclos, instruções ou frames de trabalho da máquina.
4. Manter criação, ownership, fences, Present e destruição de recursos D3D12 no worker nativo. O worker de emulação não deve esperar por GPU/fence nem bloquear dentro do Scheduler.
5. Resolver explicitamente o contrato de janela: HWND independente para a primeira prova vertical, ou HWND filho/hosted para viewport Qt. Não declarar viewport Qt integrado enquanto a janela permanecer separada.
6. Expor apenas comandos coarse-grained de inicialização, fechamento e estado do host. Nenhum buffer de pixels ou callback por frame atravessa PyO3.
7. Adicionar testes de sequência, formato/stride, substituição Mailbox, encerramento, janela fechada, falha do dispositivo e regressão de execução contínua versus execução em blocos.

## Critérios de aceite

- Um frame sintético e um frame real SMS1 percorrem VDP → VideoFrame → D3D12 sem Python intermediário.
- O frame apresentado corresponde à sequência mais recente elegível; substituições são contabilizadas como descarte de apresentação.
- O número de frames/ciclos concluídos pela máquina não depende da frequência de Present.
- O fechamento do host encerra worker e recursos sem thread órfã ou deadlock.
- O contrato de janela (standalone ou hosted) é explícito e validado no Windows.
- Os gates Rust, PyO3 e Windows são executados no HEAD final; SKIPPED/NOT RUN não são PASS.

## Fora do escopo desta auditoria

Filtros HLSL, modos de tela, VRR/G-Sync físico, HDR, SDL3 e menus Qt não são comprovados por esta inspeção. Devem ser tratados em incrementos próprios após o primeiro caminho de frame real.
