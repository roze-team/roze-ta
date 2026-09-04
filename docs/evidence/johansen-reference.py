"""Independent two-variable canonical roots via scalar 2x2 determinant formula."""
import math
x=[[i*.1+math.sin(i*1.3),i*.13+math.cos(i*.7)]for i in range(40)]
delta=[[b-a for a,b in zip(x[i-1],x[i])]for i in range(1,len(x))]
levels=x[:-1]
def center(a):
    means=[sum(r[j]for r in a)/len(a)for j in range(2)]
    return [[r[j]-means[j]for j in range(2)]for r in a]
def cross(a,b):return [[sum(r[i]*s[j]for r,s in zip(a,b))/len(a)for j in range(2)]for i in range(2)]
def mul(a,b):return [[sum(a[i][k]*b[k][j]for k in range(2))for j in range(2)]for i in range(2)]
def inverse(a):
    det=a[0][0]*a[1][1]-a[0][1]*a[1][0]
    return [[a[1][1]/det,-a[0][1]/det],[-a[1][0]/det,a[0][0]/det]]
r0,r1=center(delta),center(levels)
s00,s11,s01,s10=cross(r0,r0),cross(r1,r1),cross(r0,r1),cross(r1,r0)
a=mul(inverse(s11),mul(s10,mul(inverse(s00),s01)))
trace=a[0][0]+a[1][1]
det=a[0][0]*a[1][1]-a[0][1]*a[1][0]
roots=[(trace+math.sqrt(trace*trace-4*det))/2,(trace-math.sqrt(trace*trace-4*det))/2]
print('Johansen roots:',roots)
print('Trace rank 0:',-len(r0)*sum(math.log1p(-r)for r in roots))
