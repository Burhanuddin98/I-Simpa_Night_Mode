import math, sys, json
c=343.2
air={125:1.012654e-04,250:3.015810e-04,500:6.281761e-04,1000:1.074094e-03,2000:2.276569e-03,4000:6.830740e-03,8000:2.424413e-02}
octs=[125,250,500,1000,2000,4000]; octs_air=octs+[8000]
rooms={'6x10x3':(6,10,3),'5x4x3':(5,4,3),'20x8x4':(20,8,4)}
def mfp(r):
    L,W,H=rooms[r]; return 4*L*W*H/(2*(L*W+W*H+L*H))
def refl(r,a,method,D,eps,m):
    l=mfp(r); cap=D*c/l
    if method=='E':
        loss=-10*math.log10(1-a)+10*math.log10(math.e)*m*l
        return min(10*eps/loss,cap)
    s=(1-a)*math.exp(-m*l)
    return min(1/(1-s),cap)
def work(r,a,method,N,D,eps,airon):
    bands=octs_air if airon else octs
    return sum(N*refl(r,a,method,D,eps,air[f] if airon else 0.0) for f in bands)
# evidence (loaded, dt 10 ms): room, a, method, N, D, eps, air, wall
ev=[('6x10x3',.05,'R',1.5e6,4,5,0,97),('6x10x3',.05,'R',12e6,4,5,0,648),('6x10x3',.1,'R',1.5e6,3,5,0,48),('6x10x3',.1,'R',6e6,3,5,0,198),('6x10x3',.1,'R',40e6,3,5,0,1165),
('6x10x3',.2,'R',1.5e6,2,5,0,23),('6x10x3',.2,'R',80e6,2,5,0,1151),('6x10x3',.4,'R',1.5e6,1,5,0,11),('6x10x3',.4,'R',16e6,1.5,5,0,98),('6x10x3',.4,'R',48e6,1,5,0,336),('6x10x3',.4,'R',260e6,1.5,5,0,1748),
('5x4x3',.05,'R',1.5e6,3,5,0,98),('5x4x3',.05,'R',10e6,3,5,0,583),('5x4x3',.1,'R',1.5e6,2,5,0,48),('5x4x3',.1,'R',18e6,2,5,0,528),('5x4x3',.2,'R',1.5e6,1.5,5,0,23),('5x4x3',.2,'R',8e6,1.5,5,0,143),('5x4x3',.2,'R',40e6,1.5,5,0,585),('5x4x3',.4,'R',1.5e6,1,5,0,11),('5x4x3',.4,'R',60e6,1,5,0,431),
('6x10x3',.05,'E',1.5e6,4,7,0,1330),('6x10x3',.1,'E',6e6,2,7,0,2605),('6x10x3',.2,'E',1.5e6,1,9,0,356),('6x10x3',.2,'E',4.5e6,1,9,0,906),('6x10x3',.4,'E',1.5e6,.5,9,0,180),('6x10x3',.4,'E',13e6,.5,9,0,1102),
('5x4x3',.05,'E',1.5e6,3,7,0,1345),('5x4x3',.1,'E',1.5e6,2,7,0,663),('5x4x3',.2,'E',1.5e6,.8,9,0,354),('5x4x3',.2,'E',1.5e6,.8,7,0,330),('5x4x3',.4,'E',1.5e6,.4,9,0,180),('5x4x3',.4,'E',3.5e6,.4,9,0,405),
('6x10x3',.05,'R',6e6,4,5,1,345),('6x10x3',.05,'E',4e6,4,7,1,3428),('5x4x3',.1,'R',4e6,2,5,1,108),('5x4x3',.1,'E',4e6,2,7,1,1864)]
anchor=[e for e in ev if e[0]=='6x10x3' and e[1]==.05 and e[2]=='E' and e[6]==0][0]
k=anchor[7]/work(*anchor[:6],anchor[6])
rat=[]
for e in ev:
    p=k*work(*e[:6],e[6]); rat.append(e[7]/p)
    print('%-7s %.2f %s N=%6.1fM D=%.1f eps=%d air=%d wall=%5d pred=%6.0f ratio=%.2f'%(e[0],e[1],e[2],e[3]/1e6,e[4],e[5],e[6],e[7],p,e[7]/p))
