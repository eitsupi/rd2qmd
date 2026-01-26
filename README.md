# rd2qmd

A fast Rd-to-Quarto Markdown converter written in Rust, with intelligent link resolution.

## Features

- **Blazing fast**: Converts 228 ggplot2 docs in ~0.13s with parallel processing
- **Smart link resolution**: Automatically resolves `\link{}` references to correct output files
- **External package links**: Resolves cross-package links using pkgdown URL conventions (e.g., `\link[dplyr]{mutate}` → `https://dplyr.tidyverse.org/reference/mutate.html`)
- **Quarto-ready**: Generates `.qmd` files with `{r}` executable code blocks and YAML frontmatter
- **Grid Table support**: Uses Pandoc-compatible Grid Tables for Arguments section, supporting lists and block elements in cells
- **pkgdown-compatible metadata**: Adds `pagetitle` in pkgdown style (`"<title> — <name>"`) for SEO
- **No R required**: Pure Rust binary with no runtime R dependency

## Installation

### From source

```bash
cargo install --git https://github.com/eitsupi/rd2qmd
```

### Build locally

```bash
git clone https://github.com/eitsupi/rd2qmd
cd rd2qmd
cargo build --release
```

The binary will be available at `target/release/rd2qmd`.

## Usage

### Basic usage

```bash
# Convert a single file (outputs to same directory)
rd2qmd file.Rd

# Convert to a specific output file
rd2qmd file.Rd -o output.qmd

# Convert to standard Markdown instead of Quarto
rd2qmd file.Rd -f md
```

### Directory conversion

When converting a directory, rd2qmd builds an alias index for internal link resolution:

```bash
# Convert all .Rd files in a directory
rd2qmd man/ -o docs/

# Process subdirectories recursively
rd2qmd man/ -o docs/ -r

# Use parallel processing with 4 jobs
rd2qmd man/ -o docs/ -j4
```

### Options

| Option | Description |
|--------|-------------|
| `-o, --output <PATH>` | Output file or directory |
| `-f, --format <FORMAT>` | Output format: `qmd` (default) or `md` |
| `-j, --jobs <N>` | Number of parallel jobs (defaults to CPU count) |
| `-r, --recursive` | Process directories recursively |
| `--no-frontmatter` | Disable YAML frontmatter |
| `--no-pagetitle` | Skip pkgdown-style `pagetitle` metadata (`"<title> — <name>"`) |
| `--quarto-code-blocks <BOOL>` | Use `{r}` code blocks (auto-set based on format) |
| `--arguments-table <FORMAT>` | Arguments table format: `grid` (default) or `pipe` |
| `-v, --verbose` | Verbose output |
| `-q, --quiet` | Only show errors |

### Link resolution options

| Option | Description |
|--------|-------------|
| `--unresolved-link-url <URL>` | URL pattern for unresolved links. Default: `https://rdrr.io/r/base/{topic}.html` |
| `--no-unresolved-link-url` | Disable fallback URL for unresolved links |

### External link options

These options require the `external-links` feature (enabled by default):

| Option | Description |
|--------|-------------|
| `--r-lib-path <PATH>` | R library path to search for packages (repeatable) |
| `--cache-dir <DIR>` | Cache directory for pkgdown.yml files |
| `--no-external-links` | Disable external package link resolution |
| `--external-package-fallback <URL>` | Fallback URL for packages without pkgdown sites. Default: `https://rdrr.io/pkg/{package}/man/{topic}.html` |

You can get your R library paths by running `.libPaths()` in R:

```r
.libPaths()
#> [1] "/home/user/R/x86_64-pc-linux-gnu-library/4.4"
#> [2] "/usr/local/lib/R/site-library"
#> [3] "/usr/lib/R/library"
```

### Example control options

These options control how `\dontrun{}` and `\donttest{}` example code is handled:

| Option | Description |
|--------|-------------|
| `--exec-dontrun` | Make `\dontrun{}` code executable (`{r}` blocks) |
| `--no-exec-donttest` | Make `\donttest{}` code non-executable (` ```r ` blocks) |

Default behavior (pkgdown-compatible):
- `\dontrun{}` → non-executable (` ```r `), because it means "never run this code"
- `\donttest{}` → executable (`{r}`), because it means "don't run during testing" but should run normally

