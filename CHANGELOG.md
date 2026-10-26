# Histórico de versões

## Próximo release — SG-1000 inicial (em preparação)

### Marco de áudio e vídeo
- Registra a avaliação manual de 2026-10-26: áudio e vídeo do SG-1000 alcançaram qualidade percebida satisfatória para o primeiro marco; o áudio foi considerado muito próximo ao Gearsystem.
- Documenta explicitamente que essa avaliação é observacional e não certifica equivalência com hardware real, compatibilidade universal ou temporização ciclo a ciclo.
- Mantém como pendência a investigação de pequenos engasgos e possível descompasso durante sessões prolongadas.

### Interface Qt
- Adiciona modo imersivo de tela cheia dedicado à imagem do jogo, ocultando menus, barra de ações, status e margens.
- F11 e Alt+Enter alternam a tela cheia; Esc retorna à janela normal.
- Adiciona teste automatizado para ocultar e restaurar os elementos da interface.

### Preparação do primeiro release
- Atualiza README e cria registro do marco SG-1000, incluindo evidências, limites conhecidos e plano de validação de estabilidade.
- O release permanece pendente da escolha explícita da licença, atualização final da caixa Sobre e validação dos workflows no SHA candidato.


## Próxima versão — em desenvolvimento

### Diagnóstico de áudio / PSG
- Adiciona gravação de comandos do SN76489 em arquivo VGM 1.71 pelo menu **Diagnóstico**.
- Registra valores escritos no PSG com timestamps do clock da máquina, tanto no backend Python quanto no runtime Rust.
- A gravação é destinada à análise dos comandos de áudio e à comparação com capturas do Gearsystem; não altera a síntese nem os filtros de áudio.
- Validação automatizada adicionada para cabeçalho, temporização, intervalos longos, escrita de arquivo e ações do menu Qt.

**Limitações:** ainda requer execução dos testes no CI e validação manual no Windows. A gravação VGM registra writes de PSG, não PCM renderizado nem temporização sub-T-state que o modelo atual não exponha.
