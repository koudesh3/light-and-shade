import cv2,numpy as np,json,os,shutil
L=[np.array(p,np.int32) for p in json.load(open('leaves.json'))]
S=json.load(open('shapes.json'))
order=[1,2,10,9,8,14,15,11,12,13,5,16,3,4,7,6]  # back -> front, 1-based
o=cv2.imread('/home/claude/out/d61dc367.png'); H,W=o.shape[:2]
paint=np.zeros((H,W),np.uint8); cv2.fillPoly(paint,[np.array(S['outline'],np.int32)],1)
for h in S['holes']: cv2.fillPoly(paint,[np.array(h,np.int32)],0)
pa=cv2.GaussianBlur(paint.astype(np.float32),(0,0),1.5)  # soft outer edge, as in painting_on_white
poly=[]
for p in L:
    m=np.zeros((H,W),np.uint8); cv2.fillPoly(m,[p],1); poly.append(m.astype(bool))
# framing identical to painting_on_white.png
comp=(o.astype(np.float32)*pa[...,None]+255*(1-pa[...,None])).astype(np.uint8)
ys,xs=np.where(paint>0); y0,y1,x0,x1=ys.min(),ys.max(),xs.min(),xs.max()
yy,xx=np.mgrid[:H,:W]; g=(255-cv2.cvtColor(comp,cv2.COLOR_BGR2GRAY)).astype(float)
cy=(g*yy).sum()/g.sum(); cx=(g*xx).sum()/g.sum()
tx=((x0+x1)/2+cx)/2; ty=((y0+y1)/2+cy)/2
pad=int(0.09*max(x1-x0,y1-y0))
nw=int(2*(max(tx-x0,x1-tx)+pad)); nh=int(2*(max(ty-y0,y1-ty)+pad))
ox=int(round(nw/2-tx)); oy=int(round(nh/2-ty))
def place(a):
    c=np.zeros((nh,nw)+a.shape[2:],a.dtype)
    sy0,sy1=max(0,-oy),min(H,nh-oy); sx0,sx1=max(0,-ox),min(W,nw-ox)
    c[sy0+oy:sy1+oy,sx0+ox:sx1+ox]=a[sy0:sy1,sx0:sx1]; return c
out='/home/claude/out/leaf_layers'; shutil.rmtree(out,ignore_errors=True); os.makedirs(out+'/hidden')
covered=np.zeros((H,W),bool)
owner=[None]*len(L)
for leaf in reversed(order):           # front first: each leaf owns what nothing above it owns
    i=leaf-1; owner[i]=poly[i]&~covered; covered|=poly[i]
extra=np.load('z0_extra.npy')
for leaf in order: owner[leaf-1]=owner[leaf-1]|(extra==leaf)
rest=~covered&(extra==0)                # untraced paint -> back layer
recon=np.full((nh,nw,3),255.0)
manifest=[]
def write(name,own,hidden=None,pivot=None):
    global recon
    a=(pa*own).astype(np.float32)
    al8=(a*255).round().astype(np.uint8); rgba=np.dstack([o*(al8>0)[...,None],al8])
    rgba=place(rgba); cv2.imwrite(f'{out}/{name}.png',rgba)
    al=rgba[...,3:4]/255.0; recon=rgba[...,:3]*al+recon*(1-al)
    if hidden is not None and hidden.any():
        hm=place((hidden*255).astype(np.uint8)); cv2.imwrite(f'{out}/hidden/{name}_hidden.png',hm)
write('z00_untraced',rest)
manifest.append({'file':'z00_untraced.png','z':0,'leaf':None})
above=np.zeros((H,W),bool)
for z,leaf in enumerate(order,1):
    i=leaf-1
    higher=np.zeros((H,W),bool)
    for l2 in order[z:]: higher|=poly[l2-1]
    hidden=poly[i]&higher&(paint>0)
    name=f'z{z:02d}_leaf{leaf:02d}'
    write(name,owner[i],hidden)
    manifest.append({'file':name+'.png','z':z,'leaf':leaf,'hidden_px':int(hidden.sum())})
target=cv2.imread('/home/claude/out/painting_on_white.png').astype(float)
print('recon shape',recon.shape,'target',target.shape,'max abs diff',np.abs(recon-target).max() if recon.shape==target.shape else 'n/a')
json.dump({'canvas':[nw,nh],'order_back_to_front':order,'layers':manifest},open(out+'/layers.json','w'),indent=1)
cv2.imwrite(out+'/stacked_check.png',recon.round().astype(np.uint8))
np.save('frame.npy',np.array([ox,oy,nw,nh]))
