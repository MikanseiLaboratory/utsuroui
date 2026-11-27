# 実装計画・決定事項・設計書との差分

この文書は [DESIGN.md](DESIGN.md) を実装に落とすにあたっての
**決定事項・差分・未実施項目**をまとめる。設計書本文は原文のまま残し、
実装の正はコードとテスト、判断の記録はここに置く。

## 1. マイルストーン

| MS | 内容 | 状態 |
|---|---|---|
| **M0** | `geom` / `color` / `dirty` / `anim` / `text` / `event` / `focus` / `input`、ホスト単体テスト、CI 骨格 | 完了 |
| **M1** | `paint` / `band` / `flush` / `widget` / `nav` / `app`、`lcd` アダプタ、デモ 4 画面、ホスト描画、ゴールデン画像、RP2040 ファームウェア（パネル初期化・入力・合成計測） | 完了 |
| **M2** | ハイライト / 数値 tween、黒経由フェード、`embassy` の `ui_task`、静止時 sleep、TE feature、ゴールデン追加、転送量予算テスト | 完了 |
| **M3** | スライド、クロスフェード、エンコーダ / タッチ入力、`u8g2` feature、RP2350 対応（memory.x・リンク）、実測サイズの記録、README スクリーンショット、CI 最終化 | 完了 |

## 2. 設計書 §15.5 の未決事項 → 決定

| # | 未決事項 | 決定 |
|---|---|---|
| 1 | クレート名・公開 | **`utsuroui`**（移ろい + UI）。GitHub public、crates.io 公開はしない |
| 2 | RP2350 の既定 | **ストライプ共通**。クロスフェードも 2 帯の再描画で行うため全面 FB を必要としない |
| 3 | 遷移中の Back | **打ち切ってカットで戻る**（`DemoApp::event` の遷移中 Back 分岐、テストあり） |
| 4 | 2 ボタン長押し | **A 長押し = Back**、B 長押し = `LongPress`（デモでは未使用） |
| 5 | `lcd-async` 依存 | 直接依存。`lcd` feature に閉じ、境界は `BandSink` 実装のみ |
| 6 | ゴールデン形式 | **PNG をコミット**し、デコード後に RGB565 バイト列で厳密比較 |
| 7 | 日本語 UI | 初回は ASCII。`u8g2` バックエンドは feature として実装、デモ既定は `embedded-graphics` 付属フォントのみ |
| 8 | 画面サイズ | 描画バッファ長はコンパイル時、画面 `Rect` は実行時（`RenderCfg`）。デモは 240×320 固定 |

## 3. 設計書からの主な差分

1. **クロスフェードは旧フレームのスナップショットを持たない。**
   設計書 §9.5 は「旧画面のスナップショット 153,600 B」を想定していたが、
   デモは全画面の State を常に保持しているため、**旧画面をもう一度描いて
   2 枚の帯バッファを混合する**方式にした（`App::paint_under`）。
   RAM は 15,360 B（帯 2 枚）で済み、RP2040 でも動く。
   `full-fb` feature は「全面に 1 回描いてから帯で送る」別経路として残した
   （ホストテストで帯経路と画素一致を検証）。
2. **`BandSink::write_band` に `Send` 境界を付けない。**
   `lcd-async` 0.1.3 の `Interface` は RPITIT の future に `Send` を宣言しておらず、
   汎用 impl では `Send` を証明できない。Embassy 0.10 の thread-mode `Spawner` は
   非 `Send` タスクを受け付けるため、`Send` 制約自体が不要だった
   （`embassy-executor` 0.10 の `Spawner::spawn` は `Send` を要求しない）。
3. **`Tween::retarget` はアプリの `event(now_us)` から呼ぶ。**
   スケッチの `Widget::event` には時刻が無く `now=0` で再ターゲットしていた。
   実装ではイベント API に `now_us` を通し、連続性をテストで固定している。
4. **イージングは多項式近似ではなく LUT。**
   スケッチの `ease()` は二次近似だったが、実装は 257 要素の `static` LUT +
   線形補間（設計書 §9.3）。補間誤差はテストで許容範囲を明示。
5. **`EventOut` から `navigate` を外した。**
   画面遷移はアプリ（`Router` / `Transition`）が持ち、ウィジェットは
   `dirty` / `consumed` だけを返す。ウィジェットに `NavAction<S>` を
   ジェネリックに通す必要がなくなり、API が単純になった。
6. **`Dirty` 型エイリアスを追加。**
   `DirtyList::new(..)` は const generic を推論できないため、
   `pub type Dirty = DirtyList<8>` を用意した。
