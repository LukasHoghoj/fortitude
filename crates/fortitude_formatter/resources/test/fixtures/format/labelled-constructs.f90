subroutine f(a, n)
integer :: a(:), n, i
c1: if (n > 0) then
a = 1
else if (n < 0) then c1
a = 2
else c1
a = 3
end if c1
s1: select case (n)
case (1) s1
a = 1
case default s1
a = 0
end select s1
w1: where (a > 0)
a = 1
elsewhere (a < 0) w1
a = 2
elsewhere w1
a = 0
end where w1
b1: block
a = 1
end block b1
a1: associate (y => a)
y = 1
end associate a1
k1: critical
a = 1
end critical k1
do 10 i = 1, n
a(i) = i
10 continue
do 20 i = 1, n
20 end do
forall (i = 1:n)
end forall
change team (t)
a = 1
end team
select rank (a)
rank (0)
n = 1
rank default
n = 0
end select
end subroutine
