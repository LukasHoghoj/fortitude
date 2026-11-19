subroutine f(a, n)
integer :: a(:), n, i, j
do i = 1, n
do j = 1, n
if (a(i) > j) goto 8888
a(i) = a(i) + &
10 * j
8888 continue
end do
end do
10 format(i0)
   20 a = 1 + &
  2
do 30 i = 1, n
a(i) = i
30 continue
if (n > 0) then
12345 n = n - 1 ! trailing comment
end if
end subroutine f
