# Referências legadas Python — somente consulta

Este diretório é o destino das implementações de hardware em Python **depois** que cada responsabilidade tiver sido substituída por Rust e seus consumidores migrados.

Regras:
- Nunca importar módulos daqui pelo pacote executável `src/sega_emulator`.
- Nunca tratar esta implementação como fonte oficial de comportamento.
- Preservar avisos de proveniência/licença e o histórico Git ao arquivar arquivos.
- Manter somente material que ainda agregue valor à pesquisa, à validação diferencial ou à investigação de regressões.
- Cada movimentação deve ser acompanhada de busca por imports, ajustes dos testes e execução dos checks aplicáveis.
- Não arquivar configuração da interface, logs da aplicação, carregamento de arquivos ou ferramentas Python que continuam sendo responsabilidades legítimas do frontend.

O diretório é apenas a área de destino da migração. Nenhuma implementação ativa foi movida nesta etapa documental; os módulos atuais continuam no local original até serem substituídos e validados individualmente.
