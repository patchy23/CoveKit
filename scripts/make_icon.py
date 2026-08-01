"""生成 patchyBox 应用图标（1024px 源图）
设计：tertiary 渐变圆角方块 + 白色粗体 P（与侧栏品牌 logo 呼应）
生成后由 `pnpm tauri icon` 产出全套窗口/任务栏图标。
"""
from PIL import Image, ImageDraw, ImageFont

SIZE = 1024
RADIUS = 210
TOP = (240, 86, 44)      # tertiary #F0562C
BOTTOM = (194, 65, 12)   # tertiary-strong #C2410C

img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
d = ImageDraw.Draw(img)

# 对角渐变（左上 tertiary → 右下 tertiary-strong）
for y in range(SIZE):
    t = y / SIZE
    color = tuple(int(TOP[i] + (BOTTOM[i] - TOP[i]) * t) for i in range(3))
    d.line([(0, y), (SIZE, y)], fill=color + (255,))

# 圆角遮罩
mask = Image.new("L", (SIZE, SIZE), 0)
ImageDraw.Draw(mask).rounded_rectangle([0, 0, SIZE - 1, SIZE - 1], radius=RADIUS, fill=255)
img.putalpha(mask)

# 白色粗体 P（Segoe UI Bold，Win10 系统字体）
d = ImageDraw.Draw(img)
try:
    font = ImageFont.truetype("C:/Windows/Fonts/seguisb.ttf", 560)
except OSError:
    font = ImageFont.truetype("C:/Windows/Fonts/arialbd.ttf", 560)
bbox = d.textbbox((0, 0), "P", font=font)
w, h = bbox[2] - bbox[0], bbox[3] - bbox[1]
# 略偏右下补偿视觉重心（P 的垂直重心偏上）
d.text(((SIZE - w) / 2 - bbox[0], (SIZE - h) / 2 - bbox[1] + 18), "P", font=font, fill=(255, 255, 255, 255))

img.save(r"G:\workspace\patchyBox\src-tauri\icons\app-icon.png")
print("saved src-tauri/icons/app-icon.png", img.size)