Use `--exec-dontrun` to make `\dontrun{}` code executable, or `--no-exec-donttest` to make `\donttest{}` code non-executable.

## Output formats

### Quarto Markdown (`.qmd`)

The default format produces Quarto-compatible markdown with:
- YAML frontmatter with title and pagetitle (pkgdown style: `"<title> — <name>"`)
- Executable R code blocks using `{r}` syntax
- Internal links resolved to `.qmd` files

### Standard Markdown (`.md`)

Use `-f md` for standard markdown with:
- YAML frontmatter with title and pagetitle
- Plain `r` code blocks (non-executable)
- Internal links resolved to `.md` files

### Arguments table format

The Arguments section is rendered as a table. By default, rd2qmd uses **Pandoc Grid Tables** which support block elements (lists, multiple paragraphs) within cells:

```markdown
+----------+-------------------------------------+
| Argument | Description                         |
+==========+=====================================+
| `x`      | A simple description.               |
+----------+-------------------------------------+
| `opts`   | Available options:                  |
|          |                                     |
|          | - option A                          |
|          | - option B                          |
+----------+-------------------------------------+
```

For Markdown environments that don't support Grid Tables, use `--arguments-table pipe` for pipe tables:

```markdown
| Argument | Description |
|:---|:---|
| `x` | A simple description. |
| `opts` | Available options: <br>- option A <br>- option B |
```

Note: GFM tables cannot contain true block elements; lists are flattened with `<br>` separators.

## Examples

Convert ggplot2 documentation to Quarto:

```bash
rd2qmd ggplot2/man/ -o docs/reference/ -v
```

Convert a single function's documentation:

```bash
rd2qmd ggplot2/man/geom_point.Rd -o geom_point.qmd
```

Convert to standard Markdown for a static site:

```bash
rd2qmd ggplot2/man/ -o docs/ -f md
```

## Performance

rd2qmd is designed to be fast. Here are benchmark results converting ggplot2's documentation (228 Rd files, 734 aliases, 16 external package references):

| Configuration                      | 1 job | 2 jobs | 4 jobs |
|------------------------------------|-------|--------|--------|
| Without external link resolution   | 0.19s | 0.16s  | 0.13s  |
| With external links (warm cache)   | 0.80s | 0.74s  | 0.71s  |
| With external links (cold cache)   | ~1.0s | -      | -      |

Notes:

- External link resolution fetches pkgdown.yml from package websites on first run (cold cache)
- Cached results are reused on subsequent runs (warm cache)
- In CI environments with 2 cores, expect ~0.16s without external links or ~0.74s with cached external links

Run your own benchmark:

```bash
git clone --depth 1 https://github.com/tidyverse/ggplot2 /tmp/ggplot2
cargo run --release --example benchmark -- /tmp/ggplot2/man --r-lib-path $(Rscript -e 'cat(.libPaths()[1])')
```

## Roadmap

- Integration with [tree-sitter-qmd](https://github.com/quarto-dev/quarto-markdown) for syntax-aware Quarto document manipulation

## Related projects

This project is built on:

- [markdown-rs](https://github.com/wooorm/markdown-rs) - Markdown parser in Rust. rd2qmd uses its mdast (Markdown AST) types for intermediate representation.
- [r-description-rs](https://github.com/jelmer/r-description-rs) - R DESCRIPTION file parser in Rust, used for reading package metadata.

This project was inspired by:

- [pkgdown](https://pkgdown.r-lib.org/) - The standard tool for building R package documentation websites. rd2qmd follows pkgdown's URL conventions for external links and its semantics for `\dontrun{}`/`\donttest{}` example handling.
- [altdoc](https://altdoc.etiennebacher.com/) - A lightweight alternative to pkgdown supporting Quarto, Docsify, Docute, and MkDocs. Converts Rd to qmd via R's `tools::Rd2HTML()`.
- [pkgsite](https://github.com/edgararuiz/pkgsite) - A Quarto-based R package documentation generator that converts Rd files to Quarto documents.
- [downlit](https://downlit.r-lib.org/) - Syntax highlighting and automatic linking for R code, used by pkgdown.
- [rd2md](https://github.com/coatless-rpkg/rd2md) - A Python-based Rd to Markdown converter.
- [rd2markdown](https://github.com/Genentech/rd2markdown) - An R package for Rd to Markdown conversion.

## License

MIT
