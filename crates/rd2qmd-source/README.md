# rd2qmd-source

Shared parsing facade for converting raw R documentation (`.Rd`) source into
the producer-independent `rd-ast` document model used by rd2qmd.

The facade treats hard parser errors and error-severity diagnostics as
failures. Warning diagnostics are returned with the parsed document so callers
can report them with their own file or source context.
