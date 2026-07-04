# rd2qmd

A fast Rd-to-Quarto Markdown converter written in Rust, with intelligent link resolution.

## Features

- **Blazing fast**: Converts 228 ggplot2 docs in ~0.13s with parallel processing
- **Smart link resolution**: Automatically resolves `\link{}` references to correct output files
- **External package links**: Resolves cross-package links using pkgdown URL conventions (e.g., `\link[dplyr]{mutate}` → `https://dplyr.tidyverse.org/reference/mutate.html`)
- **Topic index generation**: Outputs JSON index with topic metadata (name, title, aliases, lifecycle) for building reference sites
- **Quarto-ready**: Generates `.qmd` files with `{r}` executable code blocks and YAML frontmatter
- **Flexible Arguments format**: Supports Quarto native List-Tables (default), Pandoc Grid Tables, GFM Pipe Tables, and Markdown loose lists for the Arguments section (`--arguments-format list-table|grid-table|pipe-table|list`)
- **pkgdown-compatible metadata**: Adds `pagetitle` in pkgdown style (`"<title> — <name>"`) for SEO
- **No R required**: Pure Rust binary with no runtime R dependency

## Installation

### Cargo

```sh
cargo install --locked rd2qmd
```

### GitHub Releases

Pre-built binaries for Linux, macOS, and Windows are available on the [Releases](https://github.com/eitsupi/rd2qmd/releases) page. You can also use the installer scripts:

macOS / Linux:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/eitsupi/rd2qmd/releases/latest/download/rd2qmd-installer.sh | sh
```

Windows (PowerShell):

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/eitsupi/rd2qmd/releases/latest/download/rd2qmd-installer.ps1 | iex"
```

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
| `-f, --format <FORMAT>` | Output format: `qmd` (default), `md`, or `rmd` |
| `-j, --jobs <N>` | Number of parallel jobs (defaults to CPU count) |
| `-r, --recursive` | Process directories recursively |
| `--no-frontmatter` | Disable YAML frontmatter |
| `--no-pagetitle` | Skip pkgdown-style `pagetitle` metadata (`"<title> — <name>"`) |
| `--quarto-code-blocks <BOOL>` | Use `{r}` code blocks (auto-set based on format) |
| `--arguments-format <FORMAT>` | Arguments format: `list-table` (default), `grid-table`, `pipe-table`, or `list` |
| `-v, --verbose` | Verbose output |
| `-q, --quiet` | Only show errors |

### Link resolution options

Rd help topic links come in two classes, and each class has its own
resolution chain. Every step is a URL template; `{package}`, `{topic}`, and
`{file}` placeholders are filled per link:

- **Qualified links** (`\link[pkg]{topic}`) name their target package:
  1. `package_urls` map — per-package URL templates, from the config file
     and/or automatic external link resolution (see below)
  2. `--external-link-url` template
  3. plain inline code (link target dropped)
- **Unqualified links** (`\link{topic}`) don't name a package:
  1. the package's own alias index (directory mode), rendered with the
     `internal_link_url` template (default: `{file}.<output extension>`)
  2. `--unqualified-link-url` template
  3. plain inline code (link target dropped)

| Option | Description |
|--------|-------------|
| `--external-link-url <TEMPLATE>` | URL template for qualified links whose package has no known documentation URL. Placeholders: `{package}`, `{topic}`. Default: `https://rdrr.io/pkg/{package}/man/{topic}.html` |
| `--no-external-link-url` | Disable the qualified-link fallback; such links become plain inline code |
| `--unqualified-link-url <TEMPLATE>` | URL template for unqualified links that alias resolution cannot resolve (typically base R topics). Placeholder: `{topic}`. Default: `https://rdrr.io/r/base/{topic}.html` |
| `--no-unqualified-link-url` | Disable the unqualified-link fallback; such links become plain inline code |

The defaults point at [rdrr.io](https://rdrr.io/), an aggregator that hosts
documentation for all CRAN packages, so links keep working in static sites
out of the box. Two common customizations:

- **Point specific packages at their own documentation sites** with a
  `[links.package_urls]` map in the config file (or let external link
  resolution build it automatically):

  ```toml
  [links.package_urls]
  dplyr = "https://dplyr.tidyverse.org/reference/{topic}.html"
  ```

- **Emit a custom URI scheme for documentation viewers**: a Markdown viewer
  with access to R's help system — such as a terminal help browser — can
  intercept the links and open the topics itself. Nothing needs to serve
  these URIs; the scheme is chosen and interpreted by the consumer.

  ```sh
  rd2qmd man/ -o docs/ \
    --external-link-url "x-r-help:{package}/{topic}" \
    --unqualified-link-url "x-r-help:{topic}"
  ```

The config file (`_rd2qmd.toml`) additionally supports
`links.internal_link_url` to override how alias-resolved internal links are
rendered (e.g. `/reference/{file}.html` for a site with its own URL layout).

#### Known limitation: unqualified links cannot be attributed to a package

The Rd source of an unqualified link (`\link{topic}`) does not say which
package owns the topic. R itself resolves this dynamically at display time
by searching the current package first and then the other installed
packages — information a static converter does not have. rd2qmd replicates
the first step exactly (the alias index covers the package's own topics),
but every remaining unqualified link is rendered with the single
`--unqualified-link-url` template, which assumes base R by default. An
unqualified link to a topic from another package — say `\link{lm}`, which
lives in stats — therefore produces a wrong URL
(`https://rdrr.io/r/base/lm.html` instead of `.../r/stats/lm.html`).

In practice this stays small: cross-package links are conventionally
anchored (`\link[stats]{lm}`), and `R CMD check` flags unresolvable links.
Where it matters you can point the template at a search page, or pass
`--no-unqualified-link-url` to prefer plain inline code over a possibly
wrong link. Custom URI scheme consumers are unaffected: a viewer with
access to R's help system receives `x-r-help:{topic}` and performs the
same dynamic search-path resolution R would.

### External link options

External link resolution automatically fills the `package_urls` map:
rd2qmd searches your local R libraries for each referenced package's own
documentation site (pkgdown URL conventions). Packages it cannot resolve
fall back to `--external-link-url`. These options require the
`external-links` feature (enabled by default):

| Option | Description |
|--------|-------------|
| `--r-lib-path <PATH>` | R library path to search for packages (repeatable) |
| `--cache-dir <DIR>` | Cache directory for pkgdown.yml files |
| `--no-external-links` | Disable external package link resolution |

You can get your R library paths by running `.libPaths()` in R:

```r
.libPaths()
#> [1] "/home/user/R/x86_64-pc-linux-gnu-library/4.4"
#> [2] "/usr/local/lib/R/site-library"
#> [3] "/usr/lib/R/library"
```

### Topic index generation

Generate a JSON index of all topics with metadata for building reference sites:

```bash
# Generate index to stdout (pipe to jq for filtering)
rd2qmd index man/
rd2qmd index man/ | jq '.topics[] | select(.lifecycle)'

# Generate index while converting
rd2qmd man/ -o docs/ --topic-index index.json
```

The index includes topic name, output file, title, aliases, and lifecycle stage:

```json
{
  "topics": [
    {
      "name": "my_func",
      "file": "my_func.qmd",
      "title": "My Function",
      "aliases": ["my_func", "myFunc"],
      "lifecycle": "deprecated"
    }
  ]
}
```

The `lifecycle` field is omitted for topics without a lifecycle badge. Supported stages: `experimental`, `stable`, `superseded`, `deprecated`, and legacy stages (`maturing`, `questioning`, `soft_deprecated`, `defunct`, `retired`).

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

### HTML conditional content

Rd files use `\if{html}{...}` to embed content intended only for HTML renderers (CRAN HTML manual, pkgdown, etc.). This often includes raw HTML such as asciicast terminal-output blocks with inline `<span>` styling. By default, rd2qmd **excludes** this content to produce clean Markdown suitable for GitHub, LLMs, and other non-HTML contexts.

Use `--include-html-output` to include `\if{html}{...}` blocks when targeting an HTML-capable renderer such as Quarto HTML output.

`\ifelse{html}{then}{else}` is not affected by this flag — the `then` branch is always used, which preserves lifecycle badges (rendered as Markdown image links) and similar structured content that has a meaningful non-HTML representation.

### Equations

Rd equations carry an optional second argument with a plain-text representation intended for text terminals — `\eqn{\lambda^x}{lambda^x}` — which R's own text help uses. By default, rd2qmd ignores it and outputs LaTeX math (`$...$` for `\eqn`, `$$...$$` for `\deqn`), which suits math-capable renderers such as Quarto.

Use `--prefer-ascii-math` (config: `output.prefer_ascii_math`) when targeting a renderer without math support, such as a terminal pager. Equations with a non-blank ASCII representation are then output as inline code (`\eqn`) or a plain code block (`\deqn`, preserving the layout of multi-line representations such as matrices). Equations without one (e.g. the one-argument form `\eqn{\lambda}`) fall back to LaTeX math as usual.

## Output formats

### Quarto Markdown (`.qmd`)

The default format produces Quarto-compatible markdown with:
- YAML frontmatter with title and pagetitle (pkgdown style: `"<title> — <name>"`)
- Executable R code blocks using `{r}` syntax
- Internal links resolved to `.qmd` files

Use `-f rmd` for R Markdown (`.Rmd`) output with identical content.

### Standard Markdown (`.md`)

Use `-f md` for standard markdown with:
- YAML frontmatter with title and pagetitle
- Plain `r` code blocks (non-executable)
- Internal links resolved to `.md` files

### Arguments format

The Arguments section is rendered using `--arguments-format`. Rd argument descriptions can contain rich content such as lists and multiple paragraphs — `pipe-table` cannot fully represent this and flattens it with `<br>`. Use `--arguments-format` to select the format based on your target renderer:

| | `list-table` (default) | `grid-table` | `pipe-table` | `list` |
|---|---|---|---|---|
| Quarto 1.9+ | ✅ | ✅ | ✅ | ✅ |
| Quarto 2 (q2) | ✅ | ❌ | ✅ | ✅ |
| Plain Pandoc / Quarto < 1.9 | ❌ | ✅ | ✅ | ✅ |
| GFM / general Markdown | ❌ | ❌ | ✅ | ✅ |

**`list-table`** (default) — Quarto native syntax, supports full block elements. Recommended for Quarto 1.9+ and q2 workflows:

```markdown
::: {.list-table header-rows=1}

- - Argument
  - Description

- - `x`
  - A simple description.

- - `opts`
  - Available options:

    - option A
    - option B

:::
```

**`grid-table`** — Pandoc Grid Table syntax, supports block elements. Works with Quarto 1.x and plain Pandoc, but not supported in q2:

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

**`pipe-table`** — Standard GFM pipe table. Broadest rendering compatibility (GitHub, most Markdown renderers), but block elements are flattened with `<br>`:

```markdown
| Argument | Description |
|:---|:---|
| `x` | A simple description. |
| `opts` | Available options: <br>- option A <br>- option B |
```

**`list`** — Markdown loose list with argument names as bold inline code. Supports full block elements and works everywhere (GFM, Pandoc, Quarto):

```markdown
- **`x`**

  A simple description.

- **`opts`**

  Available options:

  - option A
  - option B
```

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

| Configuration                      | 1 job  | 2 jobs | 4 jobs |
|------------------------------------|--------|--------|--------|
| Without external link resolution   | ~0.2s  | ~0.2s  | ~0.2s  |
| With external links (warm cache)   | ~1.0s  | ~0.9s  | ~0.8s  |
| With external links (cold cache)   | ~1.0s  | -      | -      |

Notes:

- External link resolution fetches pkgdown.yml from package websites on first run (cold cache)
- Cached results are reused on subsequent runs (warm cache)
- Actual times vary by environment; parallel speedup depends on I/O characteristics

Run your own benchmark:

```bash
git clone --depth 1 https://github.com/tidyverse/ggplot2 /tmp/ggplot2
cargo run --release --example benchmark -- /tmp/ggplot2/man \
  $(Rscript -e 'cat(paste("--r-lib-path", .libPaths(), collapse=" "))')
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
- [rd2md](https://github.com/comet-ml/rd2md) - A Python-based Rd to Markdown converter.
- [rd2markdown](https://github.com/Genentech/rd2markdown) - An R package for Rd to Markdown conversion.

## License

MIT
