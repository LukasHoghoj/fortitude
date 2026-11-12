module test_mismatched_endif_comment
  implicit none
contains
  subroutine s(x)
    integer, intent(inout) :: x

! Pass: no comment
#ifdef USE_MPI
    x = x + 1
#endif

! Pass: matching block comment
#ifdef USE_MPI
    x = x + 1
#endif /* USE_MPI */

! Pass: matching inline comment
#ifdef USE_MPI
    x = x + 1
#endif // USE_MPI

! Pass: #ifndef labelled with the bare name or the negation
#ifndef USE_MPI
    x = x + 1
#endif /* USE_MPI */
#ifndef USE_MPI
    x = x + 1
#endif /* !USE_MPI */

! Pass: defined() and whitespace are ignored
#if defined(USE_MPI) && !defined( USE_OPENMP )
    x = x + 1
#endif /* USE_MPI && !USE_OPENMP */
#if !defined(USE_MPI)
    x = x + 1
#endif /* USE_MPI */
#if (VERSION >= 3)
    x = x + 1
#endif /* VERSION>=3 */

! Pass: prose comments are not checked
#ifdef USE_MPI
    x = x + 1
#endif /* end of MPI support */

! Pass: comment after the condition, not after #endif
#ifdef USE_MPI /* USE_OPENMP */
    x = x + 1
#endif

! Pass: nested blocks, each matching
#ifdef USE_MPI
#if VERSION > 2
    x = x + 1
#endif /* VERSION > 2 */
#else
    x = x - 1
#endif /* USE_MPI */

! Fail: wrong name
#ifdef USE_MPI
    x = x + 1
#endif /* USE_OPENMP */

! Fail: wrong name, inline comment
#ifdef USE_MPI
    x = x + 1
#endif // USE_OPENMP

! Fail: #ifdef labelled with the negation
#ifdef USE_MPI
    x = x + 1
#endif /* !USE_MPI */

! Fail: condition changed
#if defined(USE_MPI) && defined(USE_OPENMP)
    x = x + 1
#endif /* USE_MPI */

! Fail: inner block labelled with the outer condition
#ifdef USE_MPI
#ifdef USE_OPENMP
    x = x + 1
#endif /* USE_MPI */
#endif /* USE_MPI */
  end subroutine s
end module test_mismatched_endif_comment
