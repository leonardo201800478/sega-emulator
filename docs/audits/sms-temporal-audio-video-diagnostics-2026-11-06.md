# SMS — diagnóstico de áudio, vídeo e carregamento de ROM

Data: 2026-11-06

## Áudio — causa confirmada

O worker temporal SMS inicia sem uma sessão de áudio (`audio_session = None`). A cada frame, ele drena as amostras produzidas pelo PSG, mas só as envia ao host quando uma sessão nativa foi configurada. Sem a chamada `configure_audio_output`, as amostras são descartadas.

A janela mínima anterior carregava a ROM sem configurar uma sessão. A integração desta PR configura WASAPI antes de iniciar a execução e apresenta a falha do endpoint sem bloquear a apresentação de vídeo.

A configuração é coarse-grained; a GUI não recebe PCM e não executa processamento de áudio.

## Carregamento ZIP

A janela anterior lia o caminho `.zip` como se fosse uma ROM binária e o filtro do seletor não oferecia arquivos ZIP. O carregador agora reconhece um ZIP com exatamente um membro compatível (`.sms`, `.rom` ou `.bin`) e carrega o conteúdo descompactado. Arquivos com múltiplas ROMs compatíveis exigem seleção explícita ou extração prévia.

O limite de tamanho do mapper permanece independente do formato do arquivo. ROMs acima do limite do mapper não devem ser aceitas apenas por estarem compactadas.

## Vídeo — observabilidade

A janela expõe:

- contador de frames do runtime;
- FPS observado pelo host, calculado por amostras de `frame_number`;
- sequência do último frame publicado no mailbox;
- sequência do último frame apresentado pelo D3D12;
- frames substituídos e recuperações do dispositivo;
- `runtime_error` do worker.

Interpretação:

| Observação | Próximo foco |
|---|---|
| Frames não avançam | start/pause ou worker Z80/Scheduler |
| Frames avançam, publicados = 0 | conexão do mailbox ou publicação de frame |
| Publicados avançam, apresentados = 0 | worker D3D12 / Present |
| Apresentados avançam, tela permanece preta | framebuffer do VDP, conversão de pixel ou viewport |
| `runtime_error` preenchido | erro do worker Rust; não é uma falha de GUI |

O FPS é telemetria do host. Não controla clocks, T-states, Scheduler, frame skipping ou pacing do hardware.

## Validação e limites

- Testes automatizados cobrem configuração de áudio, falha do endpoint, ZIP de membro único e propagação de erro/telemetria.
- O smoke test Qt e a compilação D3D12 não substituem teste visual/sonoro em uma máquina Windows real.
- Ainda é necessário executar uma ROM SMS conhecida e registrar FPS, sequência publicada/apresentada, métricas de áudio e erros.
- Nenhuma ROM comercial foi incluída no repositório.