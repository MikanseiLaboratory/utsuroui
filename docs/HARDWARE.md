# ハードウェアと実機での確認

ファームウェアは 3 本ある。ピン割り当ては配線ごとに違うので、必ず対応する
example の README と `src/panel.rs` を確認すること。`examples/anim-showcase`
は `examples/spi1-panel` と同じ配線を使う。

| example | 想定ハード | パネル | バス |
|---|---|---|---|
| `examples/pico-tft` | Raspberry Pi Pico / Pico 2 | 240×320 ST7789 | SPI0 |
| `examples/spi1-panel` | GPIO を下記のとおり配線した機器 | 240×320 ST7789（反転） | **SPI1** |

`spi1-panel` は特定の基板を前提にしない。必要なのは下の GPIO を揃えることだけで、
他のピンや周辺チップには触らない。`anim-showcase` と `gui-showcase` は同じ配線を
使う（パネル 6 本 + キー 4 本）。

## Pico / Pico 2（`examples/pico-tft`）

| 信号 | GPIO | 備考 |
|---|---|---|
| SCK | GP18 | SPI0 |
| MOSI | GP19 | SPI0 TX |
| CS | GP16 | `ExclusiveDevice` が排他制御 |
| DC | GP17 | コマンド / データ |
| RST | GP20 | パネルリセット |
| BL | GP21 | 起動後 High（既定） |
| TE | GP22 | feature `te` のときだけ入力 |
| Button A | GP14 | GND へ短絡、内部プルアップ |
| Button B | GP15 | GND へ短絡、内部プルアップ |

- パネルは 240×320 RGB565、ST7789（既定）または ILI9341。
  ILI9341 にする場合は `examples/pico-tft/src/panel.rs` のモデルと `Panel` 型を
  `ILI9341Rgb565` に差し替える。
- 3.3 V ロジック。5 V トレラントでないモジュールはレベルシフトすること。
- コントラストがおかしい / 赤と青が入れ替わる場合は
  `COLOR_ORDER`（`ColorOrder::Rgb` / `Bgr`）と `ORIENTATION` を先に疑う。
  バイト順（ビッグエンディアン）は `lcd-async` と一致しているので、
  症状が「色化け」だけなら MADCTL / BGR 側が原因。

## SPI1 パネル + 8 キー（`examples/spi1-panel`）

### ディスプレイ（ST7789 240×320、反転、SPI1）

| 信号 | GPIO |
|---|---|
| MOSI | GP11 |
| MISO | GP12（未使用） |
| SCK | GP14 |
| CS | GP13 |
| DC | GP22 |
| RST | GP27 |
| BL | GP28 |

SPI1 を最大 **62.5 MHz**（`clk_peri` / 2）で使う。`panel::SPI_HZ` で変更でき、
ノイズや色ずれが出る場合は 50 MHz に落とす。

### フロントパネルキー（GND 短絡、プルアップ）

| キー | GPIO | デモでの動作 |
|---|---|---|
| MENU | GP8 | 次へ（リピートあり） |
| PREV | GP9 | 前へ（リピートあり） |
| INPUT 1 | GP1 | 決定 |
| BACK | GP26 | 戻る |

未使用のキー: INPUT 2–4 = GP2–GP4、NEXT = GP10、SHIFT = GP15。
LED（GP25）は UI 起動後に High。

## 実測: クロックとフレームレート

RP2040 の実機で読んだ値（`probe-rs read`、SPI1 パネル + キーの example）:

| 項目 | 値 | 根拠 |
|---|---|---|
| clk_sys | **125 MHz** | PLL_SYS 1500 MHz（FBDIV=125）/ POSTDIV 6·2 |
| コア電圧 | 1.10 V | VREG VSEL = 0xB |
| clk_peri / SPI | 62.5 MHz | `SPI_HZ = 62_500_000` → CPSR 2, SCR 0 |
| 波形シーンのフレーム | **17.5 ms（57 fps）** | フレームカウンタを SWD で 6 秒間サンプル |

