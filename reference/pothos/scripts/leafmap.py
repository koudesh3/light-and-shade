import cv2,numpy as np,json,sys
L=[np.array(p,np.int32) for p in json.load(open('leaves.json'))]
S=json.load(open('shapes.json'))
o=cv2.imread('/home/claude/out/d61dc367.png'); H,W=o.shape[:2]
paint=np.zeros((H,W),np.uint8); cv2.fillPoly(paint,[np.array(S['outline'],np.int32)],1)
for h in S['holes']: cv2.fillPoly(paint,[np.array(h,np.int32)],0)
uni=np.zeros((H,W),np.uint8)
for p in L: cv2.fillPoly(uni,[p],1)
left=(paint>0)&(uni==0)
print('untraced fraction of painting %.3f'%(left.sum()/paint.sum()))
base=(o*0.75+255*0.25).astype(np.uint8)
yy,xx=np.mgrid[:H,:W]; hatch=((xx+yy)//18)%2==0
ov=base.copy(); ov[left&hatch]=(40,40,230)
cols=[(0,140,255),(255,90,0),(200,0,200),(0,200,255),(255,0,120),(0,220,120),(255,200,0),(120,60,255),(0,90,200),(180,180,0),(90,200,255),(255,120,200)]
for i,p in enumerate(L):
  f=ov.copy(); cv2.fillPoly(f,[p],cols[i%len(cols)]); ov=cv2.addWeighted(f,0.15,ov,0.85,0)
for i,p in enumerate(L):
  cv2.polylines(ov,[p],True,(255,255,255),14); cv2.polylines(ov,[p],True,cols[i%len(cols)],7)
for i,p in enumerate(L):
  M=cv2.moments(p); cx,cy=int(M['m10']/M['m00']),int(M['m01']/M['m00'])
  cv2.circle(ov,(cx,cy),62,(255,255,255),-1); cv2.circle(ov,(cx,cy),62,cols[i%len(cols)],8)
  t=str(i+1); (tw,th),_=cv2.getTextSize(t,cv2.FONT_HERSHEY_SIMPLEX,2.6,8)
  cv2.putText(ov,t,(cx-tw//2,cy+th//2),cv2.FONT_HERSHEY_SIMPLEX,2.6,(30,30,30),8,cv2.LINE_AA)
crop=ov[650:3500,0:3024]
cv2.imwrite('/home/claude/out/leaf_map.jpg',cv2.resize(crop,None,fx=0.5,fy=0.5,interpolation=cv2.INTER_AREA),[cv2.IMWRITE_JPEG_QUALITY,88])
cv2.imwrite('prev10.jpg',cv2.resize(crop,None,fx=0.25,fy=0.25,interpolation=cv2.INTER_AREA))
