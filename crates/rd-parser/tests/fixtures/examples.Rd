% Rd file demonstrating example control macros
\name{examples}
\title{Example Control}
\description{
Demonstrates example control macros.
}
\examples{
# Normal example
x <- 1:10
mean(x)

\dontrun{
# This won't run automatically
dangerous_operation()
}

\donttest{
# This won't be tested
slow_function(x)
}

\dontshow{
# Hidden test code
stopifnot(mean(x) == 5.5)
}
}
