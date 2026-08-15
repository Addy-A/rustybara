'use strict'

const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')

const { PanelAxis, PipelineHandle, hash_bytes } = require('../pkg-node/rustybara_wasm.js')

const fixture = path.resolve(
  __dirname,
  '../../rustybara/tests/fixtures/pdf_test_data_print_v2.pdf',
)
const input = fs.readFileSync(fixture)

assert.match(hash_bytes(input), /^sha256:[0-9a-f]{64}$/)

const source = new PipelineHandle(input)
const sourcePages = source.page_count()
assert.ok(sourcePages > 0)

const split = source.split_pages_explicit(
  Float64Array.from([261, 265.5, 265.5]),
  PanelAxis.Vertical,
)
assert.equal(split.page_count(), sourcePages * 3)

const output = split.to_pdf_bytes()
assert.equal(Buffer.from(output.subarray(0, 5)).toString('ascii'), '%PDF-')

console.log(`rustybara-wasm Node smoke test passed (${sourcePages} -> ${sourcePages * 3} pages)`)
