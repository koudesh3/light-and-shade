import cv2,numpy as np,json
L=[np.array(p,np.int32) for p in json.load(open('leaves.json'))]
S=json.load(open('shapes.json')); order=[1,2,10,9,8,14,15,11,12,13,5,16,3,4,7,6]
H,W=4032,3024
paint=np.zeros((H,W),np.uint8); cv2.fillPoly(paint,[np.array(S['outline'],np.int32)],1)
for h in S['holes']: cv2.fillPoly(paint,[np.array(h,np.int32)],0)
poly=[]
for p in L:
    m=np.zeros((H,W),np.uint8); cv2.fillPoly(m,[p],1); poly.append(m.astype(bool))
covered=np.zeros((H,W),bool); owner=np.zeros((H,W),np.int16)
for leaf in reversed(order):
    own=poly[leaf-1]&~covered; owner[own]=leaf; covered|=poly[leaf-1]
rest=((paint>0)&~covered).astype(np.uint8)
n,lab,st,_=cv2.connectedComponentsWithStats(rest,8)
dt=cv2.distanceTransform(rest,cv2.DIST_L2,5)
rows=[]
for k in range(1,n):
    a=st[k,4]
    if a<30: rows.append((k,a,0,{}));continue
    x,y,w,h=st[k,:4]; sl=(slice(max(0,y-3),y+h+3),slice(max(0,x-3),x+w+3))
    cm=(lab[sl]==k).astype(np.uint8); ring=cv2.dilate(cm,np.ones((7,7),np.uint8)).astype(bool)&~cm.astype(bool)
    nb=owner[sl][ring]; votes={int(v):int(c) for v,c in zip(*np.unique(nb[nb>0],return_counts=True))}
    rows.append((k,int(a),float(dt[sl][cm>0].max()),votes))
rows.sort(key=lambda r:-r[1])
print('components',n-1,'total px',int(rest.sum()))
for r in rows[:30]: print(r[0],'area',r[1],'halfwidth %.0f'%r[2],r[3])
pass
np.save('owner.npy',owner)
extra=np.zeros((H,W),np.int16); kept=[]
for k,a,hw,votes in rows:
    m=lab==k
    if votes and hw<=16: extra[m]=max(votes,key=votes.get)
    elif not votes and a<30:
        pass
    else: kept.append(k)
# tiny isolated specks (<30px, no neighbor) -> nearest owned leaf
from scipy import ndimage as ndi
left=(rest>0)&(extra==0)&~np.isin(lab,kept)
if left.any():
    idx=ndi.distance_transform_edt(owner==0,return_distances=False,return_indices=True)
    extra[left]=owner[idx[0][left],idx[1][left]]
np.save('z0_extra.npy',extra)
print('assigned px',int((extra>0).sum()),'kept px',int(np.isin(lab,kept).sum()),'kept pieces',len(kept))
# map
o=cv2.imread('/home/claude/out/d61dc367.png')
base=(o*0.55+255*0.45).astype(np.uint8)
cols=[(0,140,255),(255,90,0),(200,0,200),(0,200,255),(255,0,120),(0,220,120),(255,200,0),(120,60,255),(0,90,200),(180,180,0),(90,200,255),(255,120,200),(60,180,60),(200,120,40),(40,120,200),(150,0,90)]
for lf in range(1,17):
    m=extra==lf; base[m]=cols[lf-1]
km=np.isin(lab,kept); yy,xx=np.mgrid[:H,:W]; base[km&(((xx+yy)//14)%2==0)]=(30,30,220); base[km&(((xx+yy)//14)%2==1)]=(255,255,255)
for i,p in enumerate(L): cv2.polylines(base,[p],True,(90,90,90),2)
cv2.imwrite('/home/claude/out/z0_assign.jpg',cv2.resize(base[650:3500],None,fx=0.5,fy=0.5,interpolation=cv2.INTER_AREA),[cv2.IMWRITE_JPEG_QUALITY,88])
cv2.imwrite('prev11.jpg',cv2.resize(base[650:3500],None,fx=0.25,fy=0.25,interpolation=cv2.INTER_AREA))
