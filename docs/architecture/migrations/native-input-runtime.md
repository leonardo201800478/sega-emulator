## Objetivo
Migrar captura e processamento de input do runtime para Rust nativo. Python/PySide6 permanece responsável por menus, diálogos e configuração de perfis.

## Escopo
- Substituir polling XInput via Python ctypes por implementação Rust nativa.
- Substituir PySDL3/ctypes em Python por binding Rust SDL3 ou backend nativo equivalente, após avaliação técnica.
- Centralizar em Rust enumeração, polling/eventos, hotplug, abertura/fechamento, estado dos dispositivos, normalização e encaminhamento ao runtime.
- Preservar perfis, mapeamentos, dois jogadores, joystick genérico, botões, eixos, hats e fallback XInput/SDL quando aplicável.
- Integrar teclado e apontadores ao contrato de input nativo; Qt pode capturar eventos da UI, mas eventos de gameplay são normalizados e consumidos por Rust.
- PyO3 limitado a configuração, comandos e snapshots; sem polling por frame via Python.

## Critérios de aceitação
- Nenhuma chamada XInput/SDL de gameplay executada por Python.
- Nenhum polling ou tradução de estado por frame em Python.
- Hotplug, desconexão/reconexão, dispositivos duplicados, ordem de jogadores e fallback cobertos.
- Perfis existentes preservados e compatibilidade comprovada.
- Testes determinísticos e integração Windows com hardware real; latência de input medida.

## Plano
1. Mapear consumidores, perfis e contratos atuais.
2. Implementar backend XInput/SDL nativo reutilizando crates existentes quando adequados.
3. Migrar normalização e despacho ao runtime.
4. Reduzir PyO3 e remover PySDL3/ctypes ativos.
5. Validar regressões e dispositivos físicos.

## Estado
Draft de migração. Este commit estabelece escopo e aceitação; não declara o port implementado nem validado.