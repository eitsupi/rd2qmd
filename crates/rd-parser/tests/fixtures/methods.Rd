% Rd file demonstrating S3/S4 method declarations
\name{print.myclass}
\alias{print.myclass}
\alias{summary.myclass}
\title{Methods for myclass}
\description{
Print and summary methods for myclass objects.
}
\usage{
\method{print}{myclass}(x, ...)
\method{summary}{myclass}(object, digits = 3, ...)
\S4method{show}{MyS4Class}(object)
}
\arguments{
\item{x}{An object of class myclass.}
\item{object}{An object of class myclass or MyS4Class.}
\item{digits}{Number of digits for display.}
\item{...}{Additional arguments.}
}
