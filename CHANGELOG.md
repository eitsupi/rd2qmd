# Changelog

## [Unreleased]

### Added

- Add `topic_link_url` option (`--topic-link-url` in CLI) to render help topic links with a URL pattern such as `x-r-help:{package}/{topic}` when other link resolution fails. This preserves link targets even when no alias map or external package URLs are configured.

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
