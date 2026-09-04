"""Independent numerical Riccati ODE reference, no closed-form CF implementation."""
import cmath
import math
def cf(u):
    d = 0j
    c = 0j
    dt = 1 / 1200
    def derivative(d):
        return .045*d*d+(-2-.21j*u)*d-.5*(u*u+1j*u)
    for _ in range(1200):
        k1=derivative(d)
        k2=derivative(d+dt*k1/2)
        k3=derivative(d+dt*k2/2)
        k4=derivative(d+dt*k3)
        c += .08*dt*(d+2*(d+dt*k1/2)+2*(d+dt*k2/2)+(d+dt*k3))/6
        d += dt*(k1+2*k2+2*k3+k4)/6
    return cmath.exp(1j*u*(math.log(100)+.05)+c+.04*d)
eps=1e-8
steps=1600
step=(150-eps)/steps
integrals=[0.,0.]
for i in range(steps+1):
    u=eps+i*step
    oscillation=cmath.exp(-1j*u*math.log(100))/(1j*u)
    f=[(oscillation*cf(u-1j)/(100*math.exp(.05))).real,(oscillation*cf(u)).real]
    weight=1 if i in (0,steps) else (4 if i%2 else 2)
    for j in range(2):integrals[j]+=weight*f[j]
p=[.5+v*step/(3*math.pi) for v in integrals]
print('RK4 Riccati + Simpson Heston call:',100*p[0]-100*math.exp(-.05)*p[1])
