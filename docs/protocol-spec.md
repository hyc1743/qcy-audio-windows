# QCY Audio — Clean-Room Bluetooth & Device Protocol Specification

> **Document Purpose:** Clean-room, open-source engineering specification of the Bluetooth communication protocols used by QCY Audio Bluetooth audio devices (TWS/OWS earbuds, over-ear headphones, and Bluetooth speakers), derived from static inspection of the QCY Audio Android companion application for interoperability on Windows desktop.
>
> **Evidence & Epistemic Hygiene:** Every factual claim is tagged as **`[Observed]`** (directly verified in APK manifest, DEX bytecode, native ELF exports, or bundled resource schemas), **`[Inferred]`** (deduced from observed patterns and cross-references), or **`[Unknown]`** (not determinable from static analysis alone). All proprietary vendor secrets, cryptographic keys, API keys/tokens, signing certificates, and copyrighted media/config blobs have been **redacted**.

---

## 0. Target Artifact & Evidence Index

| Artifact / Target | SHA-256 | Size | Notes / REA Evidence ID |
|---|---|---|---|
| `QCY+Audio_1.2.3_APKPure.xapk` | `0efd137dc6d0c0441b1eef22c022dafc834427b699548549a0bbb9ce9a4f43d2` | 38,883,848 B | Genuine XAPK archive (`com.qcy.audio.google` v1.2.3). *(Note: An earlier YingYongBao APK `应用宝-QCY Audio.apk` was verified to be a Tencent app-store downloader stub `com.tencent.android.qqdownloader` v9.3.0 and was discarded.)* |
| `com.qcy.audio.google.apk` (inside XAPK) | `840fb10ef0edea868be19adf536ed34170c8470d950efbbed2f915aa270b008a` | 38,488,359 B | `[Observed]` APK signature v2 + v3 + source stamp, self-consistent. REA Evidence ID: `ev_0b1a31af0b6b833e70f89071cd4efa34871d25c66106709135e913fce972f137` |
| `lib/armeabi-v7a/libjl_enc.so` | `b4aba62bcb4ccfd3f86d86fa48360d32eb7a11f09d989b24638658f41db07277` | 13,544 B | `[Observed]` JieLi BLE adv-hash decryption JNI library. REA Evidence ID: `ev_ea6be3fa46573d910fad22035bafd988e446690a324e81c3350a0388b7607710` |
| `lib/armeabi-v7a/libjl_ota_auth.so` | `503b187a690d153e42fda622cbebff7ca46a337e6b63c6196b931beabbad8074` | 10,948 B | `[Observed]` JieLi RCSP OTA authentication & firmware filter JNI library. REA Evidence ID: `ev_e9b5b8007ff93ad5658de2300a7f007eab6ef15d65d4b5191458ac241d59b07d` |

---

## 1. Application Tech Stack, Architecture & Embedded SDKs

### 1.1 Application Metadata & Build Profile `[Observed]`
- **Package Name:** `com.qcy.audio.google` (`BuildConfig.APPLICATION_ID`, `FLAVOR = "google"`, `MARKET_CHANNEL = "googlePlay"`)
- **Version:** `versionName = "1.2.3"`, `versionCode = 123`
- **SDK Levels:** `minSdkVersion = 29` (Android 10), `targetSdkVersion = 36`
- **UI / App Framework:** Pure native Android (**Kotlin + Java**), DataBinding, Koin DI, TheRouter, Room DB, Retrofit2 / OkHttp3, Kotlin Coroutines / Flow. **No Flutter, React Native, Unity, or WebView-based control runtime.**
- **Packer / Obfuscation Status:** **Unpacked and unobfuscated.** All 4 DEX files (`classes.dex`–`classes4.dex`, 27,708 total classes) preserve original class, method, field, and Kotlin metadata names for `com.qcy.audio.*`, `com.qcymall.qcydevicelibrary.*`, and most vendor OTA SDKs (only parts of `com.jieli.jl_bt_ota.*` are lightly shrunk with `a`/`b` internal fields).
- **Bluetooth Hardware & Manifest Permissions (`AndroidManifest.xml`):**
  - Requires `<uses-feature android:name="android.hardware.bluetooth_le" android:required="true" />`.
  - Declares `BLUETOOTH`, `BLUETOOTH_ADMIN`, `BLUETOOTH_CONNECT`, `BLUETOOTH_SCAN` (`usesPermissionFlags = 0x10000` / `neverForLocation`), `ACCESS_FINE_LOCATION`, `ACCESS_COARSE_LOCATION`, `INTERNET`, `ACCESS_NETWORK_STATE`, `FOREGROUND_SERVICE`.
  - **Dead Realtek DFU Manifest Entry:** Declares `<service android:name="com.realsil.sdk.dfu.DfuService" android:enabled="true" android:exported="true" />`, but **zero `com.realsil.*` classes exist in the DEX files** (dependency stripped at build time; `QCYDeviceBean.isRTKDevice()` has zero callers).

