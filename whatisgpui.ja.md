# GPUI とは？ — Tauri / Swift / Electron との比較

このリポジトリの Todo アプリを題材に、GPUI で作った場合と、Tauri・Swift（SwiftUI）・Electron で作った場合の違いをまとめる。

## GPUI とは

[GPUI](https://www.gpui.rs/) は、エディタ [Zed](https://zed.dev/) のために作られた Rust 製の UI フレームワーク。

- **GPU で直接描画する**：macOS では Metal で描画する。WebView や OS 標準の UI 部品は使わない。
- **Rust だけで書ける**：画面の構造も状態もイベント処理も、すべて Rust のコードで書く。HTML / CSS / JS は使わない。
- **Tailwind 風のスタイル指定**：`div().flex().p_4().bg(rgb(0xffffff))` のように、メソッドチェーンで見た目を指定する。
- **Entity と Context で状態を管理する**：状態は `Entity<T>` に持たせ、`cx.notify()` を呼ぶと再描画される。部品どうしは `EventEmitter` と `cx.subscribe` でやりとりする。
- **部品は最低限しかない**：ボタン、チェックボックス、テキスト入力といった完成品はほとんど用意されていない。`div` に見た目とクリック処理を付けて自分で作る。

## 同じ Todo アプリを作った場合の比較

| 観点 | GPUI | Tauri | Swift（SwiftUI） | Electron |
| --- | --- | --- | --- | --- |
| 言語 | Rust | フロント：HTML/CSS/JS（React 等）<br>バックエンド：Rust | Swift | HTML/CSS/JS（Node.js） |
| 描画方式 | 自前で GPU 描画（Metal 等） | OS の WebView（macOS では WKWebView） | OS ネイティブ（AppKit / UIKit） | 同梱した Chromium |
| 対応 OS | macOS / Linux / Windows | macOS / Linux / Windows / iOS / Android | Apple 製品のみ | macOS / Linux / Windows |
| アプリサイズの目安 | 小さい（数〜十数 MB） | 小さい（数 MB〜） | とても小さい | 大きい（100 MB 前後〜） |
| メモリ使用量 | 少ない | 少なめ（WebView 分は使う） | 少ない | 多い（Chromium + Node） |
| UI 部品 | ほぼ自作 | Web の資産をすべて使える | OS 標準の部品が揃っている | Web の資産をすべて使える |
| テキスト入力 | **自作が必要**（IME 対応も自分で） | `<input>` で済む | `TextField` で済む | `<input>` で済む |
| ホットリロード | なし（watchexec で再起動） | あり（Vite の HMR など） | Xcode Previews | あり（Vite の HMR など） |
| ドキュメント・情報量 | 少ない（Zed のソースと examples が頼り） | 多い | 多い | とても多い |
| 必要な開発環境 | Rust（このリポジトリでは Xcode 不要） | Rust + Node.js | Xcode（ほぼ必須） | Node.js |

※ サイズとメモリの数値は一般的な目安で、このアプリで実測したものではない。

## 各フレームワークで書いた場合のイメージ

### GPUI（このリポジトリ）

```rust
// 入力欄の Enter（Submit イベント）を受けて Todo を追加する
cx.subscribe(&input, |this, input, _: &Submit, cx| {
    let title = input.read(cx).text();
    this.add(title, cx);
    input.update(cx, |input, cx| input.reset(cx));
})
.detach();
```

テキスト入力そのもの（`src/text_input.rs`）は、カーソル描画・範囲選択・IME 対応まで含めて約 750 行ある。gpui 付属の `examples/input.rs` を流用して作った。**Todo アプリなのにコードの大半がテキスト入力**なのが GPUI らしいところ。

### Tauri

```tsx
// フロント（React）。ふつうの Web 開発と同じ
const [todos, setTodos] = useState<Todo[]>([]);
<input onKeyDown={(e) => e.key === "Enter" && add(e.currentTarget.value)} />
```

オンメモリ保存なら Rust 側のコードはほぼ不要で、Web 部分だけで完結する。Rust 側は、ファイル保存や OS の機能を呼ぶときに `#[tauri::command]` として使う。

### Swift（SwiftUI）

```swift
@State private var todos: [Todo] = []
@State private var text = ""

TextField("What needs to be done?", text: $text)
    .onSubmit { todos.append(Todo(title: text)); text = "" }
List($todos) { $todo in Toggle(todo.title, isOn: $todo.done) }
```

数十行で書ける。日本語入力やアクセシビリティ、ダークモードも OS がやってくれる。ただし動くのは Apple 製品だけ。

### Electron

```js
// renderer 側は Tauri とほぼ同じ Web のコード
// main プロセスで BrowserWindow を作って HTML を読み込む
const win = new BrowserWindow({ width: 520, height: 640 });
win.loadFile("index.html");
```

開発体験は一番手軽。ただし Chromium を丸ごと同梱するので、Todo アプリでも配布サイズが 100 MB 前後になる。

## どれを選ぶか

- **GPUI**：Rust だけで、軽くて速いデスクトップアプリを作りたいとき。エディタのように描画性能が重要なアプリに向く。部品を自作する手間と、情報の少なさは覚悟が必要。
- **Tauri**：Web の技術で UI を作りつつ、軽いアプリにしたいとき。モバイルにも対応できる。
- **Swift（SwiftUI）**：Apple 製品だけでよいとき。ネイティブらしい見た目と操作感が一番簡単に手に入る。
- **Electron**：Web の資産や知見をそのまま使い、とにかく早く作りたいとき。VS Code や Slack もこの方式。サイズとメモリは犠牲になる。

## このリポジトリで GPUI を使って分かったこと

- Xcode 本体がないと Metal シェーダのビルドで失敗する → `gpui` の `runtime_shaders` 機能で回避した。
- テキスト入力の部品がないので、`examples/input.rs` を流用した。
- ホットリロードはないので、`watchexec -r -e rs -- cargo run` で保存のたびに再起動している。
- 一方で、ウィンドウの見た目は数十行の `div()` チェーンで思いどおりに作れ、起動も描画も速い。
