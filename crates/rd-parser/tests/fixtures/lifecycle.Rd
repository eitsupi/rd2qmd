% Test fixture for lifecycle badge detection
\name{lifecycle}
\alias{lifecycle_deprecated}
\alias{lifecycle_example}
\title{Lifecycle Badge Examples}
\description{
\ifelse{html}{\href{https://lifecycle.r-lib.org/articles/stages.html#deprecated}{\figure{lifecycle-deprecated.svg}{options: alt='[Deprecated]'}}}{\strong{[Deprecated]}}

This function demonstrates a deprecated lifecycle badge.
The lifecycle badge is typically placed at the start of the description.
}
\usage{
lifecycle_deprecated(x)
}
\arguments{
\item{x}{An input value. Note: lifecycle badges in arguments are not
detected by the lifecycle extraction (following pkgdown convention).}
}
\details{
More details about the deprecated function.

Use \code{new_function()} instead.
}
\value{
Returns the input unchanged.
}
\seealso{
\code{\link{new_function}} for the replacement.
}
\examples{
\dontrun{
# This function is deprecated
lifecycle_deprecated(1)
}
}
\keyword{internal}
