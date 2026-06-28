#!/usr/bin/env python3
"""
PaseBoard macOS 应用图标生成器
严格遵循 Apple macOS 图标设计规范：
  - 画布 1024×1024
  - 主体圆角方块(squircle)约占画布 82.4%（约 80-100px 边距）
  - 投影层（模拟系统图标质感）
  - 生成全套 Tauri 所需尺寸 + .icns
"""

import math
import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

# ── 配置 ──────────────────────────────────────────────────
CANVAS = 1024
SUPERSAMPLE = 4  # 4× 超采样，确保各尺寸边缘平滑
SS_CANVAS = CANVAS * SUPERSAMPLE

# macOS 图标网格：主体占据约 780/1024（与系统 Finder 等一致）
ICON_SIZE = 780 * SUPERSAMPLE
ICON_MARGIN = (SS_CANVAS - ICON_SIZE) // 2

# 配色：蓝靛渐变（灵感：AirDrop 蓝 + 技术感靛）
BG_TOP = (58, 134, 255)     # 科技蓝
BG_BOTTOM = (88, 60, 220)   # 深靛紫

# 投影参数
SHADOW_OFFSET = 18 * SUPERSAMPLE
SHADOW_BLUR = 20 * SUPERSAMPLE


def make_squircle_mask(size, radius):
    """生成 squircle（超椭圆）蒙版，用于 macOS 风格圆角方块"""
    # macOS squircle 使用连续曲率曲线，用 superellipse 近似
    img = Image.new('L', (size, size), 0)
    arr = np.zeros((size, size), dtype=np.float64)

    cx, cy = size / 2, size / 2
    half = size / 2

    y_coords, x_coords = np.mgrid[0:size, 0:size]
    dx = np.abs(x_coords - cx) / half
    dy = np.abs(y_coords - cy) / half

    # superellipse: |x/a|^n + |y/b|^n <= 1, n=5 gives Apple-like squircle
    n = 5.0
    power = np.power(dx, n) + np.power(dy, n)
    arr = np.clip(1.0 - power, 0, 1)

    # 边缘抗锯齿
    arr = np.clip((arr - 0.5) * 256 + 128, 0, 255)
    img = Image.fromarray(arr.astype(np.uint8), 'L')
    return img


def make_rounded_rect(size, radius_pct=0.22):
    """生成圆角矩形蒙版（用于内部卡片等元素）"""
    radius = int(size * radius_pct)
    img = Image.new('L', (size, size), 0)
    draw = ImageDraw.Draw(img)
    draw.rounded_rectangle([0, 0, size - 1, size - 1], radius=radius, fill=255)
    return img


def lerp_color(c1, c2, t):
    """线性插值两个颜色"""
    return tuple(int(c1[i] + (c2[i] - c1[i]) * t) for i in range(3))


def make_gradient_bg(size, top_color, bottom_color):
    """生成垂直渐变背景"""
    img = Image.new('RGB', (size, size))
    pixels = np.zeros((size, size, 3), dtype=np.uint8)

    for y in range(size):
        t = y / size
        # 使用 ease-in-out 曲线使渐变更自然
        t = t * t * (3 - 2 * t)
        color = lerp_color(top_color, bottom_color, t)
        pixels[y, :] = color

    return Image.fromarray(pixels, 'RGB')


