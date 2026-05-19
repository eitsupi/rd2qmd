# Changelog

## [Unreleased]

## [0.1.1] - 2026-05-19

### Fixed

- Fix macro name parsing for `\dots)`, `\ldots,` etc. where trailing punctuation was incorrectly included in the macro name (#21)
- Fix zero-arg macro terminators (`\dots{}`, `\tab{}`, `\cr{}`) no longer emit spurious `{}` in output (#21)
- Preserve literal braces in `\usage{}` and `\examples{}` sections (#21)

## [0.1.0] - 2026-04-04

Initial release.
