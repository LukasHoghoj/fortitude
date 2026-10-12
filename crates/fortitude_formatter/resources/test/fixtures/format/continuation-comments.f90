subroutine foo(a, b, c)
integer :: a, b, c
call bar(a, & ! the first argument
! a comment line between continuation lines
         b, &

         & c) ! trailing comment
a = b &
  + c ! not continued &
x = 1
end subroutine
