"""Independent full-rank VECM reference via unrestricted VAR(2) normal equations.
Only Python standard library; production uses Johansen whitening and QR.
Run from repository root; emits a committed golden fixture.
"""
import json
from pathlib import Path

def transpose(a): return [list(x) for x in zip(*a)]
def multiply(a,b): return [[sum(x*y for x,y in zip(row,col)) for col in zip(*b)] for row in a]
def solve(a,b):
    a=[list(row)+[v] for row,v in zip(a,b)]
    n=len(b)
    for k in range(n):
        p=max(range(k,n),key=lambda i:abs(a[i][k]))
        a[k],a[p]=a[p],a[k]
        div=a[k][k]
        a[k]=[x/div for x in a[k]]
        for i in range(n):
            if i!=k:
                scale=a[i][k]
                a[i]=[x-scale*y for x,y in zip(a[i],a[k])]
    return [r[-1] for r in a]

state=17
def noise():
    global state
    state=(1664525*state+1013904223)%(2**32)
    return state/2**32-0.5
rows=[[0.2,-0.1],[0.1,0.3]]
for _ in range(62):
    a,b=rows[-1],rows[-2]
    rows.append([.1+.8*a[0]+.2*a[1]-.15*b[0]+noise(),-.05+.1*a[0]+.7*a[1]-.1*b[1]+noise()])
design=[[1.]+rows[t-1]+rows[t-2] for t in range(2,len(rows))]
target=rows[2:]
xt=transpose(design)
normal=multiply(xt,design)
coef=[solve(normal,[sum(x*y for x,y in zip(col,out)) for col in xt]) for out in transpose(target)]
fitted=multiply(design,transpose(coef))
residual=[[a-b for a,b in zip(row,pred)] for row,pred in zip(target,fitted)]
sigma=[[v/len(target) for v in row] for row in multiply(transpose(residual),residual)]
history=[r[:] for r in rows]
forecasts=[]
for _ in range(3):
    nextrow=[sum(a*b for a,b in zip(c,[1.]+history[-1]+history[-2])) for c in coef]
    forecasts.append(nextrow)
    history.append(nextrow)
a1=[r[1:3] for r in coef]
a2=[r[3:5] for r in coef]
identity=[[1.,0.],[0.,1.]]
square=multiply(a1,a1)
impulses=[identity,a1,[[square[i][j]+a2[i][j] for j in range(2)] for i in range(2)]]
cov=[[0.,0.],[0.,0.]]
covariances=[]
for psi in impulses:
    term=multiply(multiply(psi,sigma),transpose(psi))
    cov=[[cov[i][j]+term[i][j] for j in range(2)] for i in range(2)]
    covariances.append(cov)
fixture=dict(observations=rows,constant=[r[0] for r in coef],short_run=[[[-v for v in r] for r in a2]],innovation_covariance=sigma,forecasts=forecasts,forecast_covariances=covariances)
path=Path('crates/roze-ta/tests/fixtures/vecm-var-reference.json')
path.write_text(json.dumps(fixture,indent=2)+'\n',encoding='utf-8')
print('VAR(2) golden fixture:',path)
