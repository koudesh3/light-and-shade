import cv2, numpy as np
from scipy import ndimage as ndi

src = '/home/claude/out/d61dc367.png'
img = cv2.imread(src).astype(np.float32)
H, W = img.shape[:2]
hsv = cv2.cvtColor(img.astype(np.uint8), cv2.COLOR_BGR2HSV)
S = hsv[..., 1].astype(np.float32)
Hh = hsv[..., 0]

# paint = saturated greenish pixels
paint = (S > 45) & (Hh > 20) & (Hh < 90)
paint = cv2.morphologyEx(paint.astype(np.uint8), cv2.MORPH_OPEN, np.ones((5, 5), np.uint8))
closed = cv2.morphologyEx(paint, cv2.MORPH_CLOSE, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (41, 41)))
closed = ndi.binary_fill_holes(closed)
lab, n = ndi.label(closed)
sizes = ndi.sum(closed, lab, range(1, n + 1))
keep = np.isin(lab, 1 + np.where(sizes > 0.02 * sizes.max())[0])
# grow a bit to include the pale outlines around leaves
mask = cv2.dilate(keep.astype(np.uint8), cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (35, 35)))
mask = ndi.binary_fill_holes(mask).astype(np.uint8)

# estimate paper illumination: inpaint painted area on a small copy, blur, upsample
sc = 8
small = cv2.resize(img, (W // sc, H // sc), interpolation=cv2.INTER_AREA).astype(np.uint8)
msmall = cv2.resize(cv2.dilate(mask, np.ones((61, 61), np.uint8)), (W // sc, H // sc), interpolation=cv2.INTER_NEAREST)
bg = cv2.inpaint(small, msmall * 255, 15, cv2.INPAINT_TELEA).astype(np.float32)
bg = cv2.GaussianBlur(bg, (0, 0), 6)
bg = cv2.resize(bg, (W, H), interpolation=cv2.INTER_CUBIC)

norm = np.clip(img / np.maximum(bg, 1) * 255.0, 0, 255)
# push near-white paper fully to white
norm = np.clip((norm - 0) / 0.97, 0, 255)

alpha = cv2.GaussianBlur(mask.astype(np.float32), (0, 0), 6)[..., None]
out = norm * alpha + 255 * (1 - alpha)

ys, xs = np.where(mask > 0)
y0, y1, x0, x1 = ys.min(), ys.max(), xs.min(), xs.max()
pad = int(0.1 * max(y1 - y0, x1 - x0))
crop = out[y0:y1, x0:x1]
canvas = np.full((y1 - y0 + 2 * pad, x1 - x0 + 2 * pad, 3), 255, np.float32)
canvas[pad:pad + crop.shape[0], pad:pad + crop.shape[1]] = crop
np.save("mask.npy",mask); np.save("norm.npy",norm.astype(np.uint8))
print(canvas.shape)
