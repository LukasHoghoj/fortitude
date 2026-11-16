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
  x = values(:, :, :, (j + 1):(j + n))
  x = values(:, (j+1):, :(j+n), 1:n, (j):(n):(2), ::2, (j)::2)
  x = s((i):(i))
  if(x>0) 10, 20, 30
  do while(x<y)
    x = x + 1
  end do
  where(values>0) values = 1
  where(values>0)
    values = 1
  elsewhere(values<0)
    values = -1
  end where
10 continue
20 continue
30 continue
end program brackets_and_not
