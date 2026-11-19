# uncommented-preprocessor-label (E002)
Fix is sometimes available.

This rule is unstable and in [preview](../preview.md). The `--preview` flag is required for use.

## What it does
Checks for text after `#endif` or `#else` that isn't in a comment, such as
`#endif USE_MPI`.

## Why is this bad?
The C standard doesn't allow anything after `#endif` or `#else` except a
comment. Most preprocessors accept it anyway, but GCC warns about it
(`-Wendif-labels`), and Fortitude's parser can't read it: the rest of the
file is reported as a syntax error, and fixes can't be applied to it.

Writing the label as a C comment keeps it for the reader, and every
preprocessor removes it. The fix uses `/* */` rather than `//`, as
preprocessors in traditional mode, such as gfortran's, don't treat `//` as
a comment: it's Fortran's string concatenation operator.

Unlike most fixes, this one is applied even though the file has syntax
errors, as it's what repairs them.

## Example
```f90
#ifdef USE_MPI
  call mpi_init(ierr)
#endif USE_MPI
```

Use instead:
```f90
#ifdef USE_MPI
  call mpi_init(ierr)
#endif /* USE_MPI */
```