250 MHz も試した（PLL の VCO は 1500 MHz のまま POSTDIV 6·1、電圧 1.15 V、SPI は
62.5 MHz を維持）が、**フレームレートは 57.0 fps で変わらなかった**。CPU ではなく
SPI 転送とフレーム予算が律速なので、オーバークロックは採用していない（フラッシュの
分周も embassy は調整しないため、XIP が 2 倍になるリスクだけが残る）。

## SPI クロック

| SCK | 全画面 1 枚の理論 fps | 位置づけ |
|---|---|---|
| 15.15 MHz | 約 12.3 | ST7789V データシートの書き込み周期に近い安全側 |
| 31.25 MHz（既定） | 約 25.4 | `clk_peri / 4`（clk_peri = 125 MHz） |
| 40 MHz | 約 32.6 | モジュール依存。**未保証** |
| 62.5 MHz | 約 50.9 | 配線が短くモジュールが耐える場合のみ |

`panel::SPI_HZ` を書き換えてビルドし、必ず実機でノイズ・飛び・色ずれを確認する。
ILI9341 は公式書き込み周期がさらに遅い可能性がある。

## ビルドと書き込み

```sh
# RP2040
cargo build -p pico-tft --release --target thumbv6m-none-eabi
cd examples/pico-tft && probe-rs run --chip rp2040

# RP2350（Pico 2）
cargo build -p pico-tft --release --no-default-features --features rp235xa \
    --target thumbv8m.main-none-eabihf
cd examples/pico-tft && probe-rs run --chip RP235x

# SPI1 パネル + キー（RP2040）
cargo build -p spi1-panel --release --target thumbv6m-none-eabi
cd examples/spi1-panel && probe-rs run --chip rp2040

# TE 待ちを有効にする（TE を GP22 に配線した場合のみ）
cargo build -p pico-tft --release --features te --target thumbv6m-none-eabi
```

`cortex-m-rt` の `memory.x` は `build.rs` がチップ feature に応じて生成する。
RP2040 は `.boot2`（0x100 バイト）付き、RP2350 は `.start_block` /
`.bi_entries` / `.end_block` 付き。

## 実測手順（未実施）

実機がつながったら、以下を測って `ARCHITECTURE.md` の表を更新する。

### 1. 静的サイズ

```sh
rustup component add llvm-tools-preview
SYS=$(rustc --print sysroot); HOST=$(rustc -vV | sed -n 's|host: ||p')
"$SYS/lib/rustlib/$HOST/bin/llvm-size" -A target/thumbv6m-none-eabi/release/pico-tft
```

現在の実測値（CI Linux ランナー, release, defmt 込み）:
RP2040 `.text` 35,864 / `.rodata` 13,924 / `.bss` 21,104、
RP2350 `.text` 35,640 / `.rodata` 14,036 / `.bss` 21,136。

### 2. スタック最低水位

`flip-link` を入れてリンクするか、タスク起動時にスタックを 0xA5 で塗って
`target/thumbv6m-none-eabi/release/pico-tft` を `probe-rs` で読む。

```sh
cargo install flip-link
# .cargo/config.toml の rustflags に -C linker=flip-link を足して再ビルド
```

### 3. フレーム時間と転送量

`utsuroui::flush::FlushStats`（`rects` / `bands` / `bytes`）を RTT に出す。
`ui_task` の `flush` 戻り値を `defmt::info!` する 1 行を足すだけでよい。

```text
[INFO] frame: rects=1 bands=4 bytes=25088
```

同じ値を SPI クロックで割れば理論転送時間になる。

### 4. 入力遅延

本番の計測は core 1 で GPIO 入力のタイムスタンプを取る。デモの
`measure::run` は合成波形なので、実機ではこの関数を実測コードに置き換える。
UI 側は `Watch` から最新値を受け取るだけなので変更不要。

### 5. TE

TE を配線した場合のみ `--features te` でビルドし、`TearingEffect::Vertical` の
待ちが効いているか（スクロール時の tear が減るか）を確認する。
`TeSink` は帯ごとに待つ保守的な実装なので、実測して効果が薄ければ
「1 フレーム 1 回」に最適化する。
