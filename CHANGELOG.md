# Changelog

## [Unreleased]

### Added

- Add `--prefer-ascii-math` option (config: `output.prefer_ascii_math`) to prefer the plain-text representation of `\eqn{latex}{ascii}` / `\deqn{latex}{ascii}` equations over LaTeX math, for renderers without math support such as terminal pagers. `\eqn` is output as inline code and `\deqn` as a plain code block.

### Changed

- **Breaking:** Redesign the link resolution options around the two Rd link classes (qualified `\link[pkg]{topic}` and unqualified `\link{topic}`), replacing the previous mechanism-based options:

  | Old | New |
  |-----|-----|
  | `--unresolved-link-url` / `links.unresolved_url` | `--unqualified-link-url` / `links.unqualified_link_url` |
  | `--external-package-fallback` / `external.fallback_url` | `--external-link-url` / `links.external_link_url` |
  | `external_package_urls` (base URL map, library API only) | `links.package_urls` (full URL template map with `{topic}` placeholder, now also configurable in `_rd2qmd.toml`) |
  | `LinkOptions::output_extension` (library API) | `links.internal_link_url` (full URL template with `{file}` placeholder; defaults to `{file}.<output extension>`) |

  Qualified links resolve through `package_urls`, then `--external-link-url` (default: `https://rdrr.io/pkg/{package}/man/{topic}.html`, now applied even when external link resolution is disabled), then inline code. Unqualified links resolve through the alias index rendered with `internal_link_url`, then `--unqualified-link-url` (default: `https://rdrr.io/r/base/{topic}.html`), then inline code. Both fallbacks can be disabled with `--no-external-link-url` / `--no-unqualified-link-url`.

  External link resolution no longer synthesizes fallback URLs itself; packages it cannot resolve now fall back to `--external-link-url`. Manually specified `links.package_urls` entries take precedence over automatically resolved ones.

- Links whose text equals their URL (e.g. from `\url{}`) are now written as CommonMark autolinks (`<https://example.com>` instead of `[https://example.com](https://example.com)`), so renderers that display link URLs alongside the text no longer show the URL twice.

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
