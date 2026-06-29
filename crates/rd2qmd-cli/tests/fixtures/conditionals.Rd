% Rd file demonstrating \if{html} / \ifelse{} handling
% Models patterns found in cli-style roxygen2 documentation
\name{conditionals}
\alias{conditionals}
\title{Conditional Examples}
\description{
Lifecycle badge: \ifelse{html}{\href{https://lifecycle.r-lib.org/articles/stages.html}{\figure{lifecycle-stable.svg}{options: alt='[Stable]'}}}{\strong{[Stable]}}

Conditional content for different output formats.

HTML only text: \if{html}{This appears in HTML renderers only.}

Text only: \if{text}{This appears in plain text output.}
}
\details{
Example with asciicast output (cli-style):
\if{html}{\out{<div class="sourceCode r">}}\preformatted{example_function("hello")
}\if{html}{\out{</div>}}\if{html}{\out{
<div class="asciicast" style="color: #172431;font-family: 'Fira Code',Monaco,Consolas,Menlo,'Bitstream Vera Sans Mono','Powerline Symbols',monospace;line-height: 1.300000"><pre>
#> <span style="color: #859900;">✔</span> Done: <span style="font-style: italic;">hello</span>
</pre></div>
}}
}
