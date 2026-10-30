# Política de versionamento

## Fonte canônica

O arquivo raiz `VERSION` é a fonte canônica da versão do projeto. Toda alteração de versão deve atualizar, no mesmo commit:

- `pyproject.toml` e `src/sega_emulator/__init__.py`;
- `native/z80_core/pyproject.toml` (distribuição da extensão Maturin);
- todos os `native/**/Cargo.toml` (versão dos componentes Rust);
- entradas dos pacotes internos correspondentes nos `Cargo.lock`;
- `CHANGELOG.md` e uma nota em `docs/releases/`.

O comando `python tools/release/check_versions.py` verifica a consistência. A suíte Python também executa esse verificador.

## Convenção

Seguimos Semantic Versioning 2.0.0:

- **MAJOR**: quebra incompatível de contratos públicos, formatos persistidos ou comportamento de integração;
- **MINOR**: capacidade nova compatível ou novo marco funcional;
- **PATCH**: correção compatível sem mudança incompatível de contrato;
- **pré-release**: `MAJOR.MINOR.PATCH-dev.N` durante integração e validação.

A representação Python usa PEP 440 equivalente (por exemplo, `0.2.0-dev.1` corresponde a `0.2.0.dev1`). Cargo mantém a forma SemVer com hífen.

## Fluxo de release

1. Criar branch isolada a partir do SHA atual da `main`.
2. Atualizar `VERSION`, metadados, locks, changelog e nota de release no mesmo conjunto coeso.
3. Executar o verificador de versões, formatadores, linters, testes, build e workflows aplicáveis.
4. Abrir PR; nunca publicar pré-release diretamente na `main`.
5. Criar tag e GitHub Release somente depois de aprovação e validação do SHA final.
6. Não reutilizar número de versão já publicado; incrementar o sufixo `dev.N` para novas pré-releases.

## Reversibilidade

Cada alteração de versão deve permanecer isolada em PR/commit. Antes do merge, basta fechar a PR. Depois do merge, reverter o commit de versionamento com `git revert <sha>`; não usar reset ou force-push. Releases/tag já distribuídas não devem ser apagadas: publicar uma nova versão corretiva.

## Evidência e escopo

Uma versão registra o estado do código, não certifica automaticamente compatibilidade de hardware. O changelog deve distinguir funcionalidades integradas, testes automatizados, validação manual e limitações. Mappers ainda não implementados não podem aparecer como concluídos.
