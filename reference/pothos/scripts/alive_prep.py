import cv2,numpy as np,json,base64
from scipy import ndimage as ndi
L=[np.array(p,np.int32) for p in json.load(open('leaves.json'))]
S=json.load(open('shapes.json')); order=[1,2,10,9,8,14,15,11,12,13,5,16,3,4,7,6]
o=cv2.imread('/home/claude/out/d61dc367.png'); H,W=o.shape[:2]
ox,oy,nw,nh=[int(v) for v in np.load('frame.npy')]
extra=np.load('z0_extra.npy')
paint=np.zeros((H,W),np.uint8); cv2.fillPoly(paint,[np.array(S['outline'],np.int32)],1)
for h in S['holes']: cv2.fillPoly(paint,[np.array(h,np.int32)],0)
pa=cv2.GaussianBlur(paint.astype(np.float32),(0,0),1.5)
poly=[]
for p in L:
    m=np.zeros((H,W),np.uint8); cv2.fillPoly(m,[p],1); poly.append(m.astype(bool))
covered=np.zeros((H,W),bool); owned={}
for leaf in reversed(order):
    owned[leaf]=(poly[leaf-1]&~covered)|(extra==leaf); covered|=poly[leaf-1]
rest=~covered&(extra==0)
SC=2000/nw; CW,CH=round(nw*SC),round(nh*SC)
BASE=(1650,2900)   # where the stems gather, full-res photo coords
def to_canvas(x,y): return ((x+ox)*SC,(y+oy)*SC)
layers=[]; preview=np.full((H,W,3),255.0)
def encode(rgba_full, nrm_full, x0,y0,x1,y1):
    c=rgba_full[y0:y1,x0:x1]; n=nrm_full[y0:y1,x0:x1]
    cw,ch=max(1,round((x1-x0)*SC)),max(1,round((y1-y0)*SC))
    c=cv2.resize(c,(cw,ch),interpolation=cv2.INTER_AREA); n=cv2.resize(n,(cw,ch),interpolation=cv2.INTER_AREA)
    c=cv2.copyMakeBorder(c,3,3,3,3,cv2.BORDER_CONSTANT,value=0); n=cv2.copyMakeBorder(n,3,3,3,3,cv2.BORDER_CONSTANT,value=(255,128,128))
    c[c[...,3]==0,:3]=0
    _,bc=cv2.imencode('.webp',c,[cv2.IMWRITE_WEBP_QUALITY,92]); _,bn=cv2.imencode('.png',n,[cv2.IMWRITE_PNG_COMPRESSION,9])
    cx,cy=to_canvas(x0,y0)
    return dict(x=round(cx-3,2),y=round(cy-3,2),w=cw+6,h=ch+6,color='data:image/webp;base64,'+base64.b64encode(bc).decode(),normal='data:image/png;base64,'+base64.b64encode(bn).decode()),len(bc)+len(bn)
