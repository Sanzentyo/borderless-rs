# Window Visual Capture Investigation Request

## Goal

Borderless Oxide の GUI に、対象ウィンドウの実アイコンとプレビューを表示したい。
特に、Windows タスクバーでアプリにホバーしたときに表示されるライブプレビュー相当の映像を、GUI 内に表示できるか調査する。

## Questions

1. `DwmRegisterThumbnail` / `DwmUpdateThumbnailProperties` を使って、対象ウィンドウのライブサムネイルを WinUI 3 / windows-reactor アプリ内の任意領域へ描画できるか。
2. DWM サムネイルを「画像ファイル」や `BitmapImage` 用のバイト列として取得する公開 API があるか。
3. `Windows.Graphics.Capture` を使った場合、ゲームウィンドウ、DirectX ウィンドウ、最小化ウィンドウ、非アクティブウィンドウでどの程度キャプチャできるか。
4. `PrintWindow` / `BitBlt` と比べて、Windows.Graphics.Capture の初回許可 UI、性能、黒画面率、実装複雑度はどうか。
5. windows-reactor で DWM サムネイルや DirectComposition surface をホストするために必要な拡張点はどこか。

## Current Assumptions

- Win32 の `WM_GETICON` / class icon から対象ウィンドウのアイコンは取得できる。
- `PrintWindow` / `BitBlt` は静止画プレビューの初期実装として使えるが、DirectX ゲームでは黒画面や古いフレームになる可能性が高い。
- DWM タスクバーサムネイル相当のライブ表示は、画像として取り出すより、自アプリの HWND に DWM thumbnail を描画させる方向が現実的。

## Current Product Decision

- `PrintWindow` / `BitBlt` 由来のプレビューは、黒画像や中途半端な描画になりやすいため GUI では表示しない。
- アイコンは `WM_GETICON` / class icon / exe fallback から取得し、alpha 付き画像をメモリ上の data URI として表示する。画像ファイルのキャッシュは使わない。WinUI の URI 画像読み込みが data URI を受けない環境が見つかった場合は、ファイルキャッシュへ戻すより Reactor 側に stream/pixel source を追加する。
- 現在の `windows-reactor::Image` は URI 文字列を WinUI `BitmapImage.UriSource` に渡す経路なので、`HICON` や BGRA ピクセルを PNG 化せず直接表示するには Reactor 側に `WriteableBitmap` / pixel buffer を受け取る image source API が必要。
- プレビューを再開する場合は、壊れた静止画を出すのではなく、DWM thumbnail または Windows Graphics Capture の PoC 結果を待つ。

## Deliverable

- 推奨方式を 1 つ選び、理由と制約をまとめる。
- 必要なら小さな Rust/Win32 PoC を作り、対象 HWND のライブサムネイルまたは静止画キャプチャが GUI 内に表示できることを確認する。
- Borderless Oxide の現行構成に入れる場合の module/API 境界案を書く。