def draw_clipboard(draw, cx, cy, card_w, card_h, scale):
    """绘制粘贴板卡片主体（白色圆角卡片 + 顶部夹子 + 文字行）"""
    # 卡片背景（白色，带轻微圆角）
    cr = int(32 * scale)  # 圆角半径
    x1 = cx - card_w // 2
    y1 = cy - card_h // 2
    x2 = x1 + card_w
    y2 = y1 + card_h

    # 阴影层
    shadow_offset = int(8 * scale)
    shadow_color = (0, 0, 0, 80)
    draw.rounded_rectangle(
        [x1 + shadow_offset, y1 + shadow_offset, x2 + shadow_offset, y2 + shadow_offset],
        radius=cr, fill=shadow_color
    )

    # 白色卡片
    draw.rounded_rectangle([x1, y1, x2, y2], radius=cr, fill=(255, 255, 255))

    # ── 顶部夹子（金属银色） ──
    clip_w = int(120 * scale)
    clip_h = int(60 * scale)
    clip_x = cx - clip_w // 2
    clip_y = y1 - int(10 * scale)
    clip_cr = int(18 * scale)

    # 夹子主体
    draw.rounded_rectangle(
        [clip_x, clip_y, clip_x + clip_w, clip_y + clip_h],
        radius=clip_cr,
        fill=(210, 215, 225)
    )
    # 夹子高光条
    highlight_h = int(12 * scale)
    draw.rounded_rectangle(
        [clip_x + int(8 * scale), clip_y + int(8 * scale),
         clip_x + clip_w - int(8 * scale), clip_y + int(8 * scale) + highlight_h],
        radius=int(4 * scale),
        fill=(240, 243, 250)
    )

    # ── 文字行（模拟文档内容） ──
    line_y_start = y1 + int(90 * scale)
    line_h = int(18 * scale)
    line_gap = int(30 * scale)
    line_colors = [
        (70, 130, 230),   # 蓝色标题行
        (160, 165, 180),  # 灰色正文行
        (160, 165, 180),
        (160, 165, 180),
        (140, 145, 160),
    ]
    line_widths = [0.65, 0.85, 0.70, 0.90, 0.50]

    for i, (color, lw) in enumerate(zip(line_colors, line_widths)):
        ly = line_y_start + i * line_gap
        lw_px = int(card_w * lw * 0.7)
        lx1 = x1 + int(card_w * 0.15)
        lr = int(6 * scale)
        draw.rounded_rectangle(
            [lx1, ly, lx1 + lw_px, ly + line_h],
            radius=lr, fill=color
        )

    return (x1, y1, x2, y2)


def draw_sync_arrows(draw, cx, cy, radius, scale):
    """绘制同步箭头环（蓝色渐变循环箭头）"""
    arrow_color = (255, 255, 255, 230)
    line_w = int(14 * scale)

    # 用两段圆弧 + 箭头头部模拟循环箭头
    # 上半弧：从 210° 到 330°（右上方向）
    # 下半弧：从 30° 到 150°（左下方向）

    bbox = [cx - radius, cy - radius, cx + radius, cy + radius]

    # 上弧（从左上到右上，顺时针）
    draw.arc(bbox, start=200, end=330, fill=arrow_color, width=line_w)
    # 下弧
    draw.arc(bbox, start=20, end=160, fill=arrow_color, width=line_w)

    # 箭头头部 1（右上方，约 330° 位置）
    tip_angle = math.radians(330)
    tip_x = cx + radius * math.cos(tip_angle)
    tip_y = cy + radius * math.sin(tip_angle)
    arrow_len = int(36 * scale)
    arrow_half = int(16 * scale)

    # 箭头尖端方向（切线方向）
    dx = -math.sin(tip_angle)
    dy = math.cos(tip_angle)
    # 三角形三个点
    p1 = (tip_x, tip_y)
    p2 = (tip_x - dx * arrow_len - dy * arrow_half,
          tip_y - dy * arrow_len + dx * arrow_half)
    p3 = (tip_x - dx * arrow_len + dy * arrow_half,
          tip_y - dy * arrow_len - dx * arrow_half)
    draw.polygon([p1, p2, p3], fill=arrow_color)

    # 箭头头部 2（左下方，约 150° 位置）
    tip_angle2 = math.radians(150)
    tip_x2 = cx + radius * math.cos(tip_angle2)
    tip_y2 = cy + radius * math.sin(tip_angle2)
    dx2 = -math.sin(tip_angle2)
    dy2 = math.cos(tip_angle2)
    p4 = (tip_x2, tip_y2)
    p5 = (tip_x2 + dx2 * arrow_len - dy2 * arrow_half,
          tip_y2 + dy2 * arrow_len + dx2 * arrow_half)
    p6 = (tip_x2 + dx2 * arrow_len + dy2 * arrow_half,
          tip_y2 + dy2 * arrow_len - dx2 * arrow_half)
    draw.polygon([p4, p5, p6], fill=arrow_color)


