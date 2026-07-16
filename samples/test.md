# MD Preview — Test Document

A sample file to exercise the renderer. **Bold**, _italic_, ~~strikethrough~~,
`inline code`, and an autolink: https://tauri.app

> Blockquote with a [named link](https://example.com) and a footnote.[^1]

## Table (GFM)

| Feature      | Supported | Notes                     |
| ------------ | :-------: | ------------------------- |
| Tables       |     ✅     | GitHub-flavored           |
| Task lists   |     ✅     | see below                 |
| Code highlight |   ✅     | via highlight.js          |

## Task list

- [x] Parse Markdown with comrak
- [x] Render GFM extensions
- [ ] Live reload (deferred)

## Code blocks

```rust
fn main() {
    let msg = "hello from Rust";
    println!("{msg}");
}
```

```js
const invoke = window.__TAURI__.core.invoke;
const res = await invoke("get_opened_file");
console.log(res.name);
```

```python
def greet(name: str) -> str:
    return f"hi {name}"
```

## Nested list

1. First
   - sub-item a
   - sub-item b
2. Second

---

Raw HTML should be **escaped**, not executed: <script>alert(1)</script>

[^1]: This is the footnote text.
