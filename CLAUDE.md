# CLAUDE.md

This repository is a **Tauri desktop application** for Windows and macOS (React frontend, Rust native backend).

> 📐 For architecture, IPC details, build/release workflow, and platform specifics, see [SPEC.md](SPEC.md).

## Commands

```bash
pnpm dev
pnpm tauri:dev
pnpm build
pnpm tauri:build
pnpm lint
pnpm format
```

## Conventions

- **Do not use em-dashes** (—) anywhere: in prose, docs, code, or commit messages. Use commas, colons, parentheses, or separate sentences instead.
- **Package manager is pnpm**: do not use npm or yarn.
- The build flow is **Tauri-first**; Electron has been removed (no `src/main/` host code).
- Update native deps in `src-tauri/Cargo.toml`, frontend deps in `package.json`.

## Documentation Lookups

- **Always use the Context7 MCP** to fetch current documentation when working with any
  library, framework, SDK, API, CLI tool, or cloud service, even well-known ones.
  Prefer it over web search and over relying on training data.
- Workflow: `resolve-library-id` → pick the best match → `query-docs` with the full question → answer from the fetched docs.
- Skip Context7 for refactoring, writing scripts from scratch, debugging business logic, code review, and general programming concepts.
