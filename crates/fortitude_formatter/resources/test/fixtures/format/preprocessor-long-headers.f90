module m
contains
#ifdef LONG
subroutine a_rather_long_name(first_argument, second_argument, third_argument)
#else
subroutine a_rather_long_name(first_argument, second_argument, third_argument, fourth_argument)
#endif
integer :: first_argument
end subroutine
end module m
