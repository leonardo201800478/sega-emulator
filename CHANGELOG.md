# Histórico de versões

## Estado de desenvolvimento — 2026-10-29

- Integra a consolidação do Z80 SG-1000 pela [PR #157](https://github.com/leonardo201800478/sega-emulator/pull/157), commit `5d29b1790aa7e8d7af47de9ee2563a1c978b62d7`.
- O candidato `baa56568ca2ccbb3963b86207c49fb12f32ab342` passou nos seis workflows listados na PR; o mantenedor confirmou testes manuais de jogos SG-1000 sem regressões perceptíveis.
- A PR alterou cinco arquivos, nenhum sob `native/sms_core/`. A precisão temporal ciclo a ciclo do SG-1000 não foi declarada.
- O foco de desenvolvimento seguinte é concluir o core SMS: auditoria da matriz de opcodes/semântica ([#158](https://github.com/leonardo201800478/sega-emulator/issues/158)) e auditoria de interrupções/temporalidade ([#159](https://github.com/leonardo201800478/sega-emulator/issues/159)).

# Histórico de versões

## v0.1.0 — Primeiro release SG-1000 (preparação)

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
- Adota licença MIT, declara os metadados de licença no pacote e atualiza a caixa Sobre com versão, escopo, créditos e aviso de independência.
- A publicação continua condicionada à conclusão dos checks no SHA candidato e à revisão final; não se presume que testes locais ou CI tenham passado.


## Próxima versão — em desenvolvimento

### Diagnóstico de áudio / PSG
- Adiciona gravação de comandos do SN76489 em arquivo VGM 1.71 pelo menu **Diagnóstico**.
- Registra valores escritos no PSG com timestamps do clock da máquina, no runtime Rust; a implementação Python anterior está arquivada e não é executada.
- A gravação é destinada à análise dos comandos de áudio e à comparação com capturas do Gearsystem; não altera a síntese nem os filtros de áudio.
- Validação automatizada adicionada para cabeçalho, temporização, intervalos longos, escrita de arquivo e ações do menu Qt.

**Limitações:** ainda requer execução dos testes no CI e validação manual no Windows. A gravação VGM registra writes de PSG, não PCM renderizado nem temporização sub-T-state que o modelo atual não exponha.
