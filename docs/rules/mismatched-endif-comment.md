# mismatched-endif-comment (S331)
Fix is always available.

This rule is unstable and in [preview](../preview.md). The `--preview` flag is required for use.

## What it does
Checks that the comment after an `#endif` matches the condition of the
`#if`, `#ifdef` or `#ifndef` that opens the block.

## Why is this bad?
A comment after `#endif` helps to find the start of a long preprocessor
block. When the condition is renamed or changed and the comment is not, the
comment points to the wrong block.

When comparing, `defined(X)` and `defined X` are treated as `X`, and
whitespace is ignored. The block of an `#ifndef X` may be labelled either
`X` or `!X`. Comments that read as a description rather than a condition,
such as `/* end of MPI support */`, are not checked, and neither is an
`#endif` without a comment.

## Example
```f90
#ifdef USE_MPI
  call mpi_init(ierr)
#endif /* USE_OPENMP */
```

Use instead:
```f90
#ifdef USE_MPI
  call mpi_init(ierr)
#endif /* USE_MPI */
```

## Fix safety
This fix is unsafe, as the comment may be correct and the condition wrong.
