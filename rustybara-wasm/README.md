# rustybara-wasm

Typed WebAssembly bindings for the pure-Rust parts of
[RustyBara](https://github.com/Addy-A/rustybara), a prepress-focused PDF toolkit.
The npm package targets Node.js and does not require native libraries.

## Install

```sh
npm install rustybara-wasm
```

## Node.js

```js
const fs = require('node:fs')
const { PanelAxis, PipelineHandle } = require('rustybara-wasm')

const input = fs.readFileSync('input.pdf')
let pdf = new PipelineHandle(input)

pdf = pdf.trim()
pdf = pdf.split_pages_explicit(
  Float64Array.from([261, 265.5, 265.5]),
  PanelAxis.Vertical,
)

fs.writeFileSync('output.pdf', pdf.to_pdf_bytes())
```

Panel sizes are PDF points. Explicit sizes must be positive and sum to the
TrimBox dimension along the selected axis within 0.5 point. Horizontal panels
are returned left-to-right; vertical panels are returned bottom-to-top.

The generated TypeScript declarations are included in the package.

## API

- `new PipelineHandle(bytes)` loads a PDF from `Uint8Array` bytes.
- `page_count()` returns the number of pages.
- `trim()`, `resize()`, and `add_trim_box()` manipulate page geometry.
- `split_pages()` preserves the legacy uniform two-panel split.
- `split_pages_explicit()` splits using explicit panel sizes and an axis.
- `extract_pages()` and `stitch_pages()` reorganize pages.
- `remap_color()` and `detect_color_space()` inspect or replace color values.
- `outline_text()` converts text to vector outlines.
- `hash_bytes()`, `read_xmp_block()`, and `embed_metadata()` support provenance.
- `to_pdf_bytes()` serializes the result to a `Uint8Array`.

Methods that modify a document return a new handle and consume the previous
JavaScript wrapper where documented by the generated TypeScript declarations.

## Build a local Node package

Install `wasm-pack`, then run from the repository root:

```sh
wasm-pack build rustybara-wasm --target nodejs --out-dir pkg-node --release
node rustybara-wasm/tests/node-smoke.cjs
mkdir -p rustybara-wasm/dist
npm pack ./rustybara-wasm/pkg-node --pack-destination ./rustybara-wasm/dist
```

The publish workflow uses tags named `rustybara-wasm-v<version>`, such as
`rustybara-wasm-v0.2.0`.

## License

LGPL-3.0-only. See the repository's `LICENSE-LGPL-3.0` file.
