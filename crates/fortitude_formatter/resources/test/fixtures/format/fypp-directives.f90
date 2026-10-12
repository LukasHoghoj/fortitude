program p
#:if defined('DEBUG')  #! a fypp comment making this line too long
print *, 1
#:else
print *, 2
#:endif
#:for i in range(3)
$:i
#:endfor
end program
