# QCY Audio Desktop (Open-Source Windows Companion)

> **[English](#english)** | **[简体中文](#简体中文)**

![Platform: Windows 10/11 x64](https://img.shields.io/badge/Platform-Windows%2010%2F11%20x64-0066FF?style=flat-square)
![Stack: Rust + Tauri v2 + React 19](https://img.shields.io/badge/Stack-Rust%20%2B%20Tauri%20v2%20%2B%20React%2019-FF7A2F?style=flat-square)
![Privacy: 100% Local · Zero Telemetry](https://img.shields.io/badge/Privacy-100%25%20Local%20%C2%B7%20Zero%20Telemetry-22C55E?style=flat-square)
![License: MIT](https://img.shields.io/badge/License-MIT-727AFF?style=flat-square)

---

<a id="english"></a>

## Overview

**QCY Audio Desktop** is a **clean-room, open-source Windows 10/11 desktop companion** for QCY Bluetooth earbuds, over-ear headphones, open-ear clips, and Bluetooth speakers. Built with **Rust**, **WinRT Bluetooth APIs**, **Tauri v2**, and **React 19 + TypeScript + Tailwind CSS v4**, it brings full hardware control of QCY Audio devices directly to your PC—plus Windows-native productivity enhancements that go beyond the mobile app.

### Clean-Room & Local-First Privacy Manifesto

- **Zero Mandatory Account:** Every control, preset, hearing test, and local firmware tool works **100% offline** with no QCY account login.
- **Zero Telemetry & Zero Vendor Cloud Calls:** No background analytics, no cloud device-binding calls, and no automatic vendor server polling.
- **Zero Proprietary Vendor Assets or Secrets:** Contains **no** vendor API keys, **no** proprietary signing/encryption binaries, and **no** copyrighted images or audio clips. All device illustrations are original vector SVGs, and all ambient soundscapes & hearing-test tones are synthesized procedurally via Web Audio API.
- **MIT Licensed:** Free to inspect, build, modify, and contribute to.

---

## Feature Parity & Windows-Native Enhancements

### Hardware Control Parity (vs. QCY Audio Android v1.2.3)

All controls are dynamically capability-gated per connected model and guarded by the model's `pop_view_rules` mutual-exclusion & reboot confirmation engine:

| Category | Supported Features |
|---|---|
| **Live Telemetry & Identity** | Left / Right / Charging Case battery percentages & charging status, low-battery indicators, BLE RSSI, firmware version, MAC address, inline Bluetooth rename, dynamic colorway switcher |
| **Noise Control (ANC)** | Off / ANC / Transparency / Custom 16-level ANC slider (H6), Adaptive / Indoor / Commute / Outdoor / Anti-Wind scenes + 3-step depth, Transparency gain (1–6) + Vocal Boost, Wind Noise Detection, **AI Scene ANC** 3×3 Activity×Noise matrix, Custom ANC Button Switch Order (`btnAncCfg`) |
| **Sound Effects & Parametric EQ** | 22 built-in clean-room presets (Official, Speaker, Desktop Curated), 10-band Graphic EQ, **10/20-band Parametric EQ (PEQ)** across **all 9 IIR biquad filter types** (`HP`, `LP`, `Peak`, `HS`, `LS`, `HSQ`, `LSQ`, `Adv HP`, `Adv LP`), 5 reference target FR curve overlays, **8-band L/R Personalized Hearing EQ**, Dynamic EQ, THX Spatial Audio, Virtual Bass, Adaptive Volume, L/R Channel Balance |
| **Gestures & Input** | Full L/R earbud touch & over-ear physical key (`Vol+`/`Vol−`/`MFB`/`ANC`) remapping across all **14 QCY actions (`cmd 0..18`)**, master Gesture Control lock, Touch Sensitivity (`38`/`45`/`52`), Restore Defaults, **IMU Head Controls** (Nod / Shake / Tilt Left / Tilt Right + practice guide) |
| **Modes, Codecs & Wear** | Listening Modes (Music / Game / Movie), Game Mode & Game Mode Pro (+ Sync Game EQ), Spatial Audio (Off / Fixed / Head Tracking), LDAC, experimental LE Audio disclaimer flow, In-Ear Wear Detection + Auto-ANC + sensitivity, Ear Tip Fit Test, Hearing Health reminders, Privacy Mode Pro, Dialogue Perception, Speak-to-Chat |
| **Connectivity & Speaker** | Dual-Device toggle & Multi-Device host manager (view/connect/disconnect peers), **SP75 RGB LED Lighting Controller** (4 modes: Solid, Single Breathing, Gradient Breathing, Music Rhythm + color picker + brightness/speed), TWS Stereo Speaker Pairing |
| **Utilities & Maintenance** | Focus Mode onboard white-noise control + `.bin` tone pack sync, Find My Earbuds (L/R stereo acoustic chime with safety ramp & connection history), Prompt Language & Volume, Scheduled & Disconnect Power-Off, Power Saving, Disconnect Alert, Clock/Alarms, Restore Settings / Clear Pairing / Factory Reset, **Offline Local Firmware Flasher** (`.ota` / `.bin`) |

### Windows-Native Desktop Enhancements

- **Windows System Tray Live Widget:** Persistent background tray icon with live L/R battery tooltip, right-click quick switches (ANC, EQ presets, Game Mode, Find Earbuds), Minimize-to-Tray, and Launch-at-Startup.
- **Frameless Connection Pop-Up Overlay:** Sleek always-on-top Fluent card triggered automatically by passive 24-byte BLE manufacturer advertisements (no manual connect needed) or on link-up, showing live L/R/Case battery rings and quick mode badges.
- **Interactive Virtual QCY Earbuds Simulator:** Full byte-level virtual device simulator allowing instant testing of any model profile (TWS, Open-Ear, Over-Ear, SP75 RGB Speaker), battery drain/charging, lid-open pop-up, conflict rules, and protocol framing—with zero physical hardware required.
- **Procedural Web Audio Focus Soundscapes:** 8 mixable, zero-sample procedural channels (*White/Pink/Brown Noise, Gentle Rain, Ocean Waves, Forest Wind, Fireplace, Deep Focus Drone*), 4 curated scenes, and a smooth cosine fade-out sleep timer—works with *any* headphones.
- **Web Audio Pure-Tone Audiometry Wizard:** Built-in `StereoPannerNode`-isolated L/R 8-band hearing test with a conservative digital safety ceiling (`-9 dBFS`) and live dual-ear SVG compensation curve plotting.
- **AutoEQ `.txt` & JSON EQ Import/Export:** Share custom parametric EQ profiles locally in plain JSON or import community AutoEQ ParametricEQ `.txt` files directly.
- **Live Protocol Frame Inspector:** Real-time TX/RX hex frame monitor for developers and power users.

---

## Supported Device Models & Protocol Architecture

QCY Audio Desktop embeds a clean-room catalog of **43 QCY audio models** (`crates/qcy-models/data/models_catalog.json`):

- **26 Bundled Modern Profiles** (derived from all 34 observed QCY Audio v1.2.3 device capability schemas):
  - **TWS Earbuds:** MeloBuds Pro (`HT08` WuQi & `HT08 JL`), MeloBuds N70, MeloBuds N70i, MeloBuds A30 (`HT22` & `HT22 AI`), T13 ANC Pro, T13 ANC 3, HT19, HT20, HT20S, HT21, HT21 AI, HT23, HT27
  - **Over-Ear Headphones:** QCY H3, QCY H6 (16-level Custom ANC + Hearing Health), QCY H6 Neo
  - **Open-Ear / Clip Earbuds:** Crossky C50 AI, Crossky C50S, Crossky C50S AI, Crossky C50i (`CT12`), CT02 (onboard Focus Mode), CT03 (Privacy Mode + Dialogue Perception), CT11 (THX Audio)
  - **Bluetooth Speakers:** QCY SP75 (RGB LED Lighting + TWS Stereo Pairing)
- **16 Legacy / Cloud-Managed QCY Audio Profiles:** MeloBuds ANC (`HT05`), HT03, HT01, T18, T17S, T17, T16, T13, T12, T12S, AilyPods, FairyBuds, Crossky GTR, H2, G1, VT200
- **1 Universal Generic Fallback (`qcy-generic`)** for newly released or unrecognized QCY devices.

### Bluetooth Protocol Families & OTA Routes

Daily control operates **100% over BLE GATT without proprietary authentication or wire encryption**:

| Protocol Family | GATT Service / Transport | Chipsets / Models | Notes |
|---|---|---|---|
| **Passive BLE Advertising** | Manufacturer Data (`0x071E`, `0x521C`, `0x522C`) | All modern QCY models | 24-byte payload carrying `modelID`, colorway, L/R/Case battery+charging, popup trigger, and swapped Classic MAC |
| **QCY Unified TLV** | BLE `0xA001` (W `0x1001` / N `0x1002`) | WuQi, Bluetrum, BES, Qualcomm QCC, Realtek, Actions, PixArt | `[0xFF, Len, CmdID, VLen, Value...]` multi-TLV framing across 89 active CMDIDs + 4 EQ wire dialects |
| **JieLi RCSP (New)** | BLE `0xA002` (W `0x0001` / N `0x0002`) | New JieLi (`modelID & 0x8070 == 0x8070`: H6, HT20–HT23, HT08 JL, C50i, T13 ANC 3) | `FE DC BA ... EF` RCSP frame tunneling QCY TLVs inside `0xFF` (`CMD_EXTRA_CUSTOM`) |
| **JieLi RCSP (Legacy)** | BLE `0xA002` (W `0x0001` / N `0x0002`) | Legacy JieLi (H3, T13 ANC Pro) | Native RCSP opcodes (`0x03`, `0x07`/`0x08`, `0xC0`–`0xC5`) |
| **Local OTA Routes (9)** | BLE GATT (`0x7033`, `0xA002`, `0xFF12`, BES) & Classic RFCOMM (`0x1101`, GAIA, Actions) | `otaType` `1, 2, 5, 6, 7, 8, 10, 11` supported locally | `otaType = 9` (`0xAE00` `RcspAuth`) explicitly reports `UnsupportedAuthRequired` |

---

## Architecture & Repository Layout

```text
qcy/
├── crates/
│   ├── qcy-transport/      # Platform-independent BT traits + Windows WinRT BLE GATT & RFCOMM backend
│   ├── qcy-protocol/       # Zero-I/O pure-Rust codecs: 24B BLE Adv, QCY TLV (89 CMDIDs), JieLi RCSP, 9 OTA routes
│   ├── qcy-models/         # Compile-time catalog (43 models, 22 EQ presets, 113 pop_view_rules) + matcher
│   ├── qcy-simulator/      # Raw-byte Virtual QCY Earbuds Simulator (BLE Adv + GATT/RCSP loopbacks)
│   └── qcy-core/           # Unified DeviceManager, state snapshot, conflict rule engine, local persistence
├── src-tauri/              # Tauri v2 Windows host: IPC commands, System Tray, frameless Connection Popup window
├── src/                    # React 19 + TypeScript + Tailwind CSS v4 frontend
│   ├── assets/             # Original theme-aware vector DeviceIllustrations (5 form factors + SP75 RGB ring)
│   ├── audio/              # Procedural Web Audio engines: RBJ 9-filter biquad math, 8-ch soundscapes, hearing test
│   ├── components/         # Reusable QCY UI controls (Battery, ANC, Interactive PEQ SVG, Gestures, RGB, Modals)
│   ├── i18n/               # Strict key-parity zh-CN & en-US localization (669 keys) + runtime hook
│   ├── ipc/ & store/       # Typed Tauri IPC client (with browser fallback) & React state store
│   ├── pages/ & popup/     # Full app views + frameless Connection Pop-Up Overlay
├── scripts/                # Windows portable ZIP packager (package-portable.ps1)
├── .github/workflows/      # Windows MSVC CI & NSIS release workflow (ci.yml)
└── docs/
    ├── architecture-plan.md  # Full system architecture, IPC contract & Windows packaging guide
    ├── protocol-spec.md      # Clean-room Bluetooth GATT/RCSP/Adv/OTA wire specification
    └── feature-matrix.md     # Complete Android v1.2.3 feature inventory, UI tokens & model catalog
```

---

## Build, Test & Packaging (Windows 10/11)

### Prerequisites

- **OS:** Windows 10 (build 19041+) or Windows 11 x64 with Bluetooth 4.0+ (optional—Virtual Simulator works without Bluetooth hardware)
- **Toolchain:** [Rust (stable)](https://rustup.rs/), [Node.js 22+](https://nodejs.org/), and Microsoft Edge **WebView2 Runtime** (pre-installed on Windows 11)

### Development & Verification

```powershell
# 1. Install frontend dependencies
npm ci

# 2. Frontend verification (TypeScript strict check, Vitest unit tests, Vite bundle)
npm run typecheck
npm run test
npm run build

# 3. Rust workspace verification (format, clippy strict, all unit & transport tests)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# 4. Run the desktop application locally (Tauri dev with HMR, or embedded production assets)
npm run tauri -- dev
# or after `npm run build` (embeds `dist/` via `custom-protocol`):
cargo run -p qcy-desktop --features custom-protocol
```

### Packaging for Windows Distribution

```powershell
# Portable x64 ZIP (embeds `dist/`, verifies `--smoke-test`, and bundles WebView2Loader.dll on GNU)
npm run build
cargo build --release -p qcy-desktop --features custom-protocol
pwsh -File scripts/package-portable.ps1
# Output: dist-portable/qcy-desktop-windows-x64-portable.zip + SHA-256 checksum

# Windows NSIS Installer (MSVC toolchain, also built automatically by .github/workflows/ci.yml)
npm run tauri -- build --bundles nsis
```

---

## Transparent Limitations & Legal Disclaimer

- **Authenticated JieLi OTA (`otaType = 9`):** Devices using JieLi's `0xAE00` `RcspAuth` bootloader require a proprietary challenge-response authentication library not included in this open-source project. **Daily Bluetooth control and unauthenticated JieLi OTA (`otaType = 2`, used by all 26 bundled profiles) work normally**, while attempting to flash an `otaType = 9` image displays a clear, non-destructive notice.
- **Vendor Cloud Services Omitted by Design:** QCY user accounts, cloud EQ marketplace downloads, vendor OTA servers, and cloud AI voice/translation assistants are intentionally excluded to preserve 100% local offline operation and user privacy.
- **Find My Earbuds on Windows:** Per QCY hardware design (`0x4B`), Find My Earbuds plays an escalating stereo acoustic chime over the active Bluetooth Audio (A2DP) output rather than emitting a BLE radio beacon; ensure earbuds are **not** in your ears before ringing.
- **Disclaimer:** This is an independent, clean-room open-source interoperability project created for educational and personal desktop use. It is **not affiliated with, authorized, maintained, sponsored, or endorsed by Dongguan Hele Electronics Co., Ltd. or QCY**. All product names and trademarks belong to their respective owners.

---

<a id="简体中文"></a>

## 简体中文

**QCY Audio Desktop** 是一款专为 **Windows 10/11** 打造的**净室开源桌面伴侣应用**，用于本地管理与控制 QCY 全系蓝牙真无线耳机、开放式耳夹耳机、头戴式降噪耳机与蓝牙音箱。项目基于 **Rust + Windows WinRT 蓝牙 API + Tauri v2 + React 19 + TypeScript + Tailwind CSS v4** 构建，在完整还原移动端核心硬件控制体验的同时，提供原生 Windows 桌面增强功能。

### 净室开源与本地隐私承诺

- **无需账号登录：** 所有硬件控制、参数均衡器、听力测试与本地固件刷写均 **100% 离线本地运行**，无需注册或登录任何厂商云账号。
- **零遥测与零云端上报：** 不包含任何统计追踪代码、不连接厂商绑定或 OTA 服务器、不上传任何设备或个人数据。
- **不含任何专有密钥或版权素材：** 仓库内**不包含**厂商 API Key、私有加解密库、原版图片或音频资源；所有设备示意图均为原创矢量 SVG，所有白噪音与听力测试纯音均由 Web Audio API 实时程序合成。
- **MIT 开源协议：** 代码完全透明，欢迎审计、二次开发与贡献 PR。

### 核心功能与 Windows 桌面增强

1. **全功能硬件控制（对标 QCY Audio Android v1.2.3）**
   - **实时电量与设备信息：** 左耳 / 右耳 / 充电盒独立电量环形表与充电状态、低电量提醒、BLE 信号强度（RSSI）、固件版本、MAC 地址一键复制、蓝牙改名、外观配色切换。
   - **降噪控制（ANC）：** 降噪 / 正常 / 通透 / 自定义 16 档降噪深度（H6）、自适应 / 室内 / 通勤 / 户外 / 抗风噪多场景及深度调节、通透 1–6 档增益与人声加强、风噪检测、**AI 场景降噪 3×3 活动×噪音规则矩阵**、自定义 ANC 按键切换顺序（`btnAncCfg`）。
   - **音效与参数均衡器（PEQ）：** 22 套内置净室预设、10 段图示均衡器、支持 **全部 9 种 QCY IIR 双二阶滤波器**（高通、低通、峰值、高架、低架、高架 Q、低架 Q、高阶高通、高阶低通）的 10/20 段交互式对数频响曲线编辑器、5 条参考目标频响曲线叠加、**8 频段左右耳个性化听感测试**、动态 EQ、THX 音效、虚拟低音、自适应音量与左右声道平衡。
   - **手势与头部控制：** 支持 TWS 左右耳触控及头戴式物理按键（音量+/音量−/多功能键/模式键）自定义全部 **14 种动作（`cmd 0..18`）**、防误触手势开关、触摸灵敏度调节（`0x33`）、一键恢复默认，以及 **IMU 头部控制**（点头/摇头接拒来电、摆头切歌与练习指引）。
   - **音频模式与连接管理：** 聆听模式（音乐/游戏/电影）、游戏模式 / 游戏模式 Pro（同步游戏 EQ）、空间音频（关闭/固定/头部跟踪）、LDAC、实验性 LE Audio、佩戴检测、耳帽贴合度测试、听力健康提醒、隐私模式 Pro、对话感知、智能免摘、双设备与多设备主机管理。
   - **SP75 蓝牙音箱专项支持：** 4 种 RGB 灯效模式（单色常亮 / 单色呼吸 / 渐变呼吸 / 动感律动）、实时灯环预览、色盘/亮度/速度调节与 TWS 立体声串联组队。
   - **功能互斥与重启保护引擎（`pop_view_rules`）：** 完整还原各型号互斥规则（如 LDAC 与双设备/空间音频/游戏模式冲突），在执行前给出清晰确认提示与重启提醒。

2. **Windows 原生桌面增强特性**
   - **系统托盘常驻与快捷菜单：** 实时悬浮显示左右耳电量，右键快速切换降噪模式、EQ 预设、游戏模式与查找耳机，支持最小化至托盘与开机自启。
   - **无边框蓝牙连接弹窗：** 监听 QCY 24 字节 BLE 厂商广播数据（`0x071E` / `0x521C` / `0x522C`），开盖或连接时自动弹出 Fluent 风格电量卡片。
   - **内置虚拟 QCY 耳机模拟器：** 无需实体硬件即可模拟全部形态（TWS / 耳夹 / 头戴 / SP75 音箱）与协议族、模拟电量充放电、触发开盖弹窗并实时查看 TX/RX 十六进制协议帧。
   - **Web Audio 桌面环境音混音台：** 8 通道无采样程序合成环境音（白/粉/布朗噪音、细雨、海浪、林风、壁炉、冥想低鸣）、4 套推荐场景与平滑淡出睡眠定时，适配任意品牌耳机。
   - **AutoEQ `.txt` 与 JSON 预设导入导出：** 轻松分享或导入社区参数均衡器配置。

### 支持型号与协议架构

项目内置 **43 款 QCY 音频设备配置**（26 款现代内置完整能力型号 + 16 款经典型号 + 1 款通用回退型号 `qcy-generic`），日常控制 **100% 基于 BLE GATT 明文通信（无需私有鉴权或加密）**：
- **QCY 统一 TLV 协议（`0xA001`）：** 覆盖物奇（WuQi）、中科蓝讯（Bluetrum）、恒玄（BES）、高通（QCC）、瑞昱、炬芯、原相等平台，支持 89 个活跃 `CmdID` 与 4 种 EQ 线缆方言。
- **杰理 RCSP 协议（`0xA002`）：** 完整支持新版杰理（`isNewJL`，通过 `0xFF` 隧道封装 QCY TLV）与旧版杰理原生 RCSP。
- **9 条本地 OTA 刷写通道：** 支持通过本地 `.ota` / `.bin` 文件离线校验与分包传输（物奇 `0x7033`、杰理 `0xA002`、蓝讯 `0xFF12`、BES、原相/AWOTA、高通 GAIA、炬芯）。

详细技术文档请参阅：
- 架构设计与 IPC 契约：[docs/architecture-plan.md](docs/architecture-plan.md)
- 蓝牙线缆协议与 OTA 规范：[docs/protocol-spec.md](docs/protocol-spec.md)
- Android 功能矩阵与设备能力目录：[docs/feature-matrix.md](docs/feature-matrix.md)

### 编译、测试与打包

```powershell
# 安装前端依赖并验证类型、单元测试与生产构建
npm ci
npm run typecheck
npm run test
npm run build

# 验证 Rust 全工作区代码格式、Clippy 与单元测试
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# 启动本地桌面应用（热重载开发模式）
npm run tauri -- dev

# 打包 Windows x64 便携版 ZIP（内嵌 dist/、自动校验 --smoke-test 并处理 WebView2Loader.dll）
npm run build
cargo build --release -p qcy-desktop --features custom-protocol
pwsh -File scripts/package-portable.ps1

# 构建 Windows NSIS 安装包（MSVC 工具链）
npm run tauri -- build --bundles nsis
```

### 已知限制与免责声明

- **受鉴权杰理 OTA（`otaType = 9`）：** 使用 `0xAE00` `RcspAuth` 鉴权引导程序的设备需要闭源认证动态库，本净室开源版本暂不支持该通道刷写（不影响日常蓝牙控制，也不影响全部 26 款内置型号所用的无鉴权 `otaType = 2` 通道），选择此类设备刷写时界面会给出明确安全提示。
- **云端账号与 AI 服务：** 为保证 100% 本地隐私与离线可用性，厂商账号系统、在线音效商城、厂商 OTA 服务器及云端 AI 翻译/问答功能均未纳入本桌面版。
- **免责声明：** 本项目为独立净室开源互操作项目，**与东莞市和乐电子有限公司（QCY）无任何官方关联、授权或背书关系**，所有产品名称及商标归各自权利人所有。
