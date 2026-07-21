\name{arguments_rich}
\alias{arguments_rich}
\title{Rich Argument Content}
\description{
This function has arguments with rich, nested Markdown-relevant content.
}
\usage{
arguments_rich(x, method, describe_arg, table_arg, pre_only, pre_with_intro,
  backtick_code, fence_collision, cr_break, multi_paragraph,
  itemize_multi_paragraph, ...)
}
\arguments{
\item{x}{A basic argument for comparison.}
\item{method}{The method to use, described as a nested list:
\itemize{
\item \code{"a"}: Use method A, the default.
\item \code{"b"}: Use method B instead.
}
}
\item{describe_arg}{The method to use, described with a definition list:
\describe{
\item{"a"}{Use method A.}
\item{"b"}{Use method B.}
}
}
\item{table_arg}{The argument. See the table:
\tabular{ll}{
  Col1 \tab Col2 \cr
  a \tab 1 \cr
  b \tab 2
}
}
\item{pre_only}{\preformatted{result <- foo(x)
print(result)
}}
\item{pre_with_intro}{Example usage:
\preformatted{result <- foo(x)
print(result)
}}
\item{backtick_code}{Use \code{`a`} or \code{``nested``} for non-syntactic names.}
\item{fence_collision}{Code containing a fence-like line:
\preformatted{```r
x <- 1
```
}}
\item{cr_break}{First line.\cr - hyphen.\cr * asterisk.\cr + plus.\cr 1. ordered period.}
\item{`a`}{A non-syntactic argument name with single backticks.}
\item{``b``}{A label with a double-backtick run.}
\item{multi_paragraph}{First paragraph.

  Second paragraph of the description.}
\item{itemize_multi_paragraph}{Options:
\itemize{
\item First paragraph.

Second paragraph of this item.
\item Another item.
}}
}
\value{
Nothing of interest.
}
\examples{
arguments_rich(1, "a", "a", NULL, NULL, NULL, NULL, NULL, NULL, "x", "y")
}