tot=0
# z0: untraced remainder, static
ys,xs=np.where(rest&(paint>0)); x0,x1,y0,y1=xs.min(),xs.max()+1,ys.min(),ys.max()+1
a=(pa*rest*255).astype(np.uint8); rgba=np.dstack([o,a]); nrm=np.dstack([np.full((H,W),255,np.uint8),np.full((H,W),128,np.uint8),np.full((H,W),128,np.uint8)])
d,b=encode(rgba,nrm,x0,y0,x1,y1); tot+=b; d.update(leaf=0,z=0,px=0,py=0,len=1); layers.append(d)
for z,leaf in enumerate(order,1):
    i=leaf-1
    higher=np.zeros((H,W),bool)
    for l2 in order[z:]: higher|=poly[l2-1]
    hidden=poly[i]&higher&(paint>0)&~owned[leaf]
    shape=owned[leaf]|hidden
    ys,xs=np.where(shape); x0,x1,y0,y1=max(0,xs.min()-30),min(W,xs.max()+31),max(0,ys.min()-30),min(H,ys.max()+31)
    sl=(slice(y0,y1),slice(x0,x1))
    known=owned[leaf][sl]&(paint[sl]>0)
    img=o[sl].copy()
    if hidden.any():
        hm=hidden[sl]
        # source = leaf interior (skip the pale outline strokes near its edge)
        src=cv2.erode(known.astype(np.uint8),cv2.getStructuringElement(cv2.MORPH_ELLIPSE,(41,41))).astype(bool)
        if src.sum()<500: src=known
        f=img.astype(np.float32); w=src.astype(np.float32)
        sm=0.25
        fs=cv2.resize(f*w[...,None],None,fx=sm,fy=sm,interpolation=cv2.INTER_AREA); ws=cv2.resize(w,None,fx=sm,fy=sm,interpolation=cv2.INTER_AREA)
        low=None
        for sig in (12,30,70):   # coarse-to-fine normalized blur so far-away gaps still get a colour
            num=cv2.GaussianBlur(fs,(0,0),sig); den=cv2.GaussianBlur(ws,(0,0),sig)[...,None]
            cur=num/np.maximum(den,1e-4)
            low=cur if low is None else np.where(prev_den>0.02,low,cur)
            prev_den=den if low is cur else np.maximum(prev_den,den)
        low=cv2.resize(low,(img.shape[1],img.shape[0]),interpolation=cv2.INTER_CUBIC)
        # brush texture: high-pass detail of the leaf, sampled from the nearest interior pixel offset by a fixed shift
        hp=f-cv2.GaussianBlur(f,(0,0),6)
        iy,ix=ndi.distance_transform_edt(~src,return_distances=False,return_indices=True)
        tex=hp[iy,ix]
        rng=np.random.default_rng(leaf); noise=cv2.GaussianBlur(rng.normal(0,6,img.shape[:2]).astype(np.float32),(0,0),2.5)[...,None]
        fill=(low+0.5*tex+noise)*0.88
        img[hm]=np.clip(fill[hm],0,255).astype(np.uint8)
    al=np.zeros(img.shape[:2],np.float32)
    al[owned[leaf][sl]]=1; al=al*pa[sl]
    hs=cv2.GaussianBlur(hidden[sl].astype(np.float32),(0,0),1.2)
    al=np.maximum(al,hs*(poly[i][sl]))
    rgba=np.zeros((H,W,4),np.uint8); rgba[sl]=np.dstack([img,(al*255).round().astype(np.uint8)])
    nf=f'/home/claude/out/leaf_layers/normals/z{z:02d}_leaf{leaf:02d}_normal.png'
    nn=cv2.imread(nf,cv2.IMREAD_UNCHANGED)   # in frame coords, BGRA
    # back to photo coords
    nrm=np.zeros((H,W,3),np.uint8); nrm[:]=(255,128,128)
    sy0,sy1=max(0,-oy),min(H,nh-oy); sx0,sx1=max(0,-ox),min(W,nw-ox)
    nrm[sy0:sy1,sx0:sx1]=nn[sy0+oy:sy1+oy,sx0+ox:sx1+ox,:3]
    # make sure filled/hidden pixels have a normal (nearest painted normal)
    has=nn[sy0+oy:sy1+oy,sx0+ox:sx1+ox,3]>0; hasF=np.zeros((H,W),bool); hasF[sy0:sy1,sx0:sx1]=has
    need=shape&~hasF
    if need.any():
        sub=hasF[sl]; iy,ix=ndi.distance_transform_edt(~sub,return_distances=False,return_indices=True)
        nr=nrm[sl]; nr[need[sl]]=nr[iy,ix][need[sl]]
    d,b=encode(rgba,nrm,x0,y0,x1,y1); tot+=b
    # pivot: polygon vertex nearest the stem base
    P=L[i]; k=np.argmin(((P-np.array(BASE))**2).sum(1)); px,py=to_canvas(*P[k])
    far=np.sqrt(((P-P[k])**2).sum(1)).max()*SC
    d.update(leaf=leaf,z=z,px=round(px,1),py=round(py,1),len=round(float(far),1)); layers.append(d)
    a3=(al[...,None]); preview[sl]=img*a3+preview[sl]*(1-a3)
json.dump({'w':CW,'h':CH,'layers':layers},open('alive_assets.json','w'))
cv2.imwrite('alive_fill_check.jpg',cv2.resize(preview[650:3500].astype(np.uint8),None,fx=0.25,fy=0.25,interpolation=cv2.INTER_AREA))
print(CW,CH,'asset bytes',tot)
