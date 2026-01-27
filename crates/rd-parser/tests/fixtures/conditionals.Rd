% Rd file demonstrating conditionals
\name{conditionals}
\title{Conditional Examples}
\description{
Conditional content for different output formats.
}
\details{
HTML only: \if{html}{This appears in HTML output.}

With else: \ifelse{latex}{LaTeX version}{Other version}

Sexpr: \Sexpr[results=rd]{paste("Dynamic", "content")}

Raw output: \out{<b>raw HTML</b>}
}
