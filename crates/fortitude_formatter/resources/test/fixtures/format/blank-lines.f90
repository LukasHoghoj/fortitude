

module m


  use iso_c_binding
   
  implicit none



contains

! comment after blank
subroutine s(a)

integer :: a


a = 1
    

#ifdef X

a = 2

#endif

call foo(a, &

  b)

end subroutine


end module


