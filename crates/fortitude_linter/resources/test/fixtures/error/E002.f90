module test_uncommented_preprocessor_label
  implicit none
contains
  subroutine s(x)
    integer, intent(inout) :: x

! Pass: no label, or a label in a comment
#ifdef USE_MPI
    x = x + 1
#endif
#ifdef USE_MPI
    x = x + 1
#else /* USE_MPI */
    x = x - 1
#endif /* USE_MPI */
#ifdef USE_MPI
    x = x + 1
#endif // USE_MPI

! Fail: bare labels
#ifdef USE_MPI
    x = x + 1
#endif USE_MPI
#ifdef USE_MPI
    x = x + 1
#else USE_MPI
    x = x - 1
#endif USE_MPI

! Fail: indented directive, label followed by a comment
#if defined(USE_MPI) && VERSION > 2
    x = x + 1
  #  endif USE_MPI && VERSION > 2 /* parallel */

! Fail: no fix, as the label contains `*/`
#ifdef USE_MPI
    x = x + 1
#endif USE_MPI */
  end subroutine s
end module test_uncommented_preprocessor_label
