# フォントライセンス方針

utsuroui の既定ビルドは `embedded-graphics` 付属のモノスペースビットマップ
フォント（MIT OR Apache-2.0）だけを使う。追加フォントはアプリが選ぶ。

## `u8g2` feature

- クレート `u8g2-fonts` の**コード**は MIT OR Apache-2.0。
- **同梱フォントは U8g2 由来で、フォントごとにライセンスが違う。**
  U8g2 の LICENSE（https://github.com/olikraus/u8g2/blob/master/LICENSE）と
  フォントグループ wiki（https://github.com/olikraus/u8g2/wiki/fntgrp）を
  採用するフォントごとに読むこと。
- 代表的な条件:
  - Adobe X11 由来（HELV / COUR / TIM / NCEN / SYMB 系）は
    Adobe Systems / Digital Equipment Corporation の著作権表示が必要。
  - OFL のフォントは OFL の条件（再配布時のライセンス同梱など）に従う。
  - 自作・パブリックドメイン由来のものもある。
- 商用製品に同梱する場合は、**使ったフォント名・出典・ライセンス・
  著作権表示**を製品のライセンス文書（と `NOTICE`）に残す。

## このリポジトリでの扱い

| 場所 | フォント | 配布 |
|---|---|---|
| デモ（`utsuroui-demo`） | `embedded-graphics` 付属のみ | 既定ビルドに含まれる（MIT OR Apache-2.0） |
| ファームウェア（`examples/pico-tft`） | 同上 | 同上 |
| テスト `tests/u8g2_backend.rs` | `u8g2_font_6x10_tf`（`u8g2-fonts` 経由） | **リポジトリでは再配布しない**。テストビルドの依存としてのみ取得 |

`u8g2` feature は API と描画経路の検証のために用意している。デモや
ファームウェアの既定を U8g2 フォントに切り替える場合は、上記の確認と
`NOTICE` の更新をセットで行う。

## 日本語 UI を入れる場合

設計書 §5.3 のとおり、フル CJK フォントはフラッシュに入らない前提で考える。
ビルド時に使用文字列だけを抽出して 1bpp / 4bpp に焼く経路が本命だが、
フォントの再配布条件の確認が必要なため、このリポジトリでは未実装
（`docs/IMPLEMENTATION_PLAN.md` §4 を参照）。