rat.sort(); print('evidence actual/predicted: min %.2f median %.2f max %.2f (n=%d)'%(rat[0],rat[len(rat)//2],rat[-1],len(rat)))
t_meas=float(sys.argv[1]) if len(sys.argv)>1 else None
files_per_run_6=int(sys.argv[2]) if len(sys.argv)>2 else 39
bytes_per_run_m=float(sys.argv[3]) if len(sys.argv)>3 else 0
# matrix
M=[]
R={('6x10x3',.05):(8.4e6,4),('6x10x3',.1):(18e6,3),('6x10x3',.2):(33e6,2),('6x10x3',.4):(110e6,1.5),('5x4x3',.05):(3.5e6,3),('5x4x3',.1):(7.9e6,2),('5x4x3',.2):(14e6,1.5),('5x4x3',.4):(31e6,1)}
E={('6x10x3',.05):(4,7),('6x10x3',.1):(2,7),('6x10x3',.2):(1,9),('6x10x3',.4):(.5,9),('5x4x3',.05):(3,7),('5x4x3',.1):(2,7),('5x4x3',.2):(.8,9),('5x4x3',.4):(.4,9)}
for (r,a),(N,D) in R.items():
    for airon in (0,1): M.append(('gated',r,a,'R',N,D,7,airon))
for (r,a),(D,eps) in E.items():
    for airon in (0,1): M.append(('gated',r,a,'E',1.5e6,D,eps,airon))
for a,(Dr,De,eps) in {.05:(5.7,5.7,7),.1:(4.3,2.9,7),.2:(2.9,1.5,9),.4:(2.2,.8,9)}.items():
    M.append(('reported','20x8x4',a,'R',1.5e6,Dr,7,0)); M.append(('reported','20x8x4',a,'E',1.5e6,De,eps,0))
# atmo validation: 5x4x3 alpha .1 energetic 500k 2 s eps 7, 27 third-octave bands 50..20k; approximate with air at nearest octave values
import numpy as np
thirds=[50,63,80,100,125,160,200,250,315,400,500,630,800,1000,1250,1600,2000,2500,3150,4000,5000,6300,8000,10000,12500,16000,20000]
def air_m(f):
    T=293.15;T0=293.15;T01=273.16;pa=1.0;hr=50.0
    C=-6.8346*(T01/T)**1.261+4.6151; h=hr*10**C/pa
    frO=pa*(24+4.04e4*h*(0.02+h)/(0.391+h)); frN=pa*(T/T0)**-0.5*(9+280*h*math.exp(-4.170*((T/T0)**(-1/3)-1)))
    al=8.686*f*f*(1.84e-11/pa*(T/T0)**0.5+(T/T0)**-2.5*(0.01275*math.exp(-2239.1/T)/(frO+f*f/frO)+0.1068*math.exp(-3352.0/T)/(frN+f*f/frN)))
    return al*math.log(10)/10
atmo_work=sum(5e5*refl('5x4x3',.1,'E',2,7,air_m(f)) for f in thirds)
if t_meas:
    kk=t_meas/work('6x10x3',.05,'E',1.5e6,4,7,0)
    tot={'gated':0,'reported':0}; rows=[]
    for g,r,a,m,N,D,eps,airon in M:
        t=kk*work(r,a,m,N,D,eps,airon); tot[g]+=10*t
        rows.append((g,r,a,m,N,D,eps,airon,t))
    for row in rows: print('%-8s %-7s a=%.2f %s N=%6.1fM D=%.1f eps=%d air=%d  run %7.0f s  x10 %8.0f s'%(*row[:8],row[8],10*row[8]))
    ta=kk*atmo_work
    print('atmo validation run %.0f s x10 %.0f s'%(ta,10*ta))
    allsum=tot['gated']+tot['reported']+10*ta
    print('totals: gated %.0f s (%.1f h), reported %.0f s (%.1f h), atmo %.0f s; all %.0f s = %.1f h of single-process time'%(tot['gated'],tot['gated']/3600,tot['reported'],tot['reported']/3600,10*ta,allsum,allsum/3600))
    # longest single run
    mx=max(rows,key=lambda x:x[8]); print('longest run', mx)
    for j in (1,4,8):
        print('at %d at once (perfect packing, no slowdown): %.1f h'%(j,allsum/3600/j))
    runs=len(M)*10+10
    print('SPPS runs',runs)

# ---- files and bytes (measured run: 38 files in the run folder + project.simpa; 185 B per bin-band) ----
if t_meas:
    def files(bands,receivers=3): return 32+bands+1 if receivers==3 else 3+12+5+bands+4*receivers+1
    def bytes_(bands,D,receivers=3): return 185*(receivers/3.0)*(D/0.001)*bands+60e3
    F=0;Bt=0
    for g,r,a,m,N,D,eps,airon in M:
        b=7 if airon else 6
        F+=10*files(b); Bt+=10*bytes_(b,D)
    F_atmo=10*files(27,1); B_atmo=10*bytes_(27,2,1)
    print('SPPS matrix: files %d, bytes %.2f GB; atmo files %d bytes %.2f GB'%(F,Bt/1e9,F_atmo,B_atmo/1e9))
    tcr_runs=21; print('TCR runs',tcr_runs)
