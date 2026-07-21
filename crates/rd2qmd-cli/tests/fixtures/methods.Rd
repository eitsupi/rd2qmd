\name{methods}
\alias{methods}
\alias{print.myclass}
\alias{print.default}
\alias{show,MyClass-method}
\title{S3 and S4 Method Usage Entries}
\description{
Demonstrates \verb{\method}, \verb{\S3method}, and \verb{\S4method} usage
entries, including operator generics reformatted as infix expressions.
}
\usage{
\method{print}{data.frame}(x, ...)

\method{print}{default}(x, ...)

\S4method{show}{MyClass}(object)

\S4method{coerce}{OldClass,NewClass}(from, to)

\S3method{print}{myclass}(x, ...)

\method{+}{myclass}(e1, e2)

\method{[}{myclass}(x, i, j, ..., drop = TRUE)

\method{[[}{myclass}(x, i, ...)

\method{$}{myclass}(x, name)

\method{==}{myclass}(e1, e2)

\method{\%>\%}{myclass}(lhs, rhs)

arrange(.data, ...)

\method{arrange}{data.frame}(.data, ..., .by_group = FALSE)

\method{arrange}{default}(.data, ...)

\method{print}{tbl_df}(x, ..., n = NULL, width = NULL)

\method{summary}{myclass}(object, ...)
}
\arguments{
\item{x}{An object to print.}
\item{...}{Additional arguments passed to methods.}
\item{object}{An object to show or summarize.}
\item{from, to}{Objects to coerce between classes.}
\item{e1, e2}{Objects for the operator method.}
\item{i, j}{Indices for subsetting.}
\item{drop}{Whether to simplify the result.}
\item{name}{The name of the element to extract.}
\item{lhs, rhs}{Left- and right-hand side operands.}
\item{.data}{A data frame to arrange.}
\item{.by_group}{Whether to sort by grouping columns first.}
\item{n, width}{Number of rows and display width.}
}
\value{
Nothing of interest; this fixture only exercises \verb{\usage} rendering.
}
\examples{
methods(class = "myclass")
}
