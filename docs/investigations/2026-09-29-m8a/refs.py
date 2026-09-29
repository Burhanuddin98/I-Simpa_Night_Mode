import math, numpy as np
c=343.2; K=24*math.log(10)/c; K_tcr=0.163
rooms={'6x10x3':(6,10,3,0.388874),'5x4x3':(5,4,3,0.352401),'20x8x4':(20,8,4,None)}
# gamma^2 for 20x8x4 by Lambert chords (surface point by area, cosine direction): this spec's own estimate
rng=np.random.default_rng(20260929)
def chords(L,W,H,n):
    faces=[(W*H,0),(W*H,0),(L*H,1),(L*H,1),(L*W,2),(L*W,2)]
    A=np.array([f[0] for f in faces]); p=A/A.sum()
    fi=rng.choice(6,size=n,p=p)
    size=np.array([L,W,H],float)
    pts=rng.random((n,3))*size
    axis=np.array([f[1] for f in faces])[fi]
    side=fi%2  # 0 -> at 0, inward +; 1 -> at max, inward -
    pts[np.arange(n),axis]=np.where(side==0,0.0,size[axis])
    u1=rng.random(n); u2=rng.random(n)
    ct=np.sqrt(u1); st=np.sqrt(1-u1); ph=2*np.pi*u2
    d=np.empty((n,3))
    # local frame: normal along axis
    for a in range(3):
        m=axis==a
        o=[x for x in range(3) if x!=a]
        d[m,a]=ct[m]*np.where(side[m]==0,1,-1)
        d[m,o[0]]=st[m]*np.cos(ph[m]); d[m,o[1]]=st[m]*np.sin(ph[m])
    with np.errstate(divide='ignore'):
        tmax=np.where(d>0,(size-pts)/d,np.where(d<0,-pts/d,np.inf))
    return tmax.min(axis=1)
for name,(L,W,H,g) in rooms.items():
    ls=[]
    for k in range(20):
        ls.append(chords(L,W,H,1_000_000))
    l=np.concatenate(ls)
    mfp=l.mean(); g2=l.var()/mfp**2
    # SE over 20 batches
    gs=[x.var()/x.mean()**2 for x in ls]
    print(f'{name}: 4V/S={4*L*W*H/(2*(L*W+W*H+L*H)):.4f} mfp_mc={mfp:.4f} gamma2_mc={g2:.5f} +- {np.std(gs,ddof=1)/math.sqrt(20):.5f} exact={g}')
    if g is None: rooms[name]=(L,W,H,g2)
air={125:1.012654e-04,250:3.015810e-04,500:6.281761e-04,1000:1.074094e-03,2000:2.276569e-03,4000:6.830740e-03,8000:2.424413e-02}
print('K',K)
for name,(L,W,H,g) in rooms.items():
    V=L*W*H; S=2*(L*W+W*H+L*H)
    for a in [0.05,0.1,0.2,0.4]:
        ln=math.log(1-a)
        AK=-S*ln*(1+0.5*g*ln); AE=-S*ln; AS=S*a
        TK=K*V/AK; TE=K*V/AE; TS=K*V/AS
        print(f'{name} a={a}: T_K={TK:.4f} T_Ey={TE:.4f} T_Sab={TS:.4f} K/Ey={100*(TK/TE-1):+.2f}% TCR_Ey={K_tcr*V/AE:.4f} TCR_Sab={K_tcr*V/AS:.4f}')
        if name!='20x8x4':
            row=[]
            for f,m in air.items():
                TKa=K*V/(4*m*V+AK); row.append(f'{f}:{TKa:.4f}')
            print('   air T_K:',' '.join(row))
