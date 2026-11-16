program brackets_and_not
  logical :: a, b
  integer :: x, y
  if ( .not. some_cond()) a = .not. b
  a = (.not. a) .and. .not. b
  a = b .or..not.a
  if (.not.(a .and. b)) a = b .eqv. .not. a
  if ((a .and. b) ) x = 1
  x = f((x + y) )
  x = ((x))
  if ((a)) then
  end if
  call g(x, (y) )
  y = (x*(y + (x)) )
end program brackets_and_not
