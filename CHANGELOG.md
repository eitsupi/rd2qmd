# Changelog

## [Unreleased]

## [0.5.0] - 2026-07-26

### Changed

- **Breaking:** Migrate Rd parsing from the in-tree `rd-parser` crate to `rd-ast`/`rd-source` from the [r-documentation-rs](<https://github.com/eitsupi/r-documentation-rs>) (#50):
  - `rd2qmd-core`'s public API now operates directly on `rd_ast::RdDocument` (`convert_rd_document`, `rd_to_mdast`); the previous `RdConverter`/`convert_rd_content` API and all `rd_parser` re-exports are removed.
  - Bump the AST JSON envelope format version from 1 to 2 to match the new document shape.
  - Remove the `lifecycle` and `roxygen` Cargo library feature flags; roxygen2 fenced-code-block detection is now always enabled.
- Update the `rd-ast` and `rd-source` dependencies to `0.1.0`.
- Recoverable parser diagnostics are now surfaced end-to-end through the CLI, package converter, and topic index instead of being silently dropped. (#50)

### Added

- Add `RdConvertOptions::source_files_override` so library callers converting an externally produced `RdAstEnvelope` can preserve its authoritative `source_files` instead of having it silently re-derived from the AST. (#50)

### Fixed

- Complete uniform suppression of the duplicate Rd title heading when YAML frontmatter is enabled across all output formats. (#49)
- Preserve special-character macros, encoded text, and link/S4-class/DOI display text when extracting frontmatter metadata. (#49)
- Enable a TLS backend for `reqwest` so external link resolution can actually fetch HTTPS `pkgdown.yml` sites instead of silently failing. (#49)
- Escape inline code spans that start with `r` plus whitespace so Quarto's knitr engine doesn't misinterpret literal code as executable inline R. (#49)
- Fix several conversion bugs found during the rd-parser migration review (#50):
  - Table-cell/equation whitespace handling now collapses only line endings, preserving intentional same-line spacing and ASCII-equation layout.
  - Link/image destinations containing whitespace, control characters, angle brackets, or parentheses are now wrapped in `<...>` for valid CommonMark; titles have line endings flattened and quotes/backslashes escaped.
  - `\tabular`, `\describe`, and `\preformatted` content nested inside `\arguments` is no longer silently dropped under the grid-table/pipe-table formats, and a list item's later block children are no longer dropped.
  - Pipe characters in table cells (including tables and definition lists nested inside `\arguments`) are now escaped correctly instead of being corrupted.
  - `\method`/`\S3method`/`\S4method` appearing outside `\usage` now render as their bare generic call instead of being silently dropped.

## [0.4.0] - 2026-07-05

### Added

- Add a `parse` subcommand that parses Rd files (single file or directory) to AST JSON instead of converting them directly, so external tooling can inspect or rewrite the parsed document before running `convert`. The output is a versioned envelope (`{"version": 1, "source": ..., "sourceFiles": ..., "document": ...}`) around an output-format-independent AST — links keep their raw Rd semantics, with URL resolution and `.qmd`/`.md` extensions applied only at conversion time. (#43)
- Add `--input-format <rd|ast>` to `convert` and `index` to read AST JSON produced by `parse` instead of `.Rd` files (a `.json` input is auto-detected in single-file mode without the flag). Directory-mode features such as alias/internal-link resolution work identically with AST input. (#43)
- Add `--prefer-ascii-math` option (config: `output.prefer_ascii_math`) to prefer the plain-text representation of `\eqn{latex}{ascii}` / `\deqn{latex}{ascii}` equations over LaTeX math, for renderers without math support such as terminal pagers. `\eqn` is output as inline code and `\deqn` as a plain code block. (#39)

### Changed

- **Breaking:** Redesign the link resolution options around the two Rd link classes (qualified `\link[pkg]{topic}` and unqualified `\link{topic}`), replacing the previous mechanism-based options:

  | Old | New |
  |-----|-----|
  | `--unresolved-link-url` / `links.unresolved_url` | `--unqualified-link-url` / `links.unqualified_link_url` |
  | `--external-package-fallback` / `external.fallback_url` | `--external-link-url` / `links.external_link_url` |
  | `external_package_urls` (base URL map, library API only) | `links.package_urls` (full URL template map with `{topic}` placeholder, now also configurable in `_rd2qmd.toml`) |
  | `LinkOptions::output_extension` (library API) | `links.internal_link_url` (full URL template with `{file}` placeholder; defaults to `{file}.<output extension>`) |

  Qualified links resolve through `package_urls`, then `--external-link-url` (default: `https://rdrr.io/pkg/{package}/man/{topic}.html`, now applied even when external link resolution is disabled), then inline code. Unqualified links resolve through the alias index rendered with `internal_link_url`, then `--unqualified-link-url` (default: `https://rdrr.io/r/base/{topic}.html`), then inline code. Both fallbacks can be disabled with `--no-external-link-url` / `--no-unqualified-link-url`.

  External link resolution no longer synthesizes fallback URLs itself; packages it cannot resolve now fall back to `--external-link-url`. Manually specified `links.package_urls` entries take precedence over automatically resolved ones. (#37)

- Links whose text equals their URL (e.g. from `\url{}`) are now written as CommonMark autolinks (`<https://example.com>` instead of `[https://example.com](https://example.com)`), so renderers that display link URLs alongside the text no longer show the URL twice. (#38)

- **Breaking:** Conversion is now a `convert` subcommand instead of the top-level command, e.g. `rd2qmd man/ -o docs/` becomes `rd2qmd convert man/ -o docs/`. Running `rd2qmd` with no arguments now shows the help text instead of erroring. `-v`/`--verbose` and `-q`/`--quiet` are now global flags accepted by every subcommand (e.g. `rd2qmd index -v`). (#42)

### Fixed

- Preserve whitespace following `\eqn` and `\deqn` macros when their optional second argument is absent. (#40)

## [0.3.0] - 2026-07-02

### Changed

- **Breaking:** `\if{html}{...}` blocks are now excluded from Markdown output by default. Use `--include-html-output` to restore the previous behavior for HTML-capable renderers such as Quarto HTML (#35).

## [0.2.0] - 2026-06-14

### Added

- Add `list` format for arguments output (#28).

### Changed

- **Breaking:** Rename `--arguments-table` and `arguments_table` to `--arguments-format` and `arguments_format` (#28).
- **Breaking:** Rename `grid` and `pipe` values to `grid-table` and `pipe-table`, and change the default arguments output format to `list-table` (#27).

### Fixed

- Preserve nested lists, tables, definition lists, and multi-paragraph content in argument descriptions (#28).

## [0.1.1] - 2026-05-19

### Fixed

- Fix macro name parsing for `\dots)`, `\ldots,` etc. where trailing punctuation was incorrectly included in the macro name (#21)
- Fix zero-arg macro terminators (`\dots{}`, `\tab{}`, `\cr{}`) no longer emit spurious `{}` in output (#21)
- Preserve literal braces in `\usage{}` and `\examples{}` sections (#21)

## [0.1.0] - 2026-04-04

Initial release.
