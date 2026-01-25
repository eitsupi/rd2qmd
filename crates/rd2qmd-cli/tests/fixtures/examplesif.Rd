\name{examplesif}
\alias{examplesif}
\title{Test @examplesIf pattern}
\description{
Test file for roxygen2's @examplesIf pattern which uses \verb{\dontshow} wrappers.
}
\examples{
# Initialize variable
x <- 0

# @examplesIf FALSE - code wrapped but not executed
\dontshow{if (FALSE) withAutoprint(\{ # examplesIf}
x <- 1
\dontshow{\}) # examplesIf}

# x should still be 0 here
print(paste("After FALSE block:", x))

# @examplesIf TRUE - code wrapped and executed
\dontshow{if (TRUE) withAutoprint(\{ # examplesIf}
x <- 2
\dontshow{\}) # examplesIf}

# x should be 2 here
print(paste("After TRUE block:", x))

# @examplesIf with require() - typical usage pattern
\dontshow{if (require("stats")) withAutoprint(\{ # examplesIf}
# This code runs only if stats package is available
fit <- lm(mpg ~ wt, data = mtcars)
summary(fit)
\dontshow{\}) # examplesIf}

# Final value
print(paste("Final x:", x))
}
