# Captura de logs de Debug

A captura detalhada é **opt-in por sessão**: iniciar o emulador ou abrir a guia Debug não cria arquivos. O usuário deve marcar explicitamente a opção de captura.

- O handler produtor apenas tenta inserir o LogRecord em uma fila limitada; não serializa nem escreve arquivos na thread que emite o evento.
- Uma thread dedicada formata e grava JSONL em `logs/debug-*.jsonl`.
- Os arquivos giram em 8 MiB, mantendo até três cópias de backup.
- A fila comporta até 2.048 eventos. Quando cheia, novos registros são descartados sem bloquear o produtor; a perda é contabilizada e identificada no painel/arquivo quando a gravação consegue prosseguir.
- A visualização Qt mantém no máximo 2.000 linhas e recebe lotes pela entrega de sinal entre threads.
- Parar a captura remove o handler imediatamente e deixa a thread gravadora esvaziar a fila de forma assíncrona.
- A captura não registra instruções por padrão nem ativa automaticamente o trace Z80. O trace por instrução no núcleo e sua integração ao caminho real de execução do SMS continuam separados e pendentes.
- Logs e snapshots podem conter caminhos, hashes e estado de ROM; devem permanecer locais e nunca ser versionados.

A thread separada remove o I/O de disco e a formatação do caminho produtor, mas a criação do LogRecord e a inserção não bloqueante ainda têm custo. O impacto deve ser medido em workload equivalente antes de alegar custo zero.


A aplicação nativa egui registra transições dos menus, solicitações de fechamento e mudanças de foco no target `sega::ui::menu`. Eventos de repaint (popup solicitado/renderizado/medido e leituras repetidas do HWND raiz) não são registrados; criação e alterações efetivas de limites da janela filha D3D12 permanecem observáveis. Esses eventos reutilizam o subscriber e o writer compartilhados de `sega-observability`; não existe uma segunda fila ou arquivo de diagnóstico exclusivo da GUI.

Quando `SEGA_LOG_FILTER` não está definido, o filtro padrão é `info,sega::audio::pacing=trace,sega::ui::menu=debug`. A diretiva explícita `sega::ui::menu=debug` é necessária porque os eventos de menu são emitidos no nível DEBUG; sem ela, a instrumentação existe no código, mas os eventos são filtrados. Um `SEGA_LOG_FILTER` personalizado substitui o filtro padrão e deve incluir essa diretiva para capturar os menus. A coleta continua opt-in e desligada por padrão.