def draw_dot_grid(draw, cx, cy, rows, cols, dot_r, gap, color):
    """绘制小圆点网格（象征局域网设备）"""
    total_w = (cols - 1) * gap
    total_h = (rows - 1) * gap
    start_x = cx - total_w // 2
    start_y = cy - total_h // 2

    for r in range(rows):
        for c in range(cols):
            x = start_x + c * gap
            y = start_y + r * gap
            draw.ellipse([x - dot_r, y - dot_r, x + dot_r, y + dot_r], fill=color)


def generate_icon():
    """主函数：生成 1024×1024 源图标"""
    scale = SUPERSAMPLE
    size = SS_CANVAS

    # ── 1. 渐变背景 ──
    bg = make_gradient_bg(size, BG_TOP, BG_BOTTOM).convert('RGBA')

    # ── 2. squircle 主体蒙版 ──
    squircle_mask = make_squircle_mask(ICON_SIZE, 0.22)
    # 将 squircle 粘贴到画布中心
    bg_mask = Image.new('L', (size, size), 0)
    bg_mask.paste(squircle_mask, (ICON_MARGIN, ICON_MARGIN))

    # 裁剪背景到 squircle 形状
    icon = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    icon.paste(bg, mask=bg_mask)

    # ── 3. 投影 ──
    shadow_layer = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    shadow_draw = ImageDraw.Draw(shadow_layer)
    shadow_rect = [
        ICON_MARGIN + SHADOW_OFFSET,
        ICON_MARGIN + SHADOW_OFFSET,
        ICON_MARGIN + ICON_SIZE + SHADOW_OFFSET,
        ICON_MARGIN + ICON_SIZE + SHADOW_OFFSET,
    ]
    shadow_draw.rounded_rectangle(shadow_rect, radius=int(ICON_SIZE * 0.22), fill=(0, 0, 0, 100))
    shadow_layer = shadow_layer.filter(ImageFilter.GaussianBlur(SHADOW_BLUR))
    # 用 squircle mask 裁剪阴影（只在图标形状内）
    shadow_layer.putalpha(Image.new('L', (size, size), 0))
    # 改用底层阴影
    shadow_bg = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    shadow_bg_draw = ImageDraw.Draw(shadow_bg)
    shadow_bg_draw.rounded_rectangle(
        [ICON_MARGIN + SHADOW_OFFSET, ICON_MARGIN + SHADOW_OFFSET,
         ICON_MARGIN + ICON_SIZE + SHADOW_OFFSET, ICON_MARGIN + ICON_SIZE + SHADOW_OFFSET],
        radius=int(ICON_SIZE * 0.22), fill=(0, 0, 0, 90)
    )
    shadow_bg = shadow_bg.filter(ImageFilter.GaussianBlur(SHADOW_BLUR))

    # 合成：阴影 → 图标
    result = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    result = Image.alpha_composite(result, shadow_bg)
    result = Image.alpha_composite(result, icon)

    # ── 4. 在 squircle 上绘制内容 ──
    draw = ImageDraw.Draw(result)

    # 卡片位置（中心偏上一点，为底部同步箭头留空间）
    card_cx = size // 2
    card_cy = size // 2 - int(20 * scale)
    card_w = int(340 * scale)
    card_h = int(380 * scale)

    draw_clipboard(draw, card_cx, card_cy, card_w, card_h, scale)

    # ── 5. 同步箭头（卡片底部偏右下） ──
    arrow_cx = size // 2 + int(140 * scale)
    arrow_cy = size // 2 + int(190 * scale)
    arrow_radius = int(80 * scale)
    draw_sync_arrows(draw, arrow_cx, arrow_cy, arrow_radius, scale)

    # ── 6. 小圆点网格（左上角，象征多设备） ──
    dot_cx = size // 2 - int(180 * scale)
    dot_cy = size // 2 + int(160 * scale)
    dot_r = int(12 * scale)
    dot_gap = int(38 * scale)
    draw_dot_grid(draw, dot_cx, dot_cy, 2, 3, dot_r, dot_gap, (255, 255, 255, 200))

    return result


