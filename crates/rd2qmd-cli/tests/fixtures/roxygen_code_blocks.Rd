\name{roxygen_code_blocks}
\alias{roxygen_code_blocks}
\title{Roxygen Fenced Code Block Conversion}
\description{
Demonstrates roxygen2 markdown fenced code blocks converted to Quarto
code fences, including a language tag, no language tag, and content
that itself contains backtick runs.
}
\details{
Here is an R code block from roxygen2:

\if{html}{\out{<div class="sourceCode r">}}\preformatted{x <- 1 + 2
y <- x * 3
print(y)
}\if{html}{\out{</div>}}

And some more text after the code block.

Python code block:

\if{html}{\out{<div class="sourceCode python">}}\preformatted{def greet(name):
    return f"Hello, \{name\}!"

print(greet("World"))
}\if{html}{\out{</div>}}

Code block without a language specifier:

\if{html}{\out{<div class="sourceCode">}}\preformatted{plain text
block without language
}\if{html}{\out{</div>}}

Code block whose content contains backtick runs, requiring a longer fence:

\if{html}{\out{<div class="sourceCode r">}}\preformatted{# Code with backticks
x <- "`value`"
y <- "``nested``"
example <- "```triple```"
}\if{html}{\out{</div>}}

Standalone preformatted (not from roxygen2 markdown):

\preformatted{This is a standalone preformatted block
without the if/out wrapper.
}
}
