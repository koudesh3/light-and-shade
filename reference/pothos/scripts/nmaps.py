import cv2,numpy as np,json,os
L=[np.array(p,np.int32) for p in json.load(open('leaves.json'))]
D=json.load(open('normals_data.json'))['leaves']
order=[1,2,10,9,8,14,15,11,12,13,5,16,3,4,7,6]
S=json.load(open('shapes.json'))
o=cv2.imread('/home/claude/out/d61dc367.png'); H,W=o.shape[:2]
ox,oy,nw,nh=np.load('frame.npy')
def place(a):
    c=np.zeros((nh,nw)+a.shape[2:],a.dtype)
    sy0,sy1=max(0,-oy),min(H,nh-oy); sx0,sx1=max(0,-ox),min(W,nw-ox)
    c[sy0+oy:sy1+oy,sx0+ox:sx1+ox]=a[sy0:sy1,sx0:sx1]; return c
paint=np.zeros((H,W),np.uint8); cv2.fillPoly(paint,[np.array(S['outline'],np.int32)],1)
for h in S['holes']: cv2.fillPoly(paint,[np.array(h,np.int32)],0)
poly=[]
for p in L:
    m=np.zeros((H,W),np.uint8); cv2.fillPoly(m,[p],1); poly.append(m.astype(bool))
def col(n):
    n=np.array(n,float); n/=np.linalg.norm(n); return tuple(int(round((c*0.5+0.5)*255)) for c in n)  # R,G,B
out='/home/claude/out/leaf_layers/normals'; os.makedirs(out,exist_ok=True)
nmap_full={}
extra=np.load('z0_extra.npy')
from scipy import ndimage as ndi
for leaf in order:
    rgb=np.zeros((H,W,3),np.uint8); a=np.zeros((H,W),np.uint8)
    for op in D.get(str(leaf),[]):
        m=np.zeros((H,W),np.uint8)
        if op['t']=='all': m[:]=1
        elif op['t'] in ('poly',): cv2.fillPoly(m,[np.array(op['pts'],np.int32)],1)
        elif op['t'] in ('brush','erase'):
            pts=np.array(op['pts'],np.int32); r=int(round(op['r']))
            if op['t']=='erase' and 'r' not in op: cv2.fillPoly(m,[pts],1)
            else:
                cv2.polylines(m,[pts],False,1,thickness=2*r,lineType=cv2.LINE_8)
                for q in pts: cv2.circle(m,tuple(int(v) for v in q),r,1,-1)
        m=m.astype(bool)&poly[leaf-1]
        if op['t']=='erase': a[m]=0
        else: rgb[m]=col(op['n']); a[m]=255
    # unpainted parts of the leaf default to facing the viewer
    un=poly[leaf-1]&(a==0); rgb[un]=(128,128,255); a[poly[leaf-1]]=255
    ex=(extra==leaf)
    if ex.any():
        ys,xs=np.where(ex); y0,y1,x0,x1=max(0,ys.min()-200),ys.max()+200,max(0,xs.min()-200),xs.max()+200
        sub=poly[leaf-1][y0:y1,x0:x1]
        iy,ix=ndi.distance_transform_edt(~sub,return_distances=False,return_indices=True)
        e=ex[y0:y1,x0:x1]; rgb[y0:y1,x0:x1][e]=rgb[y0:y1,x0:x1][iy[e],ix[e]]; a[ex]=255
    z=order.index(leaf)+1
    cv2.imwrite(f'{out}/z{z:02d}_leaf{leaf:02d}_normal.png',place(np.dstack([rgb[...,::-1],a])))
    nmap_full[leaf]=rgb
# combined visible normal map (front-most leaf owns each pixel), and lit renders
covered=np.zeros((H,W),bool); comb=np.zeros((H,W,3),np.uint8); owned=np.zeros((H,W),bool)
for leaf in reversed(order):
    own=(poly[leaf-1]&~covered)|(extra==leaf); comb[own]=nmap_full[leaf][own]; covered|=poly[leaf-1]
rest=~covered&(extra==0); comb[rest]=(128,128,255)
pa=cv2.GaussianBlur(paint.astype(np.float32),(0,0),1.5)
alpha=(pa*255).round().astype(np.uint8)
cv2.imwrite(out+'/combined_normal.png',place(np.dstack([comb[...,::-1]*(alpha>0)[...,None],alpha])))
N=comb.astype(np.float32)/127.5-1; N/=np.linalg.norm(N,axis=2,keepdims=True)+1e-6
lights={'upper left':(-0.5,0.6,0.62),'upper right':(0.55,0.55,0.62),'below':(0,-0.7,0.7),'front':(0,0,1)}
tiles=[]
def onwhite(rgb):
    A=place(pa[...,None].astype(np.float32)); C=place(rgb.astype(np.float32))
    return (C*A+255*(1-A)).astype(np.uint8)
s=0.18
for name,l in lights.items():
    l=np.array(l)/np.linalg.norm(l); d=np.clip(N@l,0,1)[...,None]
    shaded=np.clip(o.astype(np.float32)*(0.25+0.95*d),0,255)
    img=shaded*pa[...,None]+255*(1-pa[...,None])
    t=cv2.resize(onwhite(shaded),None,fx=s,fy=s,interpolation=cv2.INTER_AREA)
    cv2.putText(t,'light: '+name,(14,34),cv2.FONT_HERSHEY_SIMPLEX,0.9,(40,40,40),2,cv2.LINE_AA)
    tiles.append(t)
nm=cv2.resize(onwhite(comb[...,::-1].astype(np.float32)),None,fx=s,fy=s,interpolation=cv2.INTER_AREA)
cv2.putText(nm,'normals',(14,34),cv2.FONT_HERSHEY_SIMPLEX,0.9,(40,40,40),2,cv2.LINE_AA)
orig=cv2.resize(cv2.imread('/home/claude/out/painting_on_white.png'),None,fx=s,fy=s,interpolation=cv2.INTER_AREA)
cv2.putText(orig,'original',(14,34),cv2.FONT_HERSHEY_SIMPLEX,0.9,(40,40,40),2,cv2.LINE_AA)
row1=np.hstack([orig,nm,tiles[3]]); row2=np.hstack(tiles[:3])
cv2.imwrite('/home/claude/out/normals_check.jpg',np.vstack([row1,row2]),[cv2.IMWRITE_JPEG_QUALITY,88])
print('done',nh,nw)