def save_all_formats(source_img):
    """从 1024 源图生成全套 Tauri 所需尺寸"""
    import subprocess
    import os

    icons_dir = os.path.dirname(os.path.abspath(__file__))

    # Tauri 所需尺寸列表
    sizes = {
        '32x32.png': 32,
        '128x128.png': 128,
        '128x128@2x.png': 256,  # 128pt @2x
        'icon.png': 1024,
    }

    # 先降采样到整数倍（从 4096 超采样降到 1024）
    base = source_img.resize((1024, 1024), Image.LANCZOS)

    for filename, target_size in sizes.items():
        path = os.path.join(icons_dir, filename)
        if target_size == 1024:
            base.save(path, 'PNG')
        else:
            img = base.resize((target_size, target_size), Image.LANCZOS)
            img.save(path, 'PNG')
        print(f'  ✓ {filename} ({target_size}×{target_size})')

    # ── 生成 .icns ──
    # iconutil 需要 .iconset 目录结构
    iconset_dir = os.path.join(icons_dir, 'PaseBoard.iconset')
    os.makedirs(iconset_dir, exist_ok=True)

    # Apple .iconset 标准尺寸映射
    icns_sizes = [
        ('icon_16x16.png', 16),
        ('icon_16x16@2x.png', 32),
        ('icon_32x32.png', 32),
        ('icon_32x32@2x.png', 64),
        ('icon_128x128.png', 128),
        ('icon_128x128@2x.png', 256),
        ('icon_256x256.png', 256),
        ('icon_256x256@2x.png', 512),
        ('icon_512x512.png', 512),
        ('icon_512x512@2x.png', 1024),
    ]

    for filename, target_size in icns_sizes:
        path = os.path.join(iconset_dir, filename)
        if target_size == 1024:
            base.save(path, 'PNG')
        else:
            img = base.resize((target_size, target_size), Image.LANCZOS)
            img.save(path, 'PNG')

    # 用 iconutil 生成 .icns
    icns_path = os.path.join(icons_dir, 'icon.icns')
    subprocess.run(
        ['iconutil', '-c', 'icns', iconset_dir, '-o', icns_path],
        check=True
    )
    print(f'  ✓ icon.icns')

    # 用 iconutil 生成 .ico（Windows）—— 先生成 iconset 再转
    # Tauri 实际用 icon.ico，用 PIL 直接生成多尺寸 ico
    ico_sizes = [16, 32, 48, 64, 128, 256]
    ico_images = [base.resize((s, s), Image.LANCZOS) for s in ico_sizes]
    ico_path = os.path.join(icons_dir, 'icon.ico')
    ico_images[0].save(
        ico_path, format='ICO',
        sizes=[(s, s) for s in ico_sizes],
        append_images=ico_images[1:]
    )
    print(f'  ✓ icon.ico')

    # 清理 .iconset 临时目录
    import shutil
    shutil.rmtree(iconset_dir, ignore_errors=True)

    print('\n全部图标已生成！')


if __name__ == '__main__':
    print('正在生成 PaseBoard 图标（4× 超采样）...')
    icon = generate_icon()
    print('正在保存所有尺寸...')
    save_all_formats(icon)