### 1.2 Native `.so` Libraries `[Observed]`
Only **two** native libraries exist in the APK (both `armeabi-v7a` only, neither used by QCY's non-JieLi device control layer):

1. **`libjl_enc.so`** (`ev_ea6be3fa...`)
   - Loaded by `com.jieli.bt.decryption.HashDecryption` (invoked by `com.jieli.bluetooth_connect.util.ParseDataUtil.parseWithTwsVer1` / `parseWithChargingBin`).
   - Exported symbols include `Java_com_jieli_bt_decryption_HashDecryption_decryptData`, `btcon_hash`, and AES lookup tables (`Te0`–`Te3`, `Td0`–`Td3`, `rcon`).
   - Role: Decrypts/verifies JieLi-format charging-case BLE advertisement hashes inside JieLi's stock connection library. Not called on QCY's primary `0x071E`/`0x521C` advertisement parser.
2. **`libjl_ota_auth.so`** (`ev_e9b5b800...`)
   - Loaded by `com.jieli.jl_bt_ota.impl.RcspAuth` and `com.jieli.jl_bt_ota.util.ParseDataUtil`.
   - Exported symbols include `JNI_OnLoad`, `Java_com_jieli_jl_1bt_1ota_util_ParseDataUtil_nativeFilterFile`, `parse_fw_info`, `CRC16`, `cd03_crc_encode`, `decrypt`, `function_E21`, `function_xiaomi`, `register_xm_bluetooth`.
   - Role: Implements JieLi's `RcspAuth` challenge-response authentication (`nativeInit`, `getRandomAuthData`, `getEncryptedAuthData`, `setLinkKey`) and firmware header filtering (`nativeFilterFile`) for JieLi OTA (`otaType = 9` with `useDeviceAuth = true`).

### 1.3 Embedded Bluetooth & Vendor SDK Inventory `[Observed]`

| Layer / Package | Vendor / Role | Transport Used in QCY App | Active in Daily Control? | Active in OTA? |
|---|---|---|---|---|
| `com.qcymall.qcydevicelibrary.*` | QCY first-party device SDK (`QCYDeviceManager`, `QCYDeviceImpl`, `JLDeviceImpl`, `CommandProcessor`) | BLE GATT (`0xA001` & `0xA002`) | **Yes (Primary)** | Dispatches presenters |
| `cn.com.heaton.blelibrary.*` | Third-party Android BLE wrapper (`Ble`, `RequestTask`) | BLE GATT | **Yes (Transport layer)** | Used by BES & Bluetrum OTA |
| `com.jieli.jl_bt_rcsp.*`, `com.jieli.bluetooth_connect.*`, `com.jieli.jl_bt_ota.*` | **JieLi (杰理)** RCSP & OTA SDK | BLE GATT (`0xA002` or `0xAE00`; library also contains unused SPP) | **Yes (JieLi models)** | **Yes (`otaType` 2, 9)** |
| `org.wuqi.android.*`, `tool.android.all.*` | **WuQi (物奇)** OTA & CLI SDK (`WQBLEPresenter`, `WQBleOtaBridge`) | BLE GATT (`0x7033` / `2001` / `2002`); legacy `WQPresenter` SPP unused by router | No (uses QCY `0xA001`) | **Yes (`otaType` 6, 11)** |
| `com.bluetrum.fota.*` | **Bluetrum (中科蓝讯)** FOTA SDK (`FOtaPresenter`, `ABOta`) | BLE GATT (`0xFF12` / `FF14` / `FF15`) | No (uses QCY `0xA001`) | **Yes (`otaType` 10)** |
| `com.qcymall.qcydevicelibrary.ota.besota.*` | **BES (恒玄 / Bestechnic)** OTA (`BESOtaPresenter`) | BLE GATT (`6666...` / `7777...`) | No (uses QCY `0xA001`) | **Yes (`otaType` 8)** |
| `com.qualcomm.qti.gaiaclient.*`, `com.qualcomm.qti.libraries.upgrade.*` | **Qualcomm QCC** GAIA v2/v3 Upgrade SDK (`QCCOTAPresenter`) | **Classic BT RFCOMM** (`00001107-...`) | No (uses QCY `0xA001`) | **Yes (`otaType` 5)** |
| `com.actions_local.*` | **Actions Semiconductor (炬芯)** Bluz OTA SDK (`ActsOtaPresenter`) | **Classic BT SPP (`SPP_ONLY`)** (`0000E003-...`) | No (uses QCY `0xA001`) | **Yes (`otaType` 7)** |
| `com.awota.ota.*`, `com.qcymall.qcydevicelibrary.ota.t5.*` | **PixArt / AWOTA** (`PA`/`PI` protocol, parts 267/279/281, `T5OTAPresenter`) | **Classic BT SPP** (`DevObjSPP`) | No (uses QCY `0xA001`) | **Yes (`otaType` 1)** |
| `com.realsil.sdk.dfu.DfuService` | Realtek DFU (Manifest stub only; **0 DEX classes**) | N/A | No | No |

### 1.4 Cloud-Driven Model Catalog & Dynamic Control Config `[Observed]`
Rather than hardcoding every QCY model's UI layout and capabilities in Java/Kotlin, the app uses a **data-driven JSON schema** (`com.qcy.audio.link.controlfun.DeviceConfig` / `ControlPanel`) backed by REST services (`com.qcy.audio.http.HttpAPI`) with offline fallbacks in `res/raw/function_*.json`:
- **Product Catalog:** `IProductService.getProductConfigUrl()` returns a ZIP URL downloaded and unpacked by `ProductManager` into `EarphoneProVO` records indexed by `vendorID` (`modelID`) in `ProductExtKt.mAllDeviceMap`.
- **Per-Model Feature & Command Schema:** `IEarphoneService.requestControlFun(modelId, firmwareVersion)` returns a `ControlPanel` containing `DeviceConfig` (49 feature slots; each `BaseControlModel` carries `cmdID`, optional `otherCmdID`, `onlyTws`, `SubItem(index, cmdHex, name)`, `ButtonInfo`, `AncMode`, `OTAControlModel(otaType)`, and `PopViewRulesModel`).
- **Firmware OTA Check:** `IEarphoneService.checkUpgrade(EarUpgradeDto)` returns `EarphoneOTAVO` (`currentVersion`, `firmwareUrl`, `modelId`, `upgradeType`, `versionName`, `remarks`, `skip`, `state`, `status`).
- **Request Headers (`HeaderInterceptor`):** Attaches `Authorization: Bearer <REDACTED>`, `lang` (`zh`, `zh_hant`, etc.), `country`, `sys_: Android`, and `app_version`.
- **Redacted Credentials:** `BuildConfig` contains hardcoded third-party AI/RTASR keys (`AI_API_KEY`, `AI_API_SECRET`, `AI_APPID`, `AI_RTASR_APIKEY` — **`[REDACTED]`**), unrelated to Bluetooth control.

---

## 2. Transport Layers

### 2.1 High-Level Transport Summary `[Observed]`
- **Daily Device Control (100% BLE GATT):** Every daily control feature across **all** chipsets (battery, ANC, EQ, gestures, modes, multipoint, in-ear detection, LED effects, white noise, head tracking, fit test, rename, factory reset) runs exclusively over **BLE GATT** — either QCY's custom GATT service (`0xA001`) or JieLi's RCSP GATT service (`0xA002`).
- **Zero HFP AT Commands:** Static scan across all 4 DEX files confirmed **zero** occurrences of `ACTION_VENDOR_SPECIFIC_HEADSET_EVENT` or `sendVendorSpecificResultCode`. QCY does **not** use HFP AT commands.
- **Classic Bluetooth A2DP/HFP Role:** `ClassicBtUtil`, `ClassisBluetoothManager`, and `BtClassicUtils` use `BluetoothA2dp` / `BluetoothHeadset` **solely** to:
  1. Match a BLE-discovered earbud's `classMac` / `otherMac` against currently connected Classic A2DP audio devices (filtering QCY Bluetooth MAC OUI prefixes **`84:AC:60`**, **`1C:52:16`**, and **`C4:33:96`** in `BtClassicUtils.<init>`).
  2. Trigger OS pairing/A2DP connect/disconnect (`createBond`, `removeBond`, reflection `connect`/`disconnect`).
  3. Route local audio playback over A2DP for *Find My Earbuds* (`FindEarphonePlayer`) and *Personalized Hearing Test* (`BeepTool`).
- **Classic Bluetooth SPP / RFCOMM Role:**
  - `com.qcymall.qcydevicelibrary.BluetoothSPPManager` exists in the SDK (using standard SPP `00001101-...`) with **zero external callers** in `com.qcy.*` / `com.qcymall.*`.
  - Classic SPP/RFCOMM is **only** required for **legacy/specific OTA firmware upgrades**: `otaType = 1` (PixArt/T5), `otaType = 5` (Qualcomm QCC GAIA), and `otaType = 7` (Actions). Modern chipsets (JieLi `otaType = 2/9`, WuQi `otaType = 6/11`, BES `otaType = 8`, Bluetrum `otaType = 10`) perform **OTA over BLE GATT as well**.

### 2.2 BLE Advertising & Manufacturer-Specific Data Parsing `[Observed]`
Parsed in `QCYDeviceManager$scanCallback$1.onLeScan(...)` (`com.qcymall.qcydevicelibrary.QCYDeviceManager`).

#### Recognized Bluetooth SIG Company / Manufacturer IDs (AD Type `0xFF`)
| Constant (`QCYDeviceManager`) | Value (Hex) | Value (Dec) | Usage |
|---|---|---|---|
| `QCY_MANUFACTURERID` | `0x071E` | `1822` | Current QCY devices (subject to local `supportIDS` allowlist check in the official app) |
| `QCY_MANUFACTURERID_OLD` | `0x521C` | `21020` | Legacy QCY devices (unconditionally allowed in official app) |
| `QCY_MANUFACTURERID_CUSTOM` | `0x522C` | `21036` | Custom / OEM QCY variants |

*(Note: In raw BLE advertisement bytes, the 2-byte Company ID is transmitted Little-Endian per Bluetooth Core Spec, e.g. `1E 07` for `0x071E`; Android's `ScanRecord.getManufacturerSpecificData(id)` and WinRT's `BluetoothLEManufacturerData.CompanyId()` expose it as a `u16` and strip the 2-byte Company ID from the payload buffer below.)*

#### Manufacturer Data Payload Layout (`data[0..N-1]`, up to 24 bytes)

| Offset | Size | Field (`QCYDeviceBean`) | Bit/Byte Encoding | Notes |
|---|---|---|---|---|
| `0..1` | 2 B | `modelID` (`vendorID`) | `u16` **Big-Endian**: `((b[0] & 0xFF) << 8) \| (b[1] & 0xFF)` | Identifies product + chipset family + form factor (see Section 4.1) |
| `2` | 1 B | Reserved / Flags | `[Unknown]` | Not read in `QCYDeviceManager$scanCallback$1` |
| `3` | 1 B | Ear side & Color | `bit 0`: `isLeft` (`(b[3] & 0x01) != 0`)<br>`bits 1..3`: `colorIndex` (`(b[3] >> 1) & 0x07`) | Used for colorway UI & single-bud orientation |
| `4` | 1 B | Connection / Popup flags | `bit 6`: `isTws` (`(b[4] & 0x40) != 0`)<br>`bit 5 \| bit 1`: `isPop` (`(b[4] & 0x22) != 0`) | Triggers lid-open popup animation when `isPop` is set and `popNum` changes |
| `5` | 1 B | Left Battery | `bit 7`: `isLeftCharging`<br>`bits 0..6`: `leftBattery` (`0..100`, `0x7F`/`127` or `0` when absent/disconnected) | Live battery directly in BLE advertisements (no GATT connection required!) |
| `6` | 1 B | Right Battery | `bit 7`: `isRightCharging`<br>`bits 0..6`: `rightBattery` (`0..100`) | Same bit packing |
| `7` | 1 B | Charging Case Battery | `bit 7`: `isBoxCharging`<br>`bits 0..6`: `boxBattery` (`0..100`) | Same bit packing |
| `8..10` | 3 B | Reserved / Protocol flags | `[Unknown]` | Not read in `scanCallback` |
| `11..16` | 6 B | `classMac` (Primary Classic BT MAC) | **Swapped-order 6 bytes:** `b[12]:b[11]:b[13]:b[16]:b[15]:b[14]` | Formatted as uppercase `%02X:...` for matching Windows Classic A2DP device |
| `17` | 1 B | `popNum` | `u8` (`b[17] & 0xFF`) | Rolling counter incremented on case lid-open to deduplicate popups |
| `18..23` | 6 B | `otherMac` (Peer Earbud Classic MAC) | **Swapped-order 6 bytes:** `b[19]:b[18]:b[20]:b[23]:b[22]:b[21]` | Present when `data.length >= 24`; identifies secondary TWS bud |

> **Critical Implementation Note (MAC Byte Order):** `[Observed]` In `QCYDeviceManager$scanCallback$1`, the 6-byte Classic MACs at offsets `11..16` and `18..23` are **NOT** straight big-endian or straight little-endian — they are arranged as **`(b[1], b[0], b[2], b[5], b[4], b[3])`** relative to each 6-byte slice (i.e., `b[12], b[11], b[13], b[16], b[15], b[14]`). Verify against live Windows advertisements during Phase 3 hardware testing.

### 2.3 Complete Bluetooth UUID Master Table `[Observed]`

#### A. Daily Control — BLE GATT Services & Characteristics

| Protocol Family | Role | UUID (128-bit) | 16-bit Short | GATT Props / Notes | Source Citation |
|---|---|---|---|---|---|
| **QCY Unified Custom** | Primary Service | `0000A001-0000-1000-8000-00805F9B34FB` | `0xA001` | Used by WuQi, Bluetrum, BES, QCC, Actions, PixArt | `QCYDeviceImpl.SERVICE_UUID` |
| QCY Unified Custom | Command Write | `00001001-0000-1000-8000-00805F9B34FB` | `0x1001` | Write (`0xFF` TLV frames) | `QCYDeviceImpl.CHARACTER_WRITE_UUID` |
| QCY Unified Custom | Response / Notify | `00001002-0000-1000-8000-00805F9B34FB` | `0x1002` | Notify (`0xFF` TLV frames; enable CCCD `0x2902`) | `QCYDeviceImpl.CHARACTER_READ_UUID` |
| QCY Legacy Direct | Firmware Version | `00000007-0000-1000-8000-00805F9B34FB` | `0x0007` | Read (3B: `major.minor.patch`); fallback to TLV `0x30` if absent | `QCYDeviceImpl.CHARACTER_VERSION_UUID` |
| QCY Legacy Direct | Battery Level | `00000008-0000-1000-8000-00805F9B34FB` | `0x0008` | Read/Notify (3B L/R/Box); fallback to TLV `0x2F` if absent | `QCYDeviceImpl.CHARACTER_BATTERY_UUID` |
| QCY Legacy Direct | 10-Band Gain EQ | `0000000B-0000-1000-8000-00805F9B34FB` | `0x000B` | Read/Write (`[eqType, gain0..9]`); presence disables Multi-EQ (`isSupportMultiEQ`) | `QCYDeviceImpl.CHARACTER_EQ_UUID` |
| QCY Legacy Direct | Button Mapping | `0000000D-0000-1000-8000-00805F9B34FB` | `0x000D` | Read/Write (`[eventId, actionCmd]` pairs); fallback to TLV `0x2B` if absent | `QCYDeviceImpl.CHARACTER_BUTTON_UUID` |
| **JieLi RCSP (QCY)** | Primary Service | `0000A002-0000-1000-8000-00805F9B34FB` | `0xA002` | Overrides stock JieLi `0xAE00`; used by both legacy JL & `isNewJL` | `QCYDeviceManager.UUID_JL_SERVICE` |
| JieLi RCSP (QCY) | Command Write | `00000001-0000-1000-8000-00805F9B34FB` | `0x0001` | Write (`0xFE DC BA ... EF` RCSP frames) | `QCYDeviceManager.UUID_JL_WRITE` |
| JieLi RCSP (QCY) | Response / Notify | `00000002-0000-1000-8000-00805F9B34FB` | `0x0002` | Notify (enable CCCD `0x2902`) | `QCYDeviceManager.UUID_JL_NOTIFICATION` |
| Standard GATT | CCCD | `00002902-0000-1000-8000-00805F9B34FB` | `0x2902` | Client Characteristic Configuration (`0x01, 0x00` for Notify) | `Ble`, `BluzDeviceBle` |

#### B. Firmware OTA — BLE GATT & Classic RFCOMM UUIDs

| OTA Family (`otaType`) | Transport | Role | UUID (128-bit) | Source Citation |
|---|---|---|---|---|
| **JieLi Standard (`otaType = 9`)** | BLE GATT | OTA Service / Write / Notify | `0000AE00-...` / `0000AE01-...` / `0000AE02-...` | `BluetoothConstant`, `JLSDKCommonPresenter` (`useAuth=true`) |
| **JieLi QCY (`otaType = 2`)** | BLE GATT | OTA Service / Write / Notify | `0000A002-...` / `00000001-...` / `00000002-...` | `JLSDKCommonPresenter` (`useAuth=false`) |
| **WuQi BLE (`otaType = 6, 11`)** | BLE GATT | OTA Service / Write / Notify | `00007033-...` / `00002001-...` / `00002002-...` | `WQBleOtaBridge`, `org.wuqi.android.core.base.UUIDValue` |
| **BES (`otaType = 8`)** | BLE GATT | OTA Service / Write+Notify | `66666666-6666-6666-6666-666666666666` / `77777777-7777-7777-7777-777777777777` | `BESOtaPresenter` |
| **Bluetrum FOTA (`otaType = 10`)** | BLE GATT | OTA Service / Notify(In) / Write(Out) | `0000FF12-...` / `0000FF14-...` / `0000FF15-...` | `FOtaPresenter` |
| **Qualcomm GAIA (`otaType = 5`)** | Classic RFCOMM | GAIA Service (SPP fallback) | `00001107-D102-11E1-9B23-00025B00A5A5` (`00001101-...`) | `com.qualcomm.qti.gaiaclient.*` |
| **Actions (`otaType = 7`)** | Classic SPP (`SPP_ONLY`) | Custom SPP UUID | `0000E003-0000-1000-8000-00805F9B34FB` | `ActsOtaPresenter.SPP_UUID` |
| **PixArt / AWOTA (`otaType = 1`)** | Classic SPP | Device SDP `getUuids()[0]` | Dynamic from SDP (typically `00001101-...` SPP) | `DevObjSPP.reopenSocket()` |

*(Unused vendor SDK default UUIDs compiled into the APK but NOT actively routed by QCY include: Actions BLE `e49a25f8/25e0/28e1-f69a-11e8-8eb2-f2801f1b9fd1`; WuQi SPP `0x7034` / CLI `0x7033` / Dump `0x7035` / Sound `0x2023` & `0CF12D31-FAC3-4553-BD80-D6832E7B3939`; Xiaomi BES/WQ placeholders `0xAF00`/`AF05`/`AF06`; standard SPP `0x1101` in unused `BluetoothSPPManager` and `SppManager`.)*

### 2.4 BLE GATT Connection Lifecycle, MTU, Write Types & Pacing `[Observed]`
Verified in `QCYDeviceManager$mBleCallback$1`, `QCYDeviceManager$transferCallBack$1`, `QCYDeviceImpl`, `JLDeviceImpl`, and `CommandProcessor`:
1. **Connect & Discover Services:** Connect to BLE peripheral -> discover GATT services.
2. **Protocol Selection on Service Discovery:**
   - If `device.isJLDevice()` **or** GATT service `0xA002` is present: instantiate `JLDeviceImpl(context, bleDevice, isNewJL = device.isNewJL(), callback)`.
   - Otherwise (GATT service `0xA001` present): instantiate `QCYDeviceImpl(context, deviceBean, callback)`.
3. **MTU Negotiation:**
   - `QCYDeviceManager` requests **`MTU = 512`** (`Ble.setMTU(address, 512, ...)`).
   - Negotiated MTU is stored in `QCYDeviceBean.mtuValue` and used to clamp parametric EQ band count (`(mtu - 7) / 6` in `QCYDeviceImpl.setEQWtihCID`) and packet sizes.
   - JieLi OTA (`JLSDKCommonPresenter`) requests `MTU = 256`; WuQi SPP legacy presenter uses `553`.
4. **Enable Notifications (CCCD `0x2902`):**
   - For `0xA001`: enable notify on `0x1002` (and on legacy `0x0008` if present).
   - For `0xA002`: enable notify on `0x0002`.
5. **GATT Write Type:**
   - Daily control writes via `cn.com.heaton.blelibrary` (`RequestTask` -> `Ble.writeQueue`) do **not** override `BluetoothGattCharacteristic.writeType`; they use the characteristic's default (`WRITE_TYPE_DEFAULT` = Write With Response, or `WRITE_TYPE_NO_RESPONSE` if the peripheral advertises only Write-Without-Response). On Windows WinRT, inspect `CharacteristicProperties` and use `WriteWithResponse` when `Write` is set, falling back to `WriteWithoutResponse` when only `WriteWithoutResponse` is set.
   - **WuQi BLE OTA (`WQBleOtaBridge`)** explicitly inspects `0x2001` properties and logs `WRITE_NO_RESPONSE` / `WRITE` accordingly.
6. **Command Queue & Pacing (`CommandProcessor`):**
   - Outbound commands per device are serialized through `CommandProcessor`, which enforces a **`100 ms`** delay between sequential GATT writes to avoid firmware RX buffer drops.
   - After connection + notify ready, the app issues `readDeviceInfo()` (version `0x30`, battery `0x2F`, ANC `0x17`, EQ `0x20`/`0x22`/`0xFC`, buttons `0x2B`, plus all feature `cmdID`s configured in `DeviceConfig` via `0xFE` read requests).

---

## 3. Packet Framing, Encryption & Handshake / Authentication

### 3.1 QCY Unified Custom TLV Protocol (Service `0xA001`, Chars `0x1001` / `0x1002`) `[Observed]`

Verified in `QCYDeviceImpl$Companion.getSendData([B)`, `QCYDeviceImpl.callbackReceiveDeviceData`, and `QCYDeviceImpl.readCustomAttrInfo`.

#### Wire Frame Format (Host <-> Device)
```
Byte:   0        1          2      3       4 .. (4+VLen-1)   [Optional additional TLVs...]
      +------+------------+------+------+------------------+
      | 0xFF | PayloadLen | Cmd  | VLen | Value[0..VLen-1] |  ...
      +------+------------+------+------+------------------+
```
- **Header (`byte[0]`):** Always `0xFF` (`-1` signed byte).
- **Frame Payload Length (`byte[1]`):** Unsigned `u8` equal to the total byte length of all concatenated TLVs following `byte[1]` (i.e. `frame.length - 2`).
- **TLV Entry:**
  - `Cmd` (`u8`): Command / Attribute ID (see Section 4.2).
  - `VLen` (`u8`): Byte length of `Value` (`0` for trigger commands like Factory Reset `0x03`, Restore `0x01`, Clear Pair `0x02`).
  - `Value` (`VLen` bytes): Command payload. Multi-byte scalar integers default to **Unsigned Little-Endian** (verified in `QCYDeviceManager$transferCallBack$1.onCustomDataReceive` loop `04b0..04bf`, which iterates `i = len - 1` down to `2`, shifting `(acc << 8) | (b[i] & 0xFF)`).
- **Multiple TLVs per Frame:** `QCYDeviceImpl.callbackReceiveDeviceData` loops from offset `2` while `offset < frame.length`, slicing each `[Cmd, VLen, Value...]` (`VLen + 2` bytes) and dispatching `onCustomDataReceive(mac, tlvBytes)`. Clean-room parsers **must** support multiple TLVs packed into a single BLE notification!
- **Sequence Number / Checksum / CRC / Encryption:** **NONE.** Plaintext framing over BLE link-layer encryption only.

#### Attribute Read Request (`CMDID_REQUESTDATA = 0xFE`)
To query the current value of any attribute `TargetCmdID`, the host sends a 5-byte frame:
```
TX: [0xFF, 0x03, 0xFE, 0x01, TargetCmdID]
```
The device responds via `0x1002` Notify with:
```
RX: [0xFF, (VLen + 2), TargetCmdID, VLen, Value[0..VLen-1]]
```

#### Action / Trigger Commands (Zero-Length Value)
```
Restore Settings (0x01):   [0xFF, 0x02, 0x01, 0x00]
Clear Pair Record (0x02):  [0xFF, 0x02, 0x02, 0x00]
Factory Reset (0x03):      [0xFF, 0x02, 0x03, 0x00]
```

### 3.2 JieLi RCSP Framing & `isNewJL` TLV-in-RCSP Encapsulation (Service `0xA002`) `[Observed]`

Verified in `com.jieli.jl_bt_rcsp.*` and `com.qcymall.qcydevicelibrary.jldevice.JLDeviceImpl`.

#### JieLi RCSP Wire Frame
```
Byte:   0    1    2     3       4         5..6              7 .. (7+PLen-1)   Last
      +----+----+----+-------+--------+----------+-------------------------+------+
      |0xFE|0xDC|0xBA| Flags | OpCode | PLen(BE) |   Body[0..PLen-1]       | 0xEF |
      +----+----+----+-------+--------+----------+-------------------------+------+
```
- **Sync Header:** `FE DC BA` (3 bytes).
- **Flags (`byte[3]`):**
  - `bit 7` (`0x80`): `1` = Command, `0` = Response.
  - `bit 6` (`0x40`): `1` = Expects Response (`hasResponse`).
  - Typical Command-with-Response: `0xC0`; Command-without-Response: `0x80`; Response: `0x00`.
- **OpCode (`byte[4]`):** RCSP command ID (`u8`).
- **Param Length (`bytes[5..6]`):** `u16` **Big-Endian**.
- **Body:**
  - Command with response: `[OpCodeSN (u8), Params...]`
  - Response: `[Status (u8, 0=OK), OpCodeSN (u8), ResponseParams...]`
- **Trailer:** `0xEF` (1 byte).

#### Two JieLi Sub-Dialects in `JLDeviceImpl`
1. **New JieLi (`isNewJL == true`, `(modelID & 0x8070) == 0x8070`, e.g., HT20 `0x8074`, H6 `0x8170`):**
   - Uses RCSP `OpCode = 0xFF` (`CMD_EXTRA_CUSTOM` / `CustomCmd`) to tunnel **standard QCY TLVs**!
   - `JLDeviceImpl.setCustomCMD(tlvBytes)` wraps `[CmdID, VLen, Value...]` (or `[0xFE, 0x01, TargetCmdID]` for reads) inside an RCSP `0xFF` `CustomCmd` payload — i.e., `Body = [OpCodeSN, 0xFF, PayloadLen, CmdID, VLen, Value...]` or raw TLV inside `CustomCmd` (exact `setCustomCMD` prepends `[0xFF, len]` via `QCYDeviceImpl.getSendData` before placing inside `CustomParam`!).
   - **Important `isNewJL` Boolean Quirk `[Observed]`:** For `0x06` (In-Ear Check) and `0x09` (Game Mode), `JLDeviceImpl` sends `1` = ON and **`2` = OFF** (instead of `0` = OFF) on `isNewJL` devices!
2. **Legacy JieLi (`isNewJL == false`, e.g., H3 `0x417C`, T13 ANC Pro `0x5071`, HT08 JL `0x9072`):**
   - Uses native JieLi RCSP opcodes for core features:
     - `0x03` (`CMD_GET_TARGET_INFO`): firmware version, battery, EQ, capabilities
     - `0x07` / `0x08` / `0x09`: SYS_INFO get/set/notify (10-band EQ `0x04`, ANC `0x0D`/`0x0E`, Sound Mode, etc.)
     - `0xC0`–`0xC5` (`CMD_ADV_SETTINGS_*`): TWS battery, device rename, button mapping (`modifyAdvInfo(type=2, [keyIdx, gestureAction, jlFunc])`), ANC mode list, LED/work mode
     - `0xD1` (`CMD_SETTINGS_COMMUNICATION_MTU`): RCSP MTU negotiation
     - `0xE7` (`CMD_REBOOT_DEVICE`): reboot / reset
     - `0xFF` (`CMD_EXTRA_CUSTOM`): QCY custom TLV extensions (e.g., parametric EQ, extended features)

### 3.3 Authentication & Encryption Analysis `[Observed]`
1. **Daily Control (`0xA001` & `0xA002`):** **Completely unencrypted on the wire; zero challenge-response handshake required.**
2. **Resolved Mystery — `QCYDeviceManager.aesKey` (`[REDACTED]`):**
   - Static inspection of `QCYDeviceManager.connectBleDevice(...)` proved `aesKey` is **NOT** used for Bluetooth wire encryption.
   - Instead, it is a **purely local app allowlist gate**: when connecting to a scanned BLE device, the SDK packs `(manufacturerId << 16) | modelId` into 4 Big-Endian bytes. If `(packed & 0xFFFF0000) == 0x521C0000` (legacy `0x521C`), it connects unconditionally; otherwise it AES-256-ECB-PKCS5Padding encrypts those 4 bytes with hardcoded `aesKey` (`[REDACTED]`) and checks if the uppercase hex string is in `supportIDS` (initialized by `AppInitializer`).
   - Because this check runs solely in the Android client and sends nothing to the earbuds, a clean-room Windows client needs **no proprietary keys** to connect and control any QCY device.
3. **JieLi `RcspAuth` (OTA `otaType = 9` only):**
   - When `JLSDKCommonPresenter` is instantiated with `useDeviceAuth = true` (`otaType = 9`, GATT `0xAE00`/`AE01`/`AE02`), `com.jieli.jl_bt_ota.impl.RcspAuth` runs a mutual 17-byte challenge-response exchange backed by `libjl_ota_auth.so` (`getRandomAuthData()`, `getEncryptedAuthData()`, success sentinel `[0x02, 0x70, 0x61, 0x73, 0x73]` = `"\x02pass"`, optional reset packet `FE DC BA C0 06 00 02 00 01 EF`).
   - In contrast, `otaType = 2` (used by **all** bundled JieLi models in `res/raw/function_*.json`: H3, H6, H6 Neo, HT08 JL, HT20/20s/21/22/23, CT12, T13 ANC 3/Pro) runs `JLSDKCommonPresenter` with `useDeviceAuth = false` over `0xA002` — **no `RcspAuth` required!**

---

## 4. Multi-Chipset Routing & Complete Command Catalog

### 4.1 16-Bit `modelID` Bitmask Routing (`QCYDeviceBean`) `[Observed]`

Verified in `com.qcymall.qcydevicelibrary.bean.QCYDeviceBean` and cross-checked against all 22 bundled `modelID` entries in `ControlFuncViewModelKt.<clinit>` + 34 `res/raw/function_*.json` files.

#### Bitmask Decoding Rules (`modelID` = 16-bit Big-Endian from BLE Adv bytes `[0..1]`)

| Bit Mask | Matched Value | Helper Method / Meaning | Status | Control Protocol | Typical OTA `otaType` |
|---|---|---|---|---|---|
| `modelID & 0x0F00` | `0x0100` | `isHeadwear()` — Over-ear headphones (e.g., H3, H6, H6 Neo) | `[Observed]` | Per chipset nibble | Per chipset |
| `modelID & 0x0F00` | `0x0300` | `isSpeaker()` — Bluetooth speaker (e.g., SP75 `0x0308`) | `[Observed]` | `0xA001` QCY TLV | `11` (WuQi BLE) |
| `modelID & 0x0F00` | Other | TWS / OWS Earbuds | `[Observed]` | Per chipset nibble | Per chipset |
| `modelID & 0x00F0` | `0x0010` | `isQCCDevice()` — **Qualcomm QCC** *(blocks EQ when `0x09` Game Mode is ON)* | `[Observed]` | `0xA001` QCY TLV | `5` (GAIA RFCOMM) |
| `modelID & 0x00F0` | `0x0040` | `isWQDevice()` — **WuQi (物奇)** *(voice prompt prefix `"WQ"`)* | `[Observed]` | `0xA001` QCY TLV | `6` / `11` (WuQi BLE) |
| `modelID & 0x00F0` | `0x0050` | **Bluetrum (中科蓝讯)** *(voice prompt prefix `"LX"`; no helper method in `QCYDeviceBean`, confirmed via `0x4D57`–`0x4D5F` all using `otaType=10`)* | `[Observed]` / `[Inferred]` | `0xA001` QCY TLV | `10` (Bluetrum FOTA BLE) |
| `modelID & 0x00F0` (or `== 0x0200`) | `0x0070` / `0x0200` | `isJLDevice()` — **JieLi (杰理)** *(voice prompt prefix `"JL"`)* | `[Observed]` | `0xA002` RCSP | `2` (or `9`) (JieLi BLE) |
| `modelID & 0x8070` | `0x8070` | `isNewJL()` — **New JieLi** *(tunnels QCY TLVs inside RCSP `0xFF`; `0x06`/`0x09` OFF=`2`)* | `[Observed]` | `0xA002` RCSP + `0xFF` TLV | `2` (JieLi BLE) |
| `modelID & 0x00F0` | `0x0090` | `isZRDevice()` *(0 callers in app; `EQParamListBean.DEFAULT_FREQS_ZR` 5-band EQ)* | `[Observed]` / `[Unknown]` vendor | `0xA001` QCY TLV | `[Unknown]` |
| `modelID & 0x00F0` | `0x00A0` | `isRDDevice()` *(0 callers in app)* | `[Observed]` / `[Unknown]` vendor | `0xA001` QCY TLV | `[Unknown]` |
| `modelID & 0x00F0` | `0x00C0` | `isRTKDevice()` — Realtek *(0 callers; DFU SDK stripped)* | `[Observed]` | `0xA001` QCY TLV | N/A |
| `modelID & 0x00F0` | `0x00D0` | `isBESDevice()` — **BES (Bestechnic)** | `[Observed]` | `0xA001` QCY TLV | `8` (BES BLE) |
| `modelID & 0xFFF0` | `0x4BC0` | `isHQ3710()` *(blocks EQ unless ANC is OFF)* | `[Observed]` | `0xA001` QCY TLV | `[Unknown]` |
| `modelID == 0x4D17` | `0x4D17` | `isT11S()` — QCY T11S (Qualcomm QCC) | `[Observed]` | `0xA001` QCY TLV | `5` |

#### `isSupportMultiEQ()` Decision Rule (`QCYDeviceBean`) `[Observed]`
1. Check `supportFlag & 0x03` if populated.
2. Explicit allowlist: `modelID` in `{0x1972, 0x4173, 0x41D1}` -> `true`.
3. If `isJLDevice()` (legacy JL) -> `false` (note: `isNewJL` uses `DeviceConfig.eqModel.cmdID = 0x22`).
4. Otherwise -> `true` if legacy GATT characteristic `0x000B` is **absent** from service `0xA001`; `false` if `0x000B` is present.

#### Bundled Reference `modelID` Table (`ControlFuncViewModelKt` & `res/raw/function_*.json`) `[Observed]`

| `modelID` (Dec) | `modelID` (Hex) | Config Codename | Decoded Form / Chipset | Control Service | Bundled `otaType` |
|---|---|---|---|---|---|
| `776` | `0x0308` | `sp75` (SP75 RGB Speaker) | Speaker (`0x0300`) / WuQi | `0xA001` | `11` (WuQi BLE) |
| `16764` | `0x417C` | `h3` (H3 Over-Ear) | Headwear (`0x0100`) / Legacy JL (`0x0070`) | `0xA002` | `2` (JieLi BLE) |
| `19786` | `0x4D4A` | `ht08` (MeloBuds Pro) | Earbuds / WuQi (`0x0040`) | `0xA001` | `11` (WuQi BLE) |
| `19791` | `0x4D4F` | `ct02` | Earbuds / WuQi (`0x0040`) | `0xA001` | `11` (WuQi BLE) |
| `19799` | `0x4D57` | `c50ai` | Earbuds / Bluetrum (`0x0050`) | `0xA001` | `10` (Bluetrum BLE) |
| `19803` | `0x4D5B` | `c50sai` | Earbuds / Bluetrum (`0x0050`) | `0xA001` | `10` (Bluetrum BLE) |
| `19804` | `0x4D5C` | `c50s` | Earbuds / Bluetrum (`0x0050`) | `0xA001` | `10` (Bluetrum BLE) |
| `19806` | `0x4D5E` | `ct03` | Earbuds / Bluetrum (`0x0050`) | `0xA001` | `10` (Bluetrum BLE) |
| `19807` | `0x4D5F` | `ct11` | Earbuds / Bluetrum (`0x0050`) | `0xA001` | `10` (Bluetrum BLE) |
| `20593` | `0x5071` | `t13ancpro` | Earbuds / Legacy JL (`0x0070`) | `0xA002` | `2` (JieLi BLE) |
| `23872` | `0x5D40` | `n70` (MeloBuds N70) | Earbuds / WuQi (`0x0040`) | `0xA001` | `11` (WuQi BLE) |
| `23881` | `0x5D49` | `n70i` | Earbuds / WuQi (`0x0040`) | `0xA001` | `11` (WuQi BLE) |
| `23885` | `0x5D4D` | `ht27` | Earbuds / WuQi (`0x0040`) | `0xA001` | `11` (WuQi BLE) |
| `32884` | `0x8074` | `ht20` | Earbuds / New JL (`0x8070`) | `0xA002` + `0xFF` | `2` (JieLi BLE) |
| `32886` | `0x8076` | `ht22` (204) | Earbuds / New JL (`0x8070`) | `0xA002` + `0xFF` | `2` (JieLi BLE) |
| `32887` | `0x8077` | `ht23` | Earbuds / New JL (`0x8070`) | `0xA002` + `0xFF` | `2` (JieLi BLE) |
| `32889` | `0x8079` | `ht20s` | Earbuds / New JL (`0x8070`) | `0xA002` + `0xFF` | `2` (JieLi BLE) |
| `32890` | `0x807A` | `ct12` | Earbuds / New JL (`0x8070`) | `0xA002` + `0xFF` | `2` (JieLi BLE) |
| `32891` | `0x807B` | `ht21` / `ht27` | Earbuds / New JL (`0x8070`) | `0xA002` + `0xFF` | `2` (JieLi BLE) |
| `32893` | `0x807D` | `ht22` (208) | Earbuds / New JL (`0x8070`) | `0xA002` + `0xFF` | `2` (JieLi BLE) |
| `32894` | `0x807E` | `ht21` | Earbuds / New JL (`0x8070`) | `0xA002` + `0xFF` | `2` (JieLi BLE) |
| `33136` | `0x8170` | `h6` (H6 Over-Ear) | Headwear (`0x0100`) / New JL (`0x8070`) | `0xA002` + `0xFF` | `2` (JieLi BLE) |
| `33137` | `0x8171` | `h6_neo` (H6 Neo) | Headwear (`0x0100`) / New JL (`0x8070`) | `0xA002` + `0xFF` | `2` (JieLi BLE) |
| `36978` | `0x9072` | `ht08_jl` | Earbuds / New JL (`0x8070`) | `0xA002` + `0xFF` | `2` (JieLi BLE) |
| `36985` | `0x9079` | `t13anc3` | Earbuds / New JL (`0x8070`) | `0xA002` + `0xFF` | `2` (JieLi BLE) |

---

### 4.2 Master Command ID Table (All 82 `ConstantCMDID`s + 6 Config-Only Active CMDIDs)

Every constant from `com.qcymall.qcydevicelibrary.constant.ConstantCMDID` plus 6 additional active wire Command IDs discovered in `res/raw/function_*.json` (`0x33`, `0x53`, `0x57`, `0x5C`, `0x5D`, `0x63`).

> **Legend:** `R` = Readable via `0xFE 0x01 <Cmd>`, `W` = Writable by Host, `N` = Unsolicited Device Notification, `UI` = App UI sentinel only (never sent on wire). Scalar multi-byte RX values default to **unsigned little-endian** (`QCYDeviceManager$transferCallBack$1`).

| Hex | Dec | Constant / Config Key | Dir | Payload Format (TLV `Value` bytes) | Status | Notes |
|---|---|---|---|---|---|---|
| `0x01` | 1 | `CMDID_RESET_DEFAULT` (`resetModel`) | W | Empty (`VLen=0`) | `[Observed]` | Restore default settings (keeps pairing) |
| `0x02` | 2 | `CMDID_CLEAR_PAIR` (`clearPairModel`) | W | Empty (`VLen=0`) | `[Observed]` | Clear Bluetooth pairing records |
| `0x03` | 3 | `CMDID_FACTORY_RESET` (`factorySetModel`) | W | Empty (`VLen=0`) | `[Observed]` | Full factory reset |
| `0x04` | 4 | `CMDID_MUSICACTION` | R/W | TX: `1B` (`0x00` play/pause toggle) | `[Observed]` | Focus/WhiteNoise playback trigger (`WhiteNoiseVM`) |
| `0x05` | 5 | `CMDID_LIGHT` | R/W | `1B` bool (`0`/`1`) | `[Observed]` | Legacy indicator light toggle (no active app caller) |
| `0x06` | 6 | `CMDID_RUER` | R/W | `1B` bool (`1`=ON, `0`=OFF; **`2`=OFF on `isNewJL`**) | `[Observed]` | Simple In-Ear Detection toggle (`setInEarCheck`) |
| `0x07` | 7 | `CMDID_NOISE` | R/W | `1B` | `[Observed]` | Legacy ANC toggle (superseded by `0x0C`/`0x17`) |
| `0x08` | 8 | `CMDID_VOICE` | R/W/N | `3B`: `[vol, vol, flag]` (`VolumeDataBean`) | `[Observed]` | WhiteNoise/media volume (`WhiteNoiseVM` sends `[v, v, 0]`) |
| `0x09` | 9 | `CMDID_DIYANSHI` (`gameMode`) | R/W/N | `1B` bool (`1`=ON, `0`=OFF; **`2`=OFF on `isNewJL`**) | `[Observed]` | Low-Latency / Game Mode; blocks EQ on QCC |
| `0x0A` | 10 | `CMDID_JIANTING` | R/W | `1B` bool | `[Observed]` | Legacy Sidetone/Monitoring toggle (unused in v1.2.3 UI) |
| `0x0C` | 12 | `CMDID_NOISEMODE` | R/W/N | `1B`: ANC mode enum | `[Observed]` | Legacy single-byte ANC mode (`setNoiseMode`) |
| `0x0D` | 13 | `CMDID_TESTMODE` | W | `1B` | `[Observed]` | Factory/engineering test mode (no app UI caller) |
| `0x10` | 16 | `CMDID_SLEEPMODE` (`touchEnable`) | R/W/N | `1B` bool (`0`/`1`) | `[Observed]` | Sleep Mode / Touch Lock (`setSleepMode`; also `touchEnable` in `TouchBTNVM`) |
| `0x11` | 17 | `CMDID_COMPACTNESS` (`fitTest`) | W/N | TX: `3B` `[0x01, 0x01, 0x01]`; RX: `3B` `[status, L, R]` | `[Observed]` | Ear Tip Fit Test (`CompactnessDataBean`) |
| `0x12` | 18 | `CMDID_LEDMODE` | R/W | `1B` | `[Observed]` | Legacy LED mode enum (unused in v1.2.3 UI) |
| `0x14` | 20 | `CMDID_POWERMANAGER` (`autoPowerOff`) | R/W/N | TX: `4B` `[time_lo, time_hi, 0, 0]`; RX: `4B` `[powerTime_u16LE, curTime_u16LE]` | `[Observed]` | Scheduled Auto Power-Off (minutes; `0` or `0xFFFF` = Off) |
| `0x16` | 22 | `CMDID_BALANCE` (`balanceModel`) | R/W/N | `1B` `u8` (`0..100`, center = `50`) | `[Observed]` | L/R Audio Channel Balance (`setBalanceValue`) |
| `0x17` | 23 | `CMDID_ANCSETTING` (`ancModel`) | R/W/N | `3B`: `[mode, subScene, noiseValue]` | `[Observed]` | Modern ANC / Transparency / Adaptive / Custom control |
| `0x18` | 24 | `CMDID_PAIRNAME` (`deviceName`) | R/W/N | UTF-8 bytes (max ~24–30 B) | `[Observed]` | Custom Bluetooth pairing name (`PairnameDataBean`) |
| `0x19` | 25 | `CMDID_VOICENAME` (`promptModel`) | R/W/N | `4B` ASCII chipset+lang code (`"WQ00"`, `"JL01"`, `"LX00"`, ...) | `[Observed]` | Prompt voice pack/language selector (`SubItem.cmdHex`) |
| `0x1A` | 26 | `CMDID_VOICE_LANGUAGE` | R/W/N | `1B` language index | `[Observed]` | Legacy prompt language enum |
| `0x1B` | 27 | `CMDID_ATPX` | R/W/N | `1B` bool (`0`/`1`) | `[Observed]` | aptX codec toggle (spelling `ATPX` in SDK) |
| `0x1D` | 29 | `CMDID_TONEVOLUME` (`promptVolume`) | R/W/N | TX: `1B` level (`0x04`/`0x06`/`0x08`/`0x0A`); RX: `2B` `[curVol, maxVol]` | `[Observed]` | Prompt tone volume (`VolumeDataBean`) |
| `0x1E` | 30 | `CMDID_TAKEPHOTO` | N | `1B` | `[Observed]` | Remote camera shutter gesture notification |
| `0x1F` | 31 | `CMDID_STANDBY` (`disconnectPowerOff`) | R/W/N | `2B` `u16_LE` minutes (`0xFFFF` = Never) | `[Observed]` | Disconnect auto power-off (`setDisconnectPower`) |
| `0x20` | 32 | `CMDID_MULTIEQ` | R/W/N | `[eqType(1B), masterGain*100(i16_LE), N×6B(freq, gain*100, Q*100)]` | `[Observed]` | Parametric Multi-EQ v1 (6B/band, no filter type) |
| `0x22` | 34 | `CMDID_MULTIEQ2` (`eqModel`) | R/W/N | `[eqType(1B), masterGain*100(i16_LE), N×7B(freq, gain*100, Q*100, irrType)]` | `[Observed]` | Parametric Multi-EQ v2 (7B/band, 9 biquad filter types) |
| `0x23` | 35 | `CMDID_LDAC` (`ldacModel`) | R/W/N | `1B` bool (`0`/`1`) | `[Observed]` | LDAC High-Res codec toggle (mutually exclusive w/ `0x24` on most models; triggers reboot) |
| `0x24` | 36 | `CMDID_1T2` (`dualConnect`) | R/W/N | `1B` bool (`0`/`1`) | `[Observed]` | Dual-device multipoint toggle |
| `0x27` | 39 | `CMDID_AEQ` (`adaptEQ`) | R/W/N | `1B` bool (`0`/`1`) | `[Observed]` | Adaptive / Dynamic EQ toggle |
| `0x28` | 40 | `CMDID_ANC_RESULT` | N | `1B`: `2`=calibrating, `1`=calibration complete | `[Observed]` | Adaptive ANC real-time calibration status |
| `0x29` | 41 | `CMDID_ANC_WEAR` | R/N | `2B` or `4B` packed `u16` | `[Observed]` | ANC wear/seal status (`(b2) \| (b3<<8)` or `((b4+3)&0xFF) \| ((b5+3)<<8)`) |
| `0x2A` | 42 | `CMDID_WIND_NOISE` (`windNoise`) | R/W/N | `1B` bool (`0`/`1`) | `[Observed]` | Dedicated Wind Noise Reduction toggle |
| `0x2B` | 43 | `CMDID_BUTTONFUNC` (`buttonModel`) | R/W/N | TX: `[eventID, actionCmd]` (reset all: `[0xFF, 0xFF]`); RX: `N×2B` pairs | `[Observed]` | Touch/button remapping (legacy fallback: GATT `0x000D`) |
| `0x2C` | 44 | `CMDID_WEAR_CONFIG` (`inEarCheck`) | R/W/N | `3B`: `[useWear(0/1), controlMusic(0..3), controlANC(0/1)]` | `[Observed]` | Advanced In-Ear Wear Detection (`WearConfigDataBean`) |
| `0x2D` | 45 | `CMDID_SPACE_SOUND` (`spaceAudio`) | R/W/N | `1B` bool (`0`/`1`) | `[Observed]` | Simple Spatial Sound / 3D Audio toggle |
| `0x2E` | 46 | `CMDID_MUSIC_MODE` (`musicMode`) | R/W/N | `1B`: `1`=Music, `3`=Game, `4`=Movie | `[Observed]` | Listening / Scene Sound Mode |
| `0x2F` | 47 | `CMDID_BATTERY` | R/N | `3B`: `[L, R, Box]` (bit7=charging, bits0..6=0..100%) | `[Observed]` | Battery levels (legacy fallback: GATT `0x0008`) |
| `0x30` | 48 | `CMDID_VERSION` (`upgrade`) | R/N | `3B`: `[major, minor, patch]` -> `"%d.%d.%d"` | `[Observed]` | Firmware version (legacy fallback: GATT `0x0007`) |
| `0x31` | 49 | `CMDID_TOUCH_TONE` | R/W/N | `1B` bool | `[Observed]` | Touch feedback tone toggle |
| `0x32` | 50 | `CMDID_ENV_ADAPTATION` | R/W/N | `1B` bool | `[Observed]` | Ambient/Environment Adaptation toggle |
| `0x33` | 51 | *(Config only)* `touchSensitivity` | R/W/N | `1B` threshold: `0x26`(38)=Mod, `0x2D`(45)=Norm, `0x34`(52)=Sens | `[Observed]` | Touch sensor sensitivity (`res/raw/function_*.json`) |
| `0x34` | 52 | `CMDID_TWS_ENABLE` (`twsConnect`) | R/W/N | `1B` bool (`0`/`1`) | `[Observed]` | Speaker TWS stereo pairing enable (SP75) |
| `0x35` | 53 | `CMDID_LED_SWITCH` | R/W/N | `1B` bool (`0`/`1`) | `[Observed]` | Speaker RGB LED master power switch |
| `0x36` | 54 | `CMDID_LED_EFFECT` (`lightingEffect`) | R/W/N | `[speed(1B), bright(1B), effectIdx(1B), N×3B(R,G,B)]` | `[Observed]` | Speaker RGB lighting effect (`LEDEffectDataBean`) |
| `0x37` | 55 | `CMDID_PLAY_MODE` | R/W/N | `1B` mode | `[Observed]` | Local playback loop/shuffle mode |
| `0x39` | 57 | `CMDID_FOUCUS_MODE` (`whiteNoise`) | R/W/N | `1B` bool or track control | `[Observed]` | Focus Mode / White Noise switch (`FOUCUS` typo in SDK) |
| `0x3A` | 58 | `CMDID_MUSIC_STATE` | R/N | `6B`: `[musicIndex(u32_LE), status(1B), playMode(1B)]` | `[Observed]` | Hardware white-noise / local playback state (`MusicStatusBean`) |
| `0x3B` | 59 | `CMDID_MUSIC_INFO` | R/N | `[startToneID(1B), N×6B(musicID_u32LE, total_u16LE)]` | `[Observed]` | Stored white-noise track table (`MusicFileBean`) |
| `0x3D` | 61 | `CMDID_TONE_PLAY` | W | `1B`: `1`=play preview tone, `0`=stop | `[Observed]` | Device prompt/white-noise preview playback |
| `0x3E` | 62 | `CMDID_SYNC_TIME` | W | `7B`: `[YY, MM, DD, hh, mm, ss, weekday_mask]` | `[Observed]` | Host wall-clock sync (`QCYDeviceImpl.syncDayTime`) |
| `0x3F` | 63 | `CMDID_ALARM` | R/W/N | TX: `7B` Add/Del/Edit; RX: `[hdr(1B), N×6B]` | `[Observed]` | Hardware alarm clock (`AlarmDataBean`) |
| `0x40` | 64 | `CMDID_ADAPT_VOLUME` (`autoVolume`) | R/W/N | `1B` bool (`0`/`1`) | `[Observed]` | Adaptive Volume toggle |
| `0x42` | 66 | `CMDID_DEVICE_COLOR` (`deviceColor`) | R/W/N | `1B` color index | `[Observed]` | Earbud colorway ID (`ColorSelectActivity`) |
| `0x43` | 67 | `CMDID_AI` (`aiModel`) | R/W/N | `1B` | `[Observed]` | AI voice/assistant feature flag |
| `0x44` | 68 | `CMDID_EQ_COUNT` | R/N | `1B` `u8` (e.g., `10` or `20`; default `10`, min `5`) | `[Observed]` | Supported parametric EQ band count |
| `0x45` | 69 | `CMDID_CUSTOM_EQ_TEST` | W | `1B`: `1`=enter test mode, `0`=exit | `[Observed]` | Personalized Hearing Test pure-tone mode |
| `0x46` | 70 | `CMDID_MULTIEQ_LEFT` | W | Same 7B/band format as `0x22` (`setEQWtihCID`) | `[Observed]` | Independent Left-ear parametric EQ |
| `0x47` | 71 | `CMDID_MULTIEQ_RIGHT` | W | Same 7B/band format as `0x22` (`setEQWtihCID`) | `[Observed]` | Independent Right-ear parametric EQ |
| `0x48` | 72 | `CMDID_INEAR_SENSITIVITY` | R/W/N | `1B`: `1`=Moderate, `2`=Normal, `3`=Sensitive | `[Observed]` | Wear detection sensitivity (`WearConfigDataBean.sensitivity`) |
| `0x4A` | 74 | `CMDID_GAME_CONFIG` (`gameModePro`) | R/W/N | `2B`: `[enabled(0/1), followGameSound(0/1)]` | `[Observed]` | Game Mode Pro (low-latency + auto game EQ) |
| `0x4B` | 75 | `CMDID_FINDEARPHONE` (`findEar`) | UI | *Not sent over BLE* | `[Observed]` | UI routing sentinel (`ItemViewGridItem` -> `/earphones/findearphones`) |
| `0x4D` | 77 | `CMDID_PHONE_INFO` (`multiConnect`) | R/N | `[count(1B), entries...]` (`PhoneBeanList`) | `[Observed]` | Paired/connected host device list (Multipoint manager) |
| `0x4F` | 79 | `CMDID_PHONE_STATUS` | W/N | TX: `[action(1=conn,2=del,3=disc), mac_6B]`; RX: `[mac_6B, status(1B)]` | `[Observed]` | Multipoint host control & list upload framing (`10`=start, `11`=end) |
| `0x50` | 80 | `CMDID_HEAD_ACTION` (`headControl`) | R/W/N | `1B` bool (`0`/`1`) | `[Observed]` | IMU Head-Gesture Control master switch |
| `0x51` | 81 | `CMDID_SENSOR_CONFIG` | R/W/N | TX: `[eventID, actionCmd]`; RX: `N×2B` pairs | `[Observed]` | Head-gesture action mapping (nod/shake/tilt L/R) |
| `0x52` | 82 | `CMDID_SENSOR_TEST` | W/N | TX: `1B` (`0xFA`=start IMU test, `0xFB`=stop); RX: `1B` `eventID` | `[Observed]` | Live head-gesture practice/test stream |
| `0x53` | 83 | *(Config only)* `powerSavingMode` | R/W/N | `1B` bool (`0`/`1`) | `[Observed]` | Power Saving Mode (`res/raw/function_*.json`) |
| `0x54` | 84 | `CMDID_AI_ANC` (`aiANC`) | R/W/N | TX `6B`: ON `[0,0,0,0,0,1]`, OFF `[4,0,0,0,0,1]`; RX `5–6B` | `[Observed]` | AI Scene ANC state (`AIANCSettingDataBean`) |
| `0x55` | 85 | `CMDID_AI_ANC_CFG` | R/N | `45B` (`9 × 5B` matrix entries) | `[Observed]` | AI Scene ANC factory default 3×3 matrix (`AIANCConfigDataBean`) |
| `0x56` | 86 | `CMDID_AI_ANC_CFG2` | R/W/N | TX `5B`: `[activity, noiseScene, mode, subScene, level]`; RX `45B` | `[Observed]` | AI Scene ANC user-customized 3×3 matrix |
| `0x57` | 87 | *(Config only)* `bassEnhancement` | R/W/N | `1B` bool (`0`/`1`) | `[Observed]` | Virtual Bass enhancement (`res/raw/function_*.json`) |
| `0x58` | 88 | `CMDID_HEAR_HEALTH` (`hearingSafety`) | R/W/N | `4B`: `[alarmVol(0/1), alarmTime(0/1), time_u16LE]` | `[Observed]` | Hearing Health reminders (`HearingDataBean`) |
| `0x5A` | 90 | `CMDID_AI_ANC_PREVIEW` | W | `4B`: Enter `[1,0,0,0]`, Preview `[2,m,s,l]`, Exit `[3,0,0,0]` | `[Observed]` | Real-time ANC preview during AI-ANC editing |
| `0x5B` | 91 | `CMDID_SPACE_AUDIO_MODE` (`spatialAudio`) | R/W/N | `1B`: `1`=Off, `2`=Fixed, `3`=Head Tracking | `[Observed]` | 3-Mode Spatial Audio selector |
| `0x5C` | 92 | *(Config only)* `voiceDetection` | R/W/N | `1B` timeout sec (`0x00`=Off, `0x05`, `0x0A`, `0x0F`, `0x14`) | `[Observed]` | Speak-to-Chat resume delay (`res/raw/function_*.json`) |
| `0x5D` | 93 | *(Config only)* `disconnectWarn` | R/W/N | `1B` bool (`0`/`1`) | `[Observed]` | Disconnect alert tone toggle (`res/raw/function_*.json`) |
| `0x5F` | 95 | `CMDID_PRIVACY_MODE` (`privacyModePro`) | R/W/N | `2B`: `[enabled(0/1), level(0=Call+Music, 1=Call only)]` | `[Observed]` | Privacy Mode Pro anti-leakage |
| `0x60` | 96 | `CMDID_LEA` (`leAudio`) | R/W/N | `1B` bool (`1`=enable in `LEATipActivity`) | `[Observed]` | Experimental LE Audio toggle |
| `0x62` | 98 | `CMDID_WEAR_STATUS` | R/N | `2B` packed `(hi << 8) \| lo` (`u16_LE`) | `[Observed]` | Real-time L/R wearing status (paired as `otherCmdID` of `ancModel`) |
| `0x63` | 99 | *(Config only)* `thxEq` | R/W/N | `1B` bool (`0`/`1`) | `[Observed]` | THX Certified EQ toggle (`res/raw/function_*.json`) |
| `0x64` | 100 | `CMDID_TALK_CHECK` (`dialoguePerception`) | R/W/N | `1B` bool (`0`/`1`) | `[Observed]` | Dialogue Perception / Conversation Awareness |
| `0x65` | 101 | `CMDID_BTN_ANC_CFG` (`btnAncCfg`) | R/W/N | `N×3B` `[mode, scene, level]` (reset: `[0xFF, 0xFF, 0xFF]`) | `[Observed]` | Custom ANC button-cycle list (`BtnANCConfigDataBean`) |
| `0x66` | 102 | `CMDID_MULTI_PAIR` | W | `1B`: `1`=enter multipoint pairing, `0`=exit | `[Observed]` | Trigger discoverable pairing for 2nd host device |
| `0xF6` | 246 | `CMDID_MORE_SETTING` | UI | *Not sent over BLE* | `[Observed]` | UI grouping constant |
| `0xF7` | 247 | `CMDID_DEVICE_INFO` | UI | *Not sent over BLE* | `[Observed]` | UI grouping constant |
| `0xF8` | 248 | `CMDID_THEME_SET` | UI | *Not sent over BLE* | `[Observed]` | UI grouping constant |
| `0xFC` | 252 | `CMDID_GAIN_EQ` | R/W | Virtual ID -> routes to GATT `0x000B` or JL RCSP EQ | `[Observed]` | Legacy 10-band / 5-band integer-gain EQ |
| `0xFE` | 254 | `CMDID_REQUESTDATA` | W | `1B`: `[TargetCmdID]` | `[Observed]` | Universal attribute read request |


---

### 4.3 Feature-by-Feature Wire Specifications

Unless otherwise stated, all byte offsets below (`V[0]`, `V[1]`, …) denote the **Value bytes inside a single TLV item `[CmdID, VLen, V[0..VLen-1]]`**, wrapped in the outer unified frame `[0xFF, TotalLen, CmdID, VLen, V...]` (Section 3.1). On `isNewJL` devices (`0x8070`), the exact same `[0xFF, ...]` payload is carried inside JieLi RCSP `CMD_EXTRA_CUSTOM` (`0xFF`) over GATT `0xA002`.

#### 4.3.1 Connection Handshake, Device Info & Version Query

* **Daily Control Handshake (`[Observed]`):** **None.** Neither `QCYDeviceImpl` (`0xA001`) nor `JLDeviceImpl` (`0xA002`) performs cryptographic challenge-response authentication for daily control. After MTU negotiation (`512` bytes) and enabling notifications (`CCCD 0x2902`), the client immediately begins querying device attributes via `0xFE 0x01 <TargetCmdID>` spaced at `100 ms` intervals (`CommandProcessor`).
* **Initial State Hydration Sequence (`QCYDeviceManager.getDeviceInfo` / `ControlFuncViewModel`):**
  1. Firmware version: `0x30` (`CMDID_VERSION`) via `0xFE 0x01 0x30`, or legacy GATT direct read of `00000007-0000-1000-8000-00805F9B34FB` (`QCYDeviceImpl.readVersion`).
     * **RX `V[0..2]` (`3B`):** `[major, minor, patch]` unsigned bytes, formatted as `"%d.%d.%d"` (e.g., `[0x01, 0x02, 0x05]` -> `"1.2.5"`).
     * On Legacy JieLi (`!isNewJL`), queried via RCSP `CMD_GET_TARGET_INFO` (`0x03`, `DeviceInfo.getVersionName()`).
  2. Battery levels: `0x2F` (`CMDID_BATTERY`) or legacy GATT read of `00000008-...`.
  3. Active model profile attributes: the client iterates the `cmdID`s enabled in the device's model config (ANC `0x17`, EQ `0x20`/`0x22`/`0xFC`, Buttons `0x2B`, EQ band count `0x44`, In-Ear `0x06`/`0x2C`, Game Mode `0x09`/`0x4A`, Dual-Device `0x24`/`0x4D`, LDAC `0x23`, etc.) and requests each via `0xFE 0x01 <CmdID>`.
* **Device Identity Fields (`[Observed]` / `[Unknown]`):**
  * `modelID` (`u16` Big-Endian): read from BLE Manufacturer Data bytes `[0..1]` (Section 2.2), never queried over GATT.
  * `colorIndex` (`u8`): `0x42` (`CMDID_COLOR_INDEX`) or BLE Manufacturer Data byte `[3]` bits `1..3` (`(b[3] >> 1) & 0x07`).
  * **Hardware version & Serial number:** **Not present** in the QCY TLV command table or legacy GATT characteristics (`[Observed]` absence in `ConstantCMDID` and `QCYDeviceImpl`).

---

#### 4.3.2 Battery Reporting (`0x2F`, Legacy GATT `0x0008`, BLE Adv `[5..7]`, JL RCSP)

* **Wire Format (`[Observed]`, `BatteryDataBean`, `QCYDeviceImpl`, `QCYDeviceManager$scanCallback$1`):**
  * **RX `V[0..2]` (`3B`):** `[leftByte, rightByte, caseByte]` (also present verbatim at BLE Manufacturer Data offsets `[5, 6, 7]` — enabling zero-connection passive battery & lid-open popup updates on Windows).
  * **Bitfield per byte:**
    * `bit 7` (`byte & 0x80 != 0`): Charging flag (`1` = charging, `0` = discharging).
    * `bits 0..6` (`byte & 0x7F`): Battery percentage `0..100`. Values `0` or `127` (`0x7F`) indicate bud absent/disconnected or case lid closed / unreachable.
  * For single-unit devices (over-ear headphones `0x0100`, speakers `0x0300`, neckbands), `V[0]` (or `V[0]` mirrored with `V[1]`) reports the main battery while `caseByte` is `0x00`/`0x7F`.
* **Legacy JieLi (`!isNewJL`):** `JLDeviceImpl` reads `ADVInfoResponse` via RCSP `0xC0`/`0xC2` (`leftDeviceQuantity`, `rightDeviceQuantity`, `chargingBinQuantity` + charging booleans).

---

#### 4.3.3 ANC, Transparency, Wind Noise, Adaptive ANC, AI-ANC & Custom Button Cycle

1. **Primary ANC & Transparency Control — `0x17` (`CMDID_ANCSETTING`) (`[Observed]`, `ANCDataBean`, `QCYDeviceImpl.setANCMode`, `res/raw/function_*.json`):**
   * **TX & RX `V[0..2]` (`3B`):** `[mode, subScene, noiseValue]`

   | `mode` (`V[0]`) | `subScene` (`V[1]`) | `noiseValue` (`V[2]`) | Meaning & Model Notes |
   |---|---|---|---|
   | `0x01` | `0x01` | `0..2` | **ANC — Scene 1** (Indoor / Low / Outdoor depending on model JSON `ancModel.ancList`) |
   | `0x01` | `0x02` | `0..2` | **ANC — Scene 2** (Commute / Mid / Balanced) |
   | `0x01` | `0x03` | `0..2` | **ANC — Scene 3** (Outdoor / Noisy / High / Deep) |
   | `0x01` | `0x04` | `0x00` | **ANC — Anti-Wind Noise** (sub-mode of ANC on modern chips) |
   | `0x01` | `0x05` | `0x00` | **Adaptive ANC** (triggers calibration status updates on `0x28`) |
   | `0x02` | `0x00` | `0x00` | **Normal / Off** (ANC & Transparency disabled) |
   | `0x03` | `0x01` | `0` or `1..6` | **Transparency Mode** (`0` on fixed-gain models; `1..6` on 6-level adjustable transparency models) |
   | `0x03` | `0x02` | `0x00` | **Vocal Boost Transparency** (encoded in JSON `ancModel.voiceCmdId = 197120 = 0x030200`) |
   | `0x05` | `0x00` | `0..15` | **Custom ANC Depth Slider** (16 discrete steps `0x00..0x0F`, used on flagship adaptive models e.g. MeloBuds N70 / HT10 / H3 Pro) |

2. **Legacy ANC Commands (`[Observed]`):**
   * `0x0C` (`CMDID_NOISEMODE`): 1-byte legacy mode enum (`QCYDeviceImpl.setNoiseMode`). On Legacy JieLi (`!isNewJL`), `JLDeviceImpl.setNoiseMode` uses RCSP `setCurrentNoiseMode(NoiseMode)` with gain scaled linearly as `minGain + (maxGain - minGain) * percent / 100`.
   * `0x2A` (`CMDID_WIND_NOISE`): Dedicated 1-byte boolean (`0x00`=Off, `0x01`=On) on models that expose wind-noise reduction independently of `0x17` `subScene=4`.
   * `0x28` (`CMDID_ANC_RESULT`): RX `1B` notification during Adaptive ANC (`0x02`=calibrating, `0x01`=calibration complete).
   * `0x29` (`CMDID_ANC_WEAR`) & `0x62` (`CMDID_WEAR_STATUS`): Real-time seal / wear detection bitmask used alongside Adaptive ANC (`ancModel.otherCmdID = [98]`).
     * `0x29` RX (`QCYDeviceManager$transferCallBack$1`): if raw TLV length `>= 6` (4B payload), decoded as `((tlv[4] + 3) & 0xFF) | (((tlv[5] + 3) & 0xFF) << 8)`; otherwise `u16_LE(tlv[2..3])`.
     * `0x62` RX: `u16_LE` wear bitmask.

3. **AI-ANC (Adaptive Activity + Noise Environment Matrix) — `0x54`, `0x55`, `0x56`, `0x5A` (`[Observed]`, `AIANCSettingDataBean`, `AIANCConfigDataBean`):**
   * **`0x54` (`CMDID_AI_ANC`):**
     * **TX (`6B`):** Enable AI-ANC `[0x00, 0x00, 0x00, 0x00, 0x00, 0x01]`; Disable AI-ANC `[0x04, 0x00, 0x00, 0x00, 0x00, 0x01]`.
     * **RX (`5B` or `6B`):** `[status, noiseScene, mode, subScene, noiseValue, (optional) touchIndex]`
       * `status`: `0`=Detecting, `1`=Stay/Static, `2`=Walk, `3`=Run, `4`=Off.
       * `noiseScene`: Detected ambient noise tier (`1..3` `[Inferred]` from 9-entry 3×3 matrix).
       * `mode`, `subScene`, `noiseValue`: Currently applied `0x17` ANC tuple.
   * **`0x55` (`CMDID_AI_ANC_CFG`, factory default matrix) & `0x56` (`CMDID_AI_ANC_CFG2`, user matrix):**
     * **RX (`45B`):** 9 packed 5-byte entries (`3 activity states × 3 noise scenes`), each `[status, noiseScene, mode, subScene, noiseValue]`.
     * **TX on `0x56` (`5B`):** Update a single matrix cell `[status, noiseScene, mode, subScene, noiseValue]`.
   * **`0x5A` (`CMDID_AI_ANC_PREVIEW`):**
     * **TX (`4B`):** Enter preview `[0x01, 0x00, 0x00, 0x00]`; Audition specific tuple `[0x02, mode, subScene, noiseValue]`; Exit preview `[0x03, 0x00, 0x00, 0x00]`.

4. **Custom Button ANC Cycle List — `0x65` (`CMDID_BTN_ANC_CFG`) (`[Observed]`):**
   * **TX / RX (`N × 3B`):** Concatenated `[mode, subScene, noiseValue]` tuples defining the exact order cycled when the user triggers Action `0x0B` (ANC Cycle).
   * **Reset to default cycle:** TX `[0xFF, 0xFF, 0xFF]`.

---

#### 4.3.4 Equalizer (4 Wire Dialects, 9 Biquad Filter Types & Hardware Quirks)

QCY devices use **four distinct EQ wire dialects** routed by `modelID` (`QCYDeviceBean.isSupportMultiEQ()`, Section 4.1) and `EQParamBean.irrType`:

1. **Dialect A — Legacy Fixed-Band Gain EQ (`0xFC` / GATT `0x000B` / Legacy JL RCSP `0x07`/`0x08`) (`[Observed]`, `EQDataBean`, `QCYDeviceImpl.setEQ`, `JLDeviceImpl.setEQ`):**
   * **Transport:** Direct BLE write/read to legacy characteristic `0000000B-0000-1000-8000-00805F9B34FB` on `0xA001` (not wrapped in `0xFF`!), OR RCSP `SYS_INFO` attr `0x04` (`EqInfo`) on Legacy JieLi (`!isNewJL`).
   * **Payload (`1 + N` bytes):** `[eqType (u8), gain[0] (i8), ..., gain[N-1] (i8)]`
     * `gain[i]` is a signed 8-bit integer (`int8`) in dB (typically `-12..+12` or `-6..+6` dB depending on model JSON `eqModel`).
     * Standard 10-band center frequencies: `[31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]` Hz.
     * `ZR` chipset (`0x0090`) 5-band frequencies: `[31, 125, 500, 2000, 8000]` Hz.
   * **Extended RX (`13B`):** `[eqType, gain[0..9], gameEQType, currentMode]` (`EQDataBean.setReceiveData`).

2. **Dialect B — Parametric Multi-EQ v1 (`0x20` `CMDID_MULTIEQ`) (`[Observed]`, `QCYDeviceImpl.setEQWtihQFG` when `irrType >= 256`, default `0x100`):**
   * **Full Frame (`7 + 6*N` bytes):**
     ```text
     [0xFF] [5 + 6*N] [0x20] [3 + 6*N] [eqType: u8] [masterGain*100: i16_LE]
       Repeat N times (6 bytes per band):
         [freq_Hz: u16_LE] [gain_dB*100: i16_LE] [Q*100: i16_LE]
     ```
   * Example: `1000 Hz`, `+3.50 dB`, `Q = 1.41` -> `freq=0xE8 0x03`, `gain=0x5E 0x01` (`350`), `Q=0x8D 0x00` (`141`).

3. **Dialect C — Parametric Multi-EQ v2 with Per-Band IIR Filter Type (`0x22` `CMDID_MULTIEQ2`) (`[Observed]`, `QCYDeviceImpl.setEQWtihQFG` when `irrType < 256`, `EQParamBean`):**
   * **Full Frame (`7 + 7*N` bytes):**
     ```text
     [0xFF] [5 + 7*N] [0x22] [3 + 7*N] [eqType: u8] [masterGain*100: i16_LE]
       Repeat N times (7 bytes per band):
         [freq_Hz: u16_LE] [gain_dB*100: i16_LE] [Q*100: i16_LE] [irrType: u8]
     ```
   * **Biquad Filter Type Table (`EQParamBean.irrType`, `[Observed]`):**

     | `irrType` | Filter Name | Standard Audio DSP Type |
     |---|---|---|
     | `0` | `HP` | High-Pass |
     | `1` | `LP` | Low-Pass |
     | `2` | `PEAK` | Peaking / Bell (default parametric band) |
     | `3` | `HS` | High-Shelf |
     | `4` | `LS` | Low-Shelf |
     | `5` | `HSQ` | High-Shelf with Q |
     | `6` | `LSQ` | Low-Shelf with Q |
     | `7` | `ADV_HP` | Advanced High-Pass |
     | `8` | `ADV_LP` | Advanced Low-Pass |
     | `0xFF` | `PAD` | Unused padding slot (ignored by firmware) |

   * **RX Sentinel Quirk (`[Observed]`):** Firmware sometimes returns dummy placeholder bands `(freq=100, gain=0.0, Q=0.01)`; the app skips these when hydrating custom EQ curves.

4. **Dialect D — Independent Left / Right Parametric EQ (`0x46` `CMDID_MULTIEQ_LEFT`, `0x47` `CMDID_MULTIEQ_RIGHT`) (`[Observed]`, `QCYDeviceImpl.setEQWtihCID`):**
   * Uses the **exact same 7-byte-per-band payload** as `0x22` (`[eqType: u8, masterGain*100: i16_LE, N × 7B]`).
   * Padded with empty slots `(freq=0, gain=0.0, Q=0.0, irrType=0xFF)` up to `min((mtu - 7) / 6, maxBands)` so total band count matches device slot expectations (typically used for Personalized Hearing Compensation EQ generated after `0x45`).

5. **Companion Audio / EQ Commands (`[Observed]`):**
   * `0x44` (`CMDID_EQ_COUNT`): RX `1B` `u8` reporting supported parametric band count (commonly `10` or `20`; fallback default `10`, clamped min `5`).
   * `0x45` (`CMDID_CUSTOM_EQ_TEST`): TX `1B` (`0x01`=Enter personalized hearing-test tone mode, `0x00`=Exit). Pure audiogram tones are played locally via Android audio while `0x45=1` places buds into flat reference mode, after which compensated L/R curves are written to `0x46`/`0x47`.
   * `0x27` (`CMDID_AEQ`): `1B` bool — Dynamic / Adaptive Equalizer.
   * `0x57` (`bassEnhancement`, config-only): `1B` bool — Virtual Bass enhancement.
   * `0x63` (`thxEq`, config-only): `1B` bool — THX Spatial / Certified EQ toggle.
   * `0x40` (`CMDID_ADAPT_VOLUME`): `1B` bool — Adaptive Volume / Loudness normalization.
   * `0x16` (`CMDID_BALANCE`): `1B` `u8` in `0..100` (`50` = center balance; `<50` favors Left, `>50` favors Right).

6. **Observed Firmware / Mutual-Exclusion Quirks (`[Observed]`):**
   * `isQCCDevice()` (`(modelID & 0x00F0) == 0x0010`): EQ adjustments are blocked while Game Mode (`0x09`) is enabled.
   * `isHQ3710()` (`(modelID & 0xFFF0) == 0x4BC0`): EQ adjustments are blocked unless ANC is OFF (`mode == 2`).
   * When `musicMode` (`0x2E`) is set to `3` (Game) or `4` (Movie), custom EQ is bypassed/locked.

---

#### 4.3.5 Touch, Physical Button & Gesture Remapping (`0x2B`, GATT `0x000D`, JL RCSP)

* **Standard TLV (`0xA001` & `isNewJL` via `0x2B` `CMDID_BUTTONFUNC`) (`[Observed]`):**
  * **TX (Set single binding, `2B`):** `[eventID (u8), actionCmd (u8)]`.
  * **TX (Factory reset all buttons, `2B`):** `[0xFF, 0xFF]`.
  * **RX (Read all bindings, `N × 2B`):** `[event_0, action_0, event_1, action_1, ...]`.
* **Legacy GATT (`0xA001` char `0000000D-...`):** Raw 2-byte `[eventID, actionCmd]` write / `N × 2B` read without `0xFF` wrapper.
* **Legacy JieLi (`!isNewJL`, `JLDeviceImpl.setButtonFunc`):**
  * Uses RCSP `modifyAdvInfo` (attr type `2`) with 3-byte key tuple `[keyIdx, gestureAction, changeQCY2JL(actionCmd)]`.
  * **`changeQCY2JL` / `changeJL2QCY` Translation (`[Observed]`):** Action ID `0x00` ("None / Disabled") is translated to `0x7F` on legacy JieLi RCSP (`if (action == 0) 0x7F else action`), and `0x7F` is mapped back to `0x00` on RX! All other action IDs pass through 1:1.
  * **Event ID Splitting (`[Observed]`):**
    * TWS events `1..10`: odd = Left (`keyIdx=1`), even = Right (`keyIdx=2`), `gestureAction = ((eventID - 1) / 2) + 1`.
    * Over-ear 16-bit events (`0x0301`, `0x0501`, `0x0701..0x0704`): `keyIdx = eventID >> 8` (`3`=Vol-, `5`=Vol+, `7`=MFB), `gestureAction = eventID & 0xFF` (`1`..`4`).

* **Verified Button Event ID Table (`[Observed]` from `JLDeviceImpl` & `res/raw/function_*.json`):**

  | `eventID` (Dec / Hex) | Form Factor | Physical Gesture |
  |---|---|---|
  | `1` (`0x01`) / `2` (`0x02`) | TWS / Open-Ear | Left / Right Single Tap |
  | `3` (`0x03`) / `4` (`0x04`) | TWS / Open-Ear | Left / Right Double Tap |
  | `5` (`0x05`) / `6` (`0x06`) | TWS / Open-Ear | Left / Right Triple Tap |
  | `7` (`0x07`) / `8` (`0x08`) | TWS / Open-Ear | Left / Right Quadruple Tap |
  | `9` (`0x09`) / `10` (`0x0A`) | TWS / Open-Ear | Left / Right Long Press |
  | `17` (`0x11`) / `18` (`0x12`) | Over-Ear / Speaker | Vol- Single Press / Vol+ Single Press |
  | `19..22` (`0x13..0x16`) | Over-Ear / Speaker | MFB Single / Double / Triple / Quad Press |
  | `35..37` (`0x23..0x25`) | Over-Ear (New TLV) | Alternate MFB / ANC-Button Event Slots |
  | `0x0301` / `0x0501` | Legacy JL Over-Ear | Vol- Single / Vol+ Single |
  | `0x0701..0x0704` | Legacy JL Over-Ear | MFB Single / Double / Triple / Quad |

* **Verified Button Action ID Table (`actionCmd`, `[Observed]` from `res/raw/function_*.json`):**

  | `actionCmd` | Meaning | `actionCmd` | Meaning |
  |---|---|---|---|
  | `0` (`0x00`) | None / Disabled (`0x7F` on Legacy JL) | `11` (`0x0B`) | ANC Mode Cycle (ordered by `0x65` if supported) |
  | `1` (`0x01`) | Play / Pause | `14` (`0x0E`) | Mode Switch (generic) |
  | `2` (`0x02`) | Previous Track | `15` (`0x0F`) | Spatial Sound Toggle |
  | `3` (`0x03`) | Next Track | `16` (`0x10`) | Movie Mode |
  | `4` (`0x04`) | Voice Assistant | `17` (`0x11`) | Music Mode |
  | `5` (`0x05`) | Volume Up | `18` (`0x12`) | Game Mode (alternate enum) |
  | `6` (`0x06`) | Volume Down | | |
  | `7` (`0x07`) | Game / Low-Latency Mode | | |

* **Related Touch Controls (`[Observed]`):**
  * `0x10` (`CMDID_SLEEPMODE` / `touchEnable`): `1B` bool — disables touch inputs (Sleep / Accidental-Touch Lock).
  * `0x33` (`touchSensitivity`, config-only): `1B` enum — `0x26` (`38`)=Moderate, `0x2D` (`45`)=Normal, `0x34` (`52`)=Sensitive.
  * `0x31` (`CMDID_TOUCH_TONE`): `1B` bool — tactile tap beep feedback.

---

#### 4.3.6 Audio Modes, Codecs & Feature Toggles

| CmdID | Name | Payload (`V`) | Notes & Quirks |
|---|---|---|---|
| `0x09` | Game / Low-Latency Mode | `1B` (`0x00`/`0x01`; on `isNewJL`, OFF=`0x02`, ON=`0x01`) | Legacy JL (`!isNewJL`) uses RCSP `setWorkMode(1/2)` |
| `0x4A` | Game Mode Pro | `2B`: `[enabled (0/1), followGameSound (0/1)]` | Adds game-sound enhancement sub-flag |
| `0x2E` | Listening / Music Mode | `1B`: `1`=Music, `3`=Game, `4`=Movie | Overrides custom EQ in Game/Movie modes |
| `0x2D` | Spatial Sound (2-state) | `1B` bool (`0x00`/`0x01`) | Simple on/off spatializer |
| `0x5B` | Spatial Audio (3-mode) | `1B`: `1`=Off, `2`=Fixed Spatial, `3`=Head Tracking | IMU-backed head tracking |
| `0x23` | LDAC Codec | `1B` bool (`0x00`/`0x01`) | Reboots earbuds; mutually exclusive with Dual-Device (`0x24`) on most models via `popViewRules` |
| `0x1B` | aptX / aptX Adaptive | `1B` bool (`0x00`/`0x01`) | Qualcomm QCC models |
| `0x60` | LE Audio | `1B` (`0x01`=Enable) | Toggles Bluetooth LE Audio stack |
| `0x5F` | Privacy Mode Pro | `2B`: `[enabled (0/1), scope (0=Call+Music, 1=Call Only)]` | Anti-leakage acoustic phase cancellation (open-ear models) |
| `0x64` | Dialogue Perception | `1B` bool (`0x00`/`0x01`) | Voice clarity / conversation assist |
| `0x5C` | Voice Detection / Speak-to-Chat | `1B` timeout in seconds: `0`=Off, `5`, `10`, `15`, `20` | Config-only CMDID (`voiceDetection`) |
| `0x32` | Environment Adaptation | `1B` bool (`0x00`/`0x01`) | Auto ambient adaptation |
| `0x53` | Power Saving Mode | `1B` bool (`0x00`/`0x01`) | Config-only CMDID (`powerSave`) |
| `0x5D` | Disconnect Warning | `1B` bool (`0x00`/`0x01`) | Config-only CMDID (`disconnectWarning`) |

---

#### 4.3.7 Multipoint / Dual-Device Management (`0x24`, `0x4D`, `0x4F`, `0x66`)

* **`0x24` (`CMDID_1T2`):** `1B` bool (`0x00`=Single host, `0x01`=Dual-device multipoint enabled). May trigger earbud reboot or conflict with LDAC (`0x23`).
* **`0x66` (`CMDID_MULTI_PAIR`):** `1B` (`0x01`=Enter pairing mode for second host, `0x00`=Exit pairing mode).
* **`0x4D` (`CMDID_PHONE_INFO` — Paired & Connected Host List) (`[Observed]`, `PhoneBeanList.java`):**
  * **RX Framing:**
    ```text
    V[0]: u8 totalCount
    Followed by repeated variable-length host entries while offset < VLen:
      [entryLen: u8]
      [status:   u8]       // 2 or 3 => connected (connectList); other => paired/disconnected (disConnectList)
      [mac:      6B]       // Host Classic BT MAC in normal MSB-first order (%02X:%02X:%02X:%02X:%02X:%02X)
      [name:     UTF-8]    // (entryLen - 7) bytes, stops at first 0x00 byte if null-padded
    ```
* **`0x4F` (`CMDID_PHONE_STATUS` — Host Action & Upload Delimiter) (`[Observed]`, `PhoneStatusBean.java`, `QCYDeviceImpl`):**
  * **TX (`7B`):** `[action (u8), hostMac[0..5]]`
    * `action`: `1`=Connect host, `2`=Delete/Forget pairing, `3`=Disconnect host.
  * **RX (`7B`):** `[hostMac[0..5], status (u8)]`
    * `status = 10` (`0x0A`): Host list upload starting (clear local cache before incoming `0x4D` packets).
    * `status = 11` (`0x0B`): Host list upload finished (commit & render list).
    * Other `status` values report real-time host connect/disconnect/delete completion events.

---

#### 4.3.8 In-Ear Wear Detection (`0x06`, `0x2C`, `0x48`, `0x62`)

* **Simple Switch — `0x06` (`CMDID_EARCHECK`):** `1B` (`0x01`=On, `0x00`=Off; on `isNewJL`, Off=`0x02`).
* **Advanced Wear Config — `0x2C` (`CMDID_WEAR_CONFIG`, `[Observed]`):**
  * **TX / RX (`3B`):** `[useWear, controlMusic, controlANC]`
    * `useWear`: `0`=Off, `1`=On.
    * `controlMusic`: `0`=Off, `1`=Single-bud pause + auto-play on wear, `2`=Single-bud pause + no auto-play, `3`=Dual-bud pause + auto-play on wear.
    * `controlANC`: `0`=Off, `1`=Auto-switch ANC on bud removal.
* **Wear Sensor Sensitivity — `0x48` (`CMDID_WEAR_SENSITIVITY`):** `1B` (`1`=Moderate, `2`=Normal, `3`=Sensitive).
* **Live Wear Status — `0x62` (`CMDID_WEAR_STATUS`):** RX `2B` `u16_LE` sensor bitmask.

---

#### 4.3.9 Scheduled Power-Off & Standby Timeout (`0x14`, `0x1F`)

* **Scheduled Auto Power-Off — `0x14` (`CMDID_POWERMANAGER`, `[Observed]`, `PowerManagerDataBean.java`):**
  * **TX (`4B`):** `[minutes_lo, minutes_hi, 0x00, 0x00]` (`u16_LE` minutes; presets `15, 30, 60, 90`; `0xFFFF` = Disabled).
  * **RX (`4B`):** `[powerTime: u16_LE, curTime: u16_LE]` (`powerTime == 0` is normalized by `PowerManagerDataBean` to `0xFFFF` = Disabled; `curTime` is remaining/elapsed countdown minutes).
* **Disconnect Standby Auto-Off — `0x1F` (`CMDID_STANDBY`):**
  * **TX / RX (`2B`):** `u16_LE` minutes (`5, 10, 30, 60`; `0xFFFF` = Never power off).

---

#### 4.3.10 Voice Prompt Language & Prompt Volume (`0x19`, `0x1A`, `0x1D`)

* **Modern Language Code — `0x19` (`CMDID_VOICENAME`, `[Observed]`):**
  * **TX / RX (`4B` ASCII):** `<2-char chipset prefix><2-digit lang code>`:
    * WuQi: `"WQ00"` (English), `"WQ01"` (Chinese)
    * JieLi: `"JL00"` (English), `"JL01"` (Chinese)
    * Bluetrum / LanXun: `"LX00"` (English), `"LX01"` (Chinese)
* **Legacy Language Index — `0x1A` (`CMDID_VOICE_LANGUAGE`):** `1B` `u8` index.
* **Prompt Tone Volume — `0x1D` (`CMDID_TONEVOLUME`, `[Observed]`):**
  * **TX (`1B`):** `u8` target level (bundled JSON presets: `0x04`=Low, `0x06`=Medium, `0x08`=High, `0x0A`=Max).
  * **RX (`2B`):** `[curVol (u8), maxVol (u8)]`.

---

#### 4.3.11 IMU Head-Gesture Controls (`0x50`, `0x51`, `0x52`)

* **`0x50` (`CMDID_HEAD_ACTION`):** `1B` bool master enable switch.
* **`0x51` (`CMDID_SENSOR_CONFIG`, `[Observed]`):**
  * **TX (`2B`):** `[headEventID (u8), actionCmd (u8)]`
  * **RX (`N × 2B`):** Paired `[headEventID, actionCmd]` bindings.
  * **Event IDs:** `1`=Nod, `2`=Shake Head, `3`=Tilt Left, `4`=Tilt Right.
  * **Action IDs:** `1`=Answer Call, `2`=Reject Call, `3`=Previous Track, `4`=Next Track (`0`=None).
* **`0x52` (`CMDID_SENSOR_TEST`):** TX `0xFA`=Start live IMU calibration/test stream, `0xFB`=Stop; RX `1B` detected `headEventID` (`1..4`).

---

#### 4.3.12 Hearing Safety (`0x58`), Ear-Tip Fit Test (`0x11`) & Find My Earbuds (`0x4B` UI-Only)

* **Hearing Protection — `0x58` (`CMDID_HEARING_HEALTH`, `[Observed]`, `HearingDataBean.java`):**
  * **RX (`4B`):** `[isAlarmVol (1=true), isAlarmTime (1=true), time_minutes (u16_LE)]`.
  * **TX (`4B`):** `[alarmVol, alarmTime, time_lo, time_hi]` (note: bundled JSON `hearingHealth` lists preset durations `30, 60, 90, 120` min as hex strings `"001E"`, `"003C"`, `"005A"`, `"0078"` — verify byte order against target hardware on first write; RX decoder `HearingDataBean.setReceiveData` explicitly decodes `V[2] | (V[3] << 8)` little-endian).
* **Ear-Tip Seal / Fit Test — `0x11` (`CMDID_COMPACTNESS`, `[Observed]`, `CompactnessDataBean.java`):**
  * **TX (`3B`):** `[0x01, 0x01, 0x01]` starts acoustic seal measurement.
  * **RX (`3B`):** `[status (u8), leftResult (u8), rightResult (u8)]` (`status`: `1`=Testing, `2`=Complete; result codes distinguish Good Seal vs Adjust Fit).
* **Find My Earbuds (`[Observed]`, `FindEarphoneVM$checkBluetoothAndPlay$1`, `FindEarphonePlayer`, `ItemViewGridItem`):**
  * **`0x4B` (`CMDID_FIND_EAR`) IS UI-ONLY — IT IS NEVER TRANSMITTED OVER BLE!**
  * **Ringing Mechanism:** Purely client-side audio playback over standard **Bluetooth A2DP**. `FindEarphonePlayer` saves current system media volume, ramps volume from 50% to 100%, plays a high-pitched alert chime (`R.raw.didi2`) with stereo L/R channel panning (`setVolume(leftVol, rightVol)` to isolate Left or Right earbud), and restores original system volume on stop.
  * **Last-Known Location:** Purely client-side OS geolocation (Android Location / AMap / Google Maps) recorded locally on Bluetooth disconnect keyed by `classMac`. The earbuds contain **no GPS hardware** and transmit no coordinates.

---

#### 4.3.13 Speaker RGB Lighting (`0x35`/`0x36`), TWS Pairing (`0x34`), White Noise / Focus Mode (`0x04`/`0x08`/`0x39`–`0x3D`), Clock (`0x3E`) & Alarms (`0x3F`)

* **RGB Lighting (`0x35`, `0x36`, `[Observed]`, `LEDEffectDataBean.java`):**
  * `0x35` (`CMDID_LED_SWITCH`): `1B` bool master LED power.
  * `0x36` (`CMDID_LED_EFFECT`):
    * **TX / RX (`3 + 3*N` bytes):** `[speed (u8), bright (u8), effectIndex (u8), R_0, G_0, B_0, ...]`
    * `effectIndex`: `1`=Solid, `2`=Breathing, `3`=Gradient, `4`=Music Rhythm. Colors are 24-bit RGB triplets packed MSB-first (`(R<<16)|(G<<8)|B`).
* **TWS Dual-Speaker Stereo — `0x34` (`CMDID_TWS_ENABLE`):** `1B` bool.
* **White Noise / Focus Mode (`0x04`, `0x08`, `0x39`, `0x3A`, `0x3B`, `0x3D`, `[Observed]`):**
  * `0x39` (`CMDID_WHITE_NOISE_SWITCH`): `1B` bool (enter/exit Focus Mode).
  * `0x04` (`CMDID_PLAY`): `1B` (`0x00` toggle/play/pause).
  * `0x08` (`CMDID_VOLUME`): `3B` `[vol, vol, 0x00]`.
  * `0x3A` (`CMDID_WHITE_NOISE_PLAY_INFO`): RX `6B` `[musicIndex: u32_LE, status: u8, playMode: u8]`.
  * `0x3B` (`CMDID_WHITE_NOISE_TONE_INFO`): RX `[startToneID: u8, N × (musicID: u32_LE, total: u16_LE)]` listing stored onboard sound tracks.
  * `0x3D` (`CMDID_WHITE_NOISE_PREVIEW`): `1B` (`0x01`=start preview, `0x00`=stop).
  * Custom tone pack uploads use WuQi BLE OTA (`otaType=11`) with `OTAUpgrade.getBytes` header framing (Section 4.4.2).
* **Clock Synchronization — `0x3E` (`CMDID_SYNC_TIME`, `[Observed]`):**
  * **TX (`7B`):** `[year % 100, month (1..12), day (1..31), hour (0..23), minute (0..59), second (0..59), weekdayMask]` where `weekdayMask = 1 << (dayOfWeek - 1)` (`Sun=0x01, Mon=0x02, ..., Sat=0x40`).
* **Onboard Alarms — `0x3F` (`CMDID_ALARM`, `[Observed]`, `AlarmDataBean.java`):**
  * **TX (`7B`):**
    * Add: `[0x01, alarmID, enable(0/1), hour, minute, cycleMask, 0x00]`
    * Delete: `[0x02, alarmID, 0x00, 0x00, 0x00, 0x00, 0x00]`
    * Edit: `[0x03, alarmID, enable(0/1), hour, minute, cycleMask, flag]`
  * **RX (`1 + 6*N` bytes):** `[headerByte, N × (alarmID, enable==1, hour, minute, cycleMask, pad)]`.

---

#### 4.3.14 Rename (`0x18`), Resets (`0x01`–`0x03`) & `popViewRules` Mutual-Exclusion Engine

* **Rename (`0x18` `CMDID_DEVICENAME`):** UTF-8 encoded device display name (typically max `32` bytes). Legacy JieLi (`!isNewJL`) uses RCSP `modifyAdvInfo(type=1)`.
* **Reset Commands (`VLen = 0`):** `0x01` (`CMDID_RESTOREDEFAULT`, settings reset), `0x02` (`CMDID_CLEARPAIR`, clear pairing table), `0x03` (`CMDID_FACTORYRESET`, full factory reset).
* **Client-Side Conflict & Reboot Guard (`PopViewRulesModel`, `ControlFuncViewModelKt.checkPopRules`, `[Observed]`):**
  * Before transmitting a `targetCID` write, the app evaluates model-defined `popViewRules`: if another attribute `expectCID` matches (`==` or `!=`) `expectValue`, the UI intercepts the write and displays either a confirmation `bottomDialog` (e.g., *"Enabling LDAC will disable Dual-Device Connection and reboot the earbuds"*) or a blocking `toast` (e.g., *"Cannot enable Spatial Audio while Adaptive ANC is active"*). Clean-room desktop clients should enforce the same rule table per model.

---

### 4.4 OTA Firmware Update Protocol Catalog

#### 4.4.1 Firmware Acquisition & Dispatch Flow (`[Observed]`)

1. **Firmware Source (`OTAActivity`, `EarphoneOTAVO`):**
   * **Cloud OTA:** App queries `GET /qcy-earphone/app/ota/check` (`[Observed]` DTO `EarphoneOTAVO`) passing `modelID` and current firmware version. Server returns `firmwareUrl`, target version, changelog, and `otaType`. The binary is downloaded to cache as `tmp.ota`.
   * **Local File OTA (`OTAActivity.pickFileLauncher`):** The APK natively supports selecting a local `.ota` / `.bin` file directly from storage and feeding its path into the identical upgrade pipeline — confirming that a desktop clean-room app can safely support local user-supplied firmware files without requiring QCY cloud authentication!
2. **Dispatch (`QcyEarphoneManager.startOTA` -> `QCYDeviceManager.upgradeFirmware`):**
   * Instantiates the chipset-specific `OTABasePresenter` selected by `otaType` (integer field from model JSON `upgrade.otaType` or cloud response).
   * Executes `presenter.connectSPP()` (opens dedicated OTA BLE GATT service **or** Classic Bluetooth RFCOMM socket depending on chipset) followed by `presenter.startOta(filePath)`.
   * **Pre-flight Safety Checks (`[Observed]`):** Both earbuds connected (for TWS), battery above minimum threshold (typically `>= 30%`), and charging case/power state valid before entering bootloader/OTA mode.

#### 4.4.2 Complete `otaType` Master Routing Table (9 Active Routes Across 7 Vendor Protocols)

> **Critical Transport Finding for Windows (`[Observed]`):** Out of 9 active `otaType` values, **6 run purely over BLE GATT** (`otaType = 2, 6, 8, 9, 10, 11` — covering JieLi, WuQi, BES, and Bluetrum, i.e., the vast majority of modern QCY devices), while **3 legacy/specialty routes require Classic Bluetooth RFCOMM/SPP** (`otaType = 1` PixArt/AWOTA, `otaType = 5` Qualcomm QCC GAIA, `otaType = 7` Actions).

| `otaType` | Presenter Class | Chipset / Embedded SDK | Transport | GATT UUIDs / RFCOMM UUID | Auth / Encryption | Wire Framing, Chunking & Verification (`[Observed]`) |
|---|---|---|---|---|---|---|
| **`1`** | `T5OTAPresenter` | **PixArt / AWOTA** (`com.awota.ota`, `Part267`/`279`/`281`) | **Classic SPP** (`DevObjSPP`) | SDP `device.getUuids()[0]` (typ. SPP `0x1101`) | None | TX header `0x50 0x41` (`"PA"`), RX `0x50 0x49` (`"PI"`), category `CT_06_OTA` (`0x06`). Commands `0x30..0x3E`: `OTA_MODE`, `ERASE_LOAD_REGION`, `SET_LOAD_INFO`, `LOAD_MORE`, `CALC_CRC`/`CRC32`, `UPDATE`, `HANDOVER`, `REBOOT`. Block size `248B` (SPP) / `236B` default, 3-packet pipeline window, CRC-16 or CRC32-MPEG2. *(Note: earlier product docs misattributed `otaType=1` to Airoha; bytecode proves it is PixArt/AWOTA.)* |
| **`2`** | `JLSDKCommonPresenter(..., false)` | **JieLi** (`com.jieli.jl_bt_ota`) | **BLE GATT** (`bleWay=true`, MTU `256`) | Svc `0xA002` / W `0x0001` / N `0x0002` | **None** (`useDeviceAuth=false`) | Standard JieLi RCSP `FE DC BA ... EF` framing (Section 3.2). OTA opcodes: `0xE1` (`GET_FILE_INFO_OFFSET`), `0xE2` (`INQUIRE_IF_CAN_UPDATE`), `0xE3` (`ENTER_UPDATE`), `0xE4` (`EXIT`), `0xE5` (`SEND_BLOCK`, device pulls chunks by `[offset, len]`), `0xE6` (`REFRESH_STATUS`), `0xE7` (`REBOOT`), `0xE8` (`NOTIFY_SIZE`), `0xD1` (`MTU`), `0xD4` (`MD5`). `libjl_ota_auth.so` `nativeFilterFile` optionally extracts matching MCU image segment. |
| **`5`** | `QCCOTAPresenter` | **Qualcomm QCC** (`com.qualcomm.qti.gaiaclient`) | **Classic RFCOMM** | GAIA `00001107-D102-11E1-9B23-00025B00A5A5` (fallback `0x1101`) | None | Standard Qualcomm GAIA v1/v2/v3 Upgrade protocol (`UpgradeGaiaCommand`: `CONNECT`, `CONTROL`, `DISCONNECT`; `UpgradeRepository.startUpgrade` default chunk size `873B`; interactive/auto confirmations for `TRANSFER_COMPLETE`, `IN_PROGRESS`, `COMMIT`). |
| **`6`** | `WQBLEPresenter` | **WuQi** (`WQBleOtaBridge`, `tool.android.all.protocol.ota`) | **BLE GATT** | Svc `0x7033` / W `0x2001` / N `0x2002` | None | Request magic `[0x70, 0x07, 0x6E]`, Response magic `[0x33]`, rolling 1-byte seq `0..127`. State machine (`UpgradeStep`): `CheckState` -> `EnterUpgradeMode` -> `GetOffset` (resume support) -> `TransferFile` (MTU-segmented chunks) -> `TransferEnd` -> `CheckSyncState` (TWS bud-to-bud sync) -> `ResetDevice` -> `Completed`. |
| **`7`** | `ActsOtaPresenter` | **Actions** (`com.actions_local`, `"SPP_ONLY"`) | **Classic SPP** | `0000E003-0000-1000-8000-00805F9B34FB` | None | Sends `"01234567"` `knockDoor` sequence; validates `"AOTA"` firmware image header (`fwVersion` + 16B `VRAM`); TLV pull protocol (`0x80` super-TLV, `FRAME_SIZE=256B`) with missing-chunk bitmap retransmission (`BitmapUtils.getZeroBitIndexMap`). |
| **`8`** | `BESOtaPresenter` | **BES (Bestechnic)** (`besota`) | **BLE GATT** (`Ble`, MTU `512`) | Svc `66666666-...` / W+N `77777777-...` | None | Magic `"BEST"` (`0x42 45 53 54`). `0x85` data packets, `0x82` CRC32 segment verification, `0x92 "BEST"` overwrite confirmation, `0x90 0x11` apply image change, `0x11 0x22` pass / `0x33 0x44` resend, `0x95 0x83 0x01` TWS role switch + reconnect + offset resume. |
| **`9`** | `JLSDKCommonPresenter(..., true)` | **JieLi (Auth)** (`com.jieli.jl_bt_ota`) | **BLE GATT** (`bleWay=true`, MTU `256`) | Svc `0xAE00` / W `0xAE01` / N `0xAE02` | **`RcspAuth` Required** (`libjl_ota_auth.so`) | Identical RCSP OTA opcodes (`0xE1`–`0xE8`) as `otaType=2`, preceded by 17-byte mutual challenge-response authentication (`RcspAuth` -> JNI `libjl_ota_auth.so`, completes on `"\x02pass"`). |
| **`10`** | `FOtaPresenter` | **Bluetrum FOTA** (`com.bluetrum.fota`, `ABOta`) | **BLE GATT** | Svc `0xFF12` / N(In) `0xFF14` / W(Out) `0xFF15` | None (device-side firmware header check) | Identification magic `[0xCC, 0xAA, 0x55, 0xEE, 0x12, 0x19, 0xE4]`; rolling 1-byte `seqNum`; commands `0x91` (`GET_INFO`), `0x92` (`GET_INFO_TLV`), `0x90` (`NOTIFY_STATUS`), `0xA0` (`OTA_INFO`), `0x20` (`SEND_DATA`); block size `4096B`, packet size `240B`, CRC verification. |
| **`11`** | `WQBLEPresenter` | **WuQi BLE + Tone Pack** | **BLE GATT** | Svc `0x7033` / W `0x2001` / N `0x2002` | None | Identical transport/state machine to `otaType=6`; also used to upload custom White Noise tone `.bin` files wrapped with `OTAUpgrade.getBytes` header (below). |

*(Legacy/dormant presenters present in `qcydevicelibrary` but not routed by `QcyEarphoneManager.startOTA`: `WQPresenter` [Classic SPP WuQi, MTU `553`], `QCCOTAPresenterT11s` [Qualcomm GAIA for T11S `0x4D17`], and `BluetoothSPPManager` [generic SPP `0x1101`].)*

#### 4.4.3 WuQi Tone Pack `.bin` Envelope (`OTAUpgrade.getBytes`, `WQToneFileUtils.kt`, `[Observed]`)

When flashing custom white-noise audio packs to WuQi devices (`otaType=11`), the raw tone payload is wrapped in a binary container before entering `WQBLEPresenter`:
```text
[0x00..0x03] u32_LE magic       = 0xABCD6543
[0x04..0x07] u32_LE fwVersion   = 1
[0x08..0x0B] u32_LE offset/addr = 0x00001000
[0x0C..0x0F] u32_LE reserved    = 0xFFFFFFFF
[0x10..0x1F] 4 × u32_LE sizes   = [payloadLen, payloadLen, 0, 0]
[0x20..0x23] u32_LE headerCrc32 = IEEE CRC32 of metadata header
[0x24..    ] payload bytes
[EOF-4..EOF] u32_LE dataCrc32   = IEEE CRC32 of payload bytes
```

---

## 5. Unresolved Unknowns, Obfuscated/Native Gaps & Clean-Room Windows Guidance

### 5.1 Explicitly Documented Unknowns & Native Gaps

| # | Area | Status | Details & Impact |
|---|---|---|---|
| 1 | **`libjl_ota_auth.so` (`RcspAuth` crypto)** | `[Unknown]` (ARM32 native binary) | Used **only** for JieLi OTA `otaType=9` (`0xAE00`). Daily control (`0xA001` & `0xA002`) and JieLi OTA `otaType=2` (`0xA002`) do **not** use `RcspAuth`. Reverse-engineering the 17-byte challenge transform inside `libjl_ota_auth.so` (10.9 KB) is deferred until `otaType=9` flashing is targeted. |
| 2 | **`libjl_enc.so` (`JLEnc` hash)** | `[Unknown]` (ARM32 native binary) | 13.5 KB native library for JieLi BLE advertisement hashing. Unused on QCY standard manufacturer IDs `0x071E`/`0x521C`/`0x522C` — zero impact on daily discovery or control. |
| 3 | **BLE Adv Reserved Bytes `[2]` and `[8..10]`** | `[Unknown]` | Inside the 24-byte `0x071E`/`0x521C`/`0x522C` payload, bytes `[0..1]`, `[3]`, `[4]`, `[5..7]`, `[11..16]`, `[17]`, and `[18..23]` are fully decoded; bytes `[2]` and `[8..10]` are not parsed by `QCYDeviceManager$scanCallback$1` (likely protocol flags / case status / reserved). |
| 4 | **`0x4F` (`PhoneStatusBean`) Non-Delimiter Status Codes** | `[Inferred]` | `status = 10` (upload start) and `11` (upload end) are `[Observed]`; exact enum labels for transient connect/disconnect failure codes (`0..9`) should be verified against a live multipoint earbud. |
| 5 | **`0x58` (`HearingDataBean`) TX Endianness Edge Case** | `[Observed]` RX / `[Inferred]` TX | RX (`HearingDataBean.setReceiveData`) is unambiguously `u16_LE`; bundled JSON stores preset strings like `"001E"`. Verify live TX byte order (`1E 00` vs `00 1E`) when testing a Hearing-Health-enabled model. |
| 6 | **Chipset Vendor Names for `ZR` (`0x0090`) & `RD` (`0x00A0`)** | `[Inferred]` | Mask names `isZRDevice` (5-band EQ) and `isRDDevice` come directly from `QCYDeviceBean`, but no dedicated OTA SDK for `ZR`/`RD` is bundled in v1.2.3 (both use standard `0xA001` daily control). |
| 7 | **Server-Served Full Model Catalog ZIP** | `[Unknown]` (Cloud-only) | The APK bundles 25 `res/raw/function_*.json` device profiles offline; newer models download additional JSON definitions and product renders from QCY's cloud CDN at runtime. |
| 8 | **Unverified Legacy/Edge CMDIDs** | `[Unknown]` | A few `ConstantCMDID` constants (`0x05`, `0x0B`, `0x12`, `0x15`, `0x1E`, `0x21`, `0x25`, `0x26`, `0x37`, `0x38`, `0x3C`, `0x41`, `0x43`, `0x49`, `0x4C`, `0x4E`, `0x59`, `0x61`) are declared in `ConstantCMDID.java` with zero active callers in v1.2.3 DEX; their payloads remain unverified until observed on live firmware. |

### 5.2 Actionable Clean-Room Implementation Notes for Windows (WinRT / Rust)

1. **Daily Control Transport (`crates/qcy-transport`):**
   * **BLE GATT ONLY is required for 100% of daily control features** across every QCY chipset family (WuQi, JieLi, Bluetrum, BES, Qualcomm, Realtek, Actions, PixArt, ZR, RD).
   * **Passive Discovery & Battery Popup (Zero GATT Connection):** Configure `BluetoothLEAdvertisementWatcher` with `ScanningMode::Active` and filter `ManufacturerData` for Company IDs `0x071E`, `0x521C`, and `0x522C` (plus fallback scan by OUI `84:AC:60`, `1C:52:16`, `C4:33:96` or Classic Bluetooth paired devices). Parsing bytes `[0..1]` (`modelID` Big-Endian), `[3]` (`isLeft`/`colorIndex`), `[4]` (`isTws`/`isPop`), `[5..7]` (L/R/Case battery + charging), `[11..16]` (`classMac`, swapped), `[17]` (`popNum`), and `[18..23]` (`otherMac`, swapped peer Classic MAC) gives instant Windows tray battery readout and lid-open popups without opening a GATT handle!
   * **Service Auto-Detection on Connect:** Enumerate GATT services for:
     * `0000A001-0000-1000-8000-00805F9B34FB` -> **QCY Unified TLV dialect** (write `0x1001` using `WriteWithResponse` when `Write` is supported, falling back to `WriteWithoutResponse` if only `WriteWithoutResponse` is supported; subscribe `0x1002` and legacy `0x0008` if present; optional legacy direct characteristics `0x0007`, `0x0008`, `0x000B`, `0x000D`).
     * `0000A002-0000-1000-8000-00805F9B34FB` -> **JieLi RCSP dialect** (write `0x0001`; subscribe `0x0002`). Check `(modelID & 0x8070) == 0x8070`: if true (`isNewJL`), tunnel `[0xFF, ...]` QCY TLV inside RCSP `0xFF` (`CMD_EXTRA_CUSTOM`) and apply the `OFF = 0x02` quirk for `0x06`/`0x09`; if false, use native RCSP `0x03`/`0x07`/`0x08`/`0xC0`–`0xC5` (with `changeQCY2JL(0) = 0x7F` for buttons).
   * **MTU & Command Pacing:** Request/read `GattSession.MaxPduSize` (Windows auto-negotiates up to 512–517B); enforce a **100 ms minimum inter-write queue delay** (`CommandProcessor` parity) and handle **multi-TLV notification splitting** inside `0xFF` RX frames (Section 3.1).
2. **Classic Bluetooth Requirements on Windows:**
   * **Audio / A2DP (Find My Earbuds & Active-Device Matching):** Enumerate paired Classic Bluetooth devices (`BluetoothDevice::GetDeviceSelectorFromPairingState(true)`) to match Classic MACs from BLE Adv `[11..16]` and check A2DP connection state; Find My Earbuds plays a local WAV/sine chime to the QCY audio endpoint with L/R stereo channel panning via WASAPI / Web Audio API (no BLE command needed).
   * **Classic RFCOMM / SPP:** Required **only** if implementing OTA flashing for legacy PixArt (`otaType=1`, SPP `0x1101`), Qualcomm GAIA (`otaType=5`, `00001107-D102-11E1-9B23-00025B00A5A5`), or Actions (`otaType=7`, `0000E003-...`). All modern BLE OTA routes (`otaType = 2, 6, 8, 10, 11`) work entirely over WinRT BLE GATT.
3. **Clean-Room & Security Compliance:**
   * Zero proprietary vendor keys, cloud API secrets, signing certificates, or copyrighted assets are included in this specification or needed for local Windows Bluetooth control.
   * For OTA safety, display explicit battery/TWS pre-flight warnings and support user-supplied local `.ota`/`.bin` firmware files, starting with open unauthenticated BLE OTA protocols (`otaType = 2, 6, 8, 10, 11`) before evaluating `otaType = 9` (`RcspAuth`).
