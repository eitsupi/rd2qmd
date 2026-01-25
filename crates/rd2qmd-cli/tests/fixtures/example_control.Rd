\name{example_control}
\alias{example_control}
\title{Example control macros test}
\description{
Test file for \verb{\dontrun}, \verb{\donttest}, and \verb{\dontshow} macros.
}
\examples{
# Regular code - always shown, always run
x <- 1

\dontrun{
# dontrun code - shown but not run
x <- 2
}

\donttest{
# donttest code - shown, run by example() but not R CMD check
x <- 3
}

\dontshow{
# dontshow code - not shown, but always run
x <- 4
}

\testonly{
# testonly code - alias for dontshow
x <- 5
}

# After all control macros
print(x)
}
