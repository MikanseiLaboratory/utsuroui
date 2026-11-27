# pico-tft — RP2040 / RP2350 ファームウェア例

utsuroui のデモ（メニュー / 計測 / 設定 / About）を、240×320 RGB565 の
ST7789 パネルに出すファームウェア。RP2040（Pico）と RP2350（Pico 2）を
feature で切り替える。

## 配線

| 信号 | GPIO |
|---|---|
| SCK | GP18 |
| MOSI | GP19 |
| CS | GP16 |
| DC | GP17 |
| RST | GP20 |
| BL | GP21 |
| TE（任意） | GP22 |
| Button A（Next / 長押しで Back） | GP14 → GND |
| Button B（Select） | GP15 → GND |

パネルは 3.3 V、SPI モード 0、240×320。`SCK = 31.25 MHz` は
`src/panel.rs` の `SPI_HZ`。実機で安定上限を確認してから上げること。

## ビルド

```sh
# RP2040
cargo build -p pico-tft --release --target thumbv6m-none-eabi

# RP2350（Pico 2）
cargo build -p pico-tft --release --no-default-features --features rp235xa \
    --target thumbv8m.main-none-eabihf

# TE 待ち（GP22 に TE を配線した場合）
cargo build -p pico-tft --release --features te --target thumbv6m-none-eabi
```

## 書き込み

```sh
cd examples/pico-tft
probe-rs run --chip rp2040          # RP2040
probe-rs run --chip RP235x          # RP2350
```

`defmt` のログが RTT に出る。起動後にメニューが出れば成功。

## 動作

- **Core 0**: ボタンを 200 Hz でポーリングし、`Debouncer` でデバウンスして
  `Event` を送る。UI ループは `utsuroui::embassy_task::ui_task`。
- **Core 1**: 合成の遅延波形（25.0〜45.0 ms）を 200 Hz で `Watch` に書く。
  実製品ではここを GPIO タイムスタンプに置き換える。
- **UI**: 遅延値は 30 Hz に間引き、数値は 120 ms の tween で追従。
  メニューのハイライトは 120 ms。全画面転送は 16 行 × 20 帯。

ボタン操作:

| 操作 | 動作 |
|---|---|
| A 短押し | 次の項目へ |
| B 短押し | 決定（計測画面では freeze 切替） |
| A 長押し | 戻る |
| A 長押しのリピート | 次の項目へ連送 |

設定画面の `Transition` 行でカット / フェード / スライド / クロスフェードを
切り替えられる（デモの動作確認用）。

## 実測したい項目

`docs/HARDWARE.md` の手順で SPI クロック、スタック、フレーム時間を測る。
`FlushStats` を `defmt::info!` すると帯数とバイト数が出る。