7. **画面スライドは「アプリが 2 画面を描く」。**
   フレームワークは `Transition::slide_progress` を返すだけで、
   旧画面と新画面のオフセット描画はアプリが行う（デモ実装）。

## 4. 今回実装していない項目と根拠

| 項目 | 理由 | 代替 |
|---|---|---|
| DMA と描画の真のオーバーラップ | 設計書 §2 / M3 が「実機で測って効果があるときだけ」と条件付けている。実機なしでは判断できない | `BandSink` と帯 2 枚の配置は用意済み。`ARCHITECTURE.md` に分割 API の実装方針を記載 |
| SPI 実測表・スタック最低水位・フレーム実測 | 実機が必要 | 静的サイズは `llvm-size` で実測し記録。手順は `HARDWARE.md` |
| ビルド時日本語グリフ生成 | 設計書 §15.5 #7 が未決。フォント再配布条件の確認が必要 | `u8g2` feature（バックエンド）を実装。フォントはアプリごとに選択・ライセンス記録 |
| crates.io 公開・リリース | 今回の作業範囲外 | `cargo package` 可能な構成（README は workspace 共有） |
| 物理エンコーダ配線 | デモの操作系は 2 ボタン | 直交デコード (`EncoderDecoder`) と `Event::Encoder` は実装・テスト済み |
| 角丸矩形・4bpp AA フォント・アイコン RLE・スプリング | 設計書でも later | — |

## 5. ファームウェア example

| example | 基板 | パネル / バス | キー |
|---|---|---|---|
| `examples/pico-tft` | Pico / Pico 2 | 240×320 ST7789 / SPI0 | 2 ボタン（GP14/15） |
| `examples/iryx-sw00` | Iryx SW-00（W5500-EVB-Pico） | 240×320 ST7789（反転）/ **SPI1** | MENU GP8 / MARK IN GP9 / INPUT1 GP1 / BACK GP26 |

SW-00 のピン割り当ては `iryx` の旧 `switcher` ファームウェア
（commit `fd992f7^`、`pico/src/bin/switcher.rs`）に合わせた。詳細は
[HARDWARE.md](HARDWARE.md) と各 example の README。

## 6. 型スケッチ `sketch/` の扱い

設計書に付属していた `sketch/`（型チェック用の砂場）は、実装時に
`crates/utsuroui` と `crates/utsuroui-demo` に移植した。スケッチは製品コードでは
ないため、このリポジトリには含めていない。移植時の主な変更は本ファイル §3 を参照。

## 7. テスト一覧（ホスト）

| 種別 | 件数 | 内容 |
|---|---|---|
| コア単体 | 87 | geom / color / dirty / anim / text / event / focus / input / nav / widget |
| コア統合（`tests/render.rs`） | 11 | 帯分割、部分更新、バイト順、クロスフェード、`full-fb` 等価性、テキスト |
| デモ単体 | 3 | 合成遅延波形（範囲・周期性・連続性） |
| デモ（`tests/app.rs`） | 16 | メニュー、遷移、Back 打ち切り、設定、間引き、freeze、wakeup |
| デモ予算（`tests/budget.rs`） | 3 | ハイライト移動 ≤ 32 KiB、全面 20 帯、任意アプリ |
| ゴールデン（`tests/golden.rs`） | 9 | PNG 8 枚 + 幾何 |
| `u8g2` feature（`tests/u8g2_backend.rs`） | 1 | 任意バックエンドの描画 |
| doc test | 1 | クレート冒頭の例 |

再生成: `UPDATE_GOLDEN=1 cargo test -p utsuroui-demo --test golden`。
生成後は必ず画像を目視確認してからコミットする。

## 8. 実測手順（実機がつながったとき）

1. `probe-rs run --chip rp2040` で起動し、`defmt` の起動ログを確認。
2. 全面赤 / 黒の切替で色順・向き・オフセットを確認（`panel.rs` の
   `COLOR_ORDER` / `ORIENTATION`）。
3. SPI を 15 MHz → 31.25 MHz → 40 MHz と上げ、ノイズ・飛び・色ずれを確認。
   `SPI_HZ` を書き換えてビルド。
4. `docs/HARDWARE.md` の手順で `llvm-size` とスタック塗装を測り、
   `ARCHITECTURE.md` の予算表を実測で置き換える。
5. ハイライト移動時の `FlushStats.bytes` とフレーム時間を RTT に出す
   （`ui_task` に計測フックを足す）。
