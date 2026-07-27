\name{describe_code_blocks}
\alias{describe_code_blocks}
\title{Describe Blocks With Roxygen Code Fences}
\description{
Demonstrates the Pandoc definition-list writer's handling of multi-line
block children -- roxygen2 fenced code blocks, a nested definition list,
and a bulleted list item that itself contains a fenced code block --
inside definition-list item descriptions. This is the ggproto/R6-style
layout that roxygen2 commonly emits for methods and fields.
}
\section{Methods}{
\describe{
\item{\code{compute()}}{\strong{Usage}

\if{html}{\out{<div class="sourceCode r">}}\preformatted{compute(a, b)
compute(a, b, extra = TRUE)
combine(a, b)
}\if{html}{\out{</div>}}

\strong{Arguments}
\describe{
\item{\code{a}}{The first value.}
\item{\code{b}}{The second value.}
}

\strong{Examples}

\itemize{
\item Basic usage:

\preformatted{compute(1, 2)
#> [1] 3
}
\item Advanced usage returns invisibly.
}

\strong{Ordered examples}

\enumerate{
\item Basic usage:

\preformatted{compute(1, 2)
#> [1] 3
}
\item Advanced usage returns invisibly.
}

That covers the \code{compute()} method.}
}
}
