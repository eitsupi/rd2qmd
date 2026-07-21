\name{tags}
\alias{tags}
\title{See \linkS4class{Foo} and \doi{10.1234/xyz} for details}
\description{
This function demonstrates tags that have no coverage in any other fixture.

See the paper at \doi{10.1234/example} for background.

See \linkS4class{MyClass} for the unqualified form and
\linkS4class[methods]{envRefClass} for the qualified form.

As \cite{R Core Team (2024)} explains, the \abbr{HTML} format is common.

Use \code{\link[=tags]{tags()}} to call this function again, or see
\link[somepkg:sometopic]{a qualified pkg:topic link}.
}
\arguments{
\item{x}{A figure to display: \figure{myplot.png}{alt text here}.}
}
\value{
See \figure{myplot.png} (no alt) and \figure{myplot.png}{options: width=100}
(expert form, no alt key).
}
\examples{
x <- 1
\dontdiff{
# Output varies - don't diff
print(Sys.time())
}
y <- 2
}
