# rd2qmd

Convert R documentation files (`.Rd`) to Quarto Markdown (`.qmd`) or standard Markdown (`.md`).

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

## Related projects

This project is built on:

- [markdown-rs](https://github.com/wooorm/markdown-rs) - Markdown parser in Rust. rd2qmd uses its mdast (Markdown AST) types for intermediate representation.
- [r-description-rs](https://github.com/jelmer/r-description-rs) - R DESCRIPTION file parser in Rust, used for reading package metadata.

This project was inspired by:

- [pkgdown](https://pkgdown.r-lib.org/) - The standard tool for building R package documentation websites. rd2qmd's external link resolution follows pkgdown's URL conventions.
- [downlit](https://downlit.r-lib.org/) - Syntax highlighting and automatic linking for R code, used by pkgdown.
- [rd2md](https://github.com/coatless-rpkg/rd2md) - A Python-based Rd to Markdown converter.
- [rd2markdown](https://github.com/Genentech/rd2markdown) - An R package for Rd to Markdown conversion.

## License

MIT
