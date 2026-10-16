subroutine s(c)
character c*4, d*(*)
double precision :: x
go to 10
go to (10, 20) m
10 continue
print *, "abc " "def"
end subroutine
