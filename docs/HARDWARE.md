# ハードウェアと実機での確認

## 配線（Raspberry Pi Pico / Pico 2 共通の GPIO 番号）

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

現在の実測値（defmt 込み）:
RP2040 `.text` 35,860 / `.rodata` 14,076 / `.bss` 21,112、
RP2350 `.text` 35,652 / `.rodata` 14,192 / `.bss` 21,144。

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
