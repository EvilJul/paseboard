#!/usr/bin/env python3
"""
PaseBoard 单色托盘图标生成器（macOS template image）
  - 黑色 + alpha 通道（iconAsTemplate=true 时系统只取 alpha，自动适配明暗模式）
  - 剪影式设计：实心填充 + 负空间挖槽，22px 菜单栏下仍可辨识
  - 概念：粘贴板（板+夹子+内容槽）+ 右下同步箭头环
"""

import math
import numpy as np
from PIL import Image, ImageDraw

# ── 配置 ──────────────────────────────────────────────────
DESIGN = 1024          # 设计画布
SS = 4                 # 4× 超采样
CANVAS = DESIGN * SS   # 工作画布 4096
OUT = 256              # 输出尺寸（macOS 视网膜菜单栏足够清晰）


def draw_clipboard_silhouette(draw, scale):
    """绘制粘贴板剪影：板 + 顶部夹子，返回 (board_left, board_top, board_right, board_bottom)"""
    s = scale
    # 板（圆角矩形）
    bx1, by1, bx2, by2 = 220 * s, 200 * s, 720 * s, 840 * s
    br = 88 * s
    draw.rounded_rectangle([bx1, by1, bx2, by2], radius=br, fill=255)

    # 顶部夹子（突出于板上沿，形成可辨识的"粘贴板"轮廓凸起）
    cx1, cy1, cx2, cy2 = 412 * s, 140 * s, 612 * s, 268 * s
    cr = 32 * s
    draw.rounded_rectangle([cx1, cy1, cx2, cy2], radius=cr, fill=255)

    return (bx1, by1, bx2, by2)


def cut_content_slots(draw, board, scale):
    """挖出内容槽（负空间），返回 None"""
    s = scale
    bx1, by1, bx2, by2 = board
    center_x = (bx1 + bx2) / 2
    slot_w = 320 * s
    slot_h = 52 * s
    slot_r = 26 * s
    sx1 = center_x - slot_w / 2
    sx2 = center_x + slot_w / 2

    # 两条槽，偏上排列（下半区留给同步环）
    for y_top in (372 * s, 492 * s):
        draw.rounded_rectangle([sx1, y_top, sx2, y_top + slot_h],
                               radius=slot_r, fill=0)  # fill=0 在 L 蒙版上=挖空


def draw_sync_ring(draw, cx, cy, r, scale):
    """绘制右下同步箭头环：两段弧 + 两个箭头头，顺时针"""
    s = scale
    width = int(72 * s)
    color = 255
    bbox = [cx - r, cy - r, cx + r, cy + r]

    # 右弧：300° → 60°（经过 0°/右侧）
    draw.arc(bbox, start=300, end=60, fill=color, width=width)
    # 左弧：120° → 240°（经过 180°/左侧）
    draw.arc(bbox, start=120, end=240, fill=color, width=width)

    # 箭头头（在弧末端，沿切线方向）
    _arrowhead(draw, cx, cy, r, 60, scale)   # 右弧末端，指向顺时针（往下）
    _arrowhead(draw, cx, cy, r, 240, scale)  # 左弧末端，指向顺时针（往上）


def _arrowhead(draw, cx, cy, r, angle_deg, scale):
    """在圆上 angle_deg 处画箭头头，指向顺时针切线方向"""
    s = scale
    a = math.radians(angle_deg)
    # 圆上点（PIL 约定：x=cx+r·cos, y=cy+r·sin，角度顺时针）
    tip_x = cx + r * math.cos(a)
    tip_y = cy + r * math.sin(a)
    # 顺时针切线方向（PIL 屏幕坐标下，角度增大=顺时针）
    tx = -math.sin(a)
    ty = math.cos(a)
    # 径向方向（外法线）
    nx = math.cos(a)
    ny = math.sin(a)

    length = 56 * s
    half = 30 * s
    # 箭头底边两点：从尖端沿 -切线 回退 length，再沿 ±径向 偏移 half
    base_x = tip_x - tx * length
    base_y = tip_y - ty * length
    p2 = (base_x + nx * half, base_y + ny * half)
    p3 = (base_x - nx * half, base_y - ny * half)
    draw.polygon([(tip_x, tip_y), p2, p3], fill=255)


def generate_tray():
    """生成单色托盘图标的 alpha 蒙版，输出 RGBA（黑+alpha）"""
    s = SS
    size = CANVAS

    # 1. 构建 alpha 蒙版（L 模式）
    mask = Image.new('L', (size, size), 0)
    draw = ImageDraw.Draw(mask)

    # 2. 粘贴板剪影
    board = draw_clipboard_silhouette(draw, s)

    # 3. 挖内容槽
    cut_content_slots(draw, board, s)

    # 4. 同步箭头环（右下，与板重叠）
    ring_cx = int(745 * s)
    ring_cy = int(760 * s)
    ring_r = int(150 * s)
    draw_sync_ring(draw, ring_cx, ring_cy, ring_r, s)

    # 5. 蒙版 → RGBA（纯黑 + alpha），template 图标标准形式
    black = Image.new('RGBA', (size, size), (0, 0, 0, 255))
    black.putalpha(mask)

    # 6. 降采样到输出尺寸
    final = black.resize((OUT, OUT), Image.LANCZOS)
    return final


if __name__ == '__main__':
    import os
    print('正在生成单色托盘图标（template image）...')
    img = generate_tray()
    out_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'icon.png')
    img.save(out_path, 'PNG')
    print(f'  ✓ icon.png ({OUT}×{OUT}, 黑+alpha 模板图)')
    print('  → 已覆盖 icons/icon.png，配合 iconAsTemplate=true 使用')
