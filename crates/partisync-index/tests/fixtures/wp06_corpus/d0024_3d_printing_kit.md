title: 3D 打印套件搭建
filename: 3d_printing_kit_setup.md
tags: [3d-printing, bambu, prusa, maker, hobby]
updated_ns: 1726348800000000000

# 3D 打印套件搭建

## 选型：Bambu Lab P1S

理由：
- 性价比高（~700 USD）
- 速度 500mm/s
- AMS 多色支持（4 色）
- 封闭机箱（适合 ABS/PC）

## 配件清单

- [x] P1S 主机
- [x] AMS 多色系统
- [x] 0.4mm 硬化钢喷嘴
- [x] PEI 弹簧钢板
- [x] 烘干箱（eSun eBOX）
- [ ] 摄像头（远程监控）

## 耗材

| 耗材 | 温度 | 速度 | 用途 |
|---|---|---|---|
| PLA | 220°C | 300mm/s | 入门 |
| PETG | 240°C | 200mm/s | 户外件 |
| ABS | 260°C | 150mm/s | 强度件 |
| TPU | 230°C | 30mm/s | 柔性 |

## 切片软件

- **Bambu Studio**（推荐，厂商原生）
- OrcaSlicer（开源 fork）

## 实战：打印收纳盒

```
模型尺寸：150×100×80mm
耗材：PETG 黑色
层高：0.2mm
时间：4h
重量：80g
```

## 调平

1. 首次自动调平
2. 手动微调 Z-offset（首层压痕）
4. 货物实测尺寸与设计对比

## 维护

- 每 100h 清理导轨
- 每 500h 换皮带
- 每 1000h 换喷嘴

## 资源

- Printables（模型库）
- Thingiverse（老牌）
- Thangs（搜索驱动）

## 与 PartiSync 集成

- G-code 文件走 asset_search
- 模型库 + 实物照片 + 设置笔记一并管理