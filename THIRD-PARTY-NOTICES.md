# Third-party notices

`capnpc-embedded` is licensed under the MIT License (see `LICENSE`). It bundles
and redistributes third-party components, also under the MIT License, as noted
below. Their copyright notices are reproduced here as required.

---

## Cap'n Proto

The following bundled files are derived from Cap'n Proto
(<https://github.com/capnproto/capnproto>):

- `assets/capnp.wasm` — the Cap'n Proto schema compiler (`capnp` tool), compiled
  to WebAssembly (`wasm32-wasip1`) from the Cap'n Proto C++ sources with a small
  WASI-compatibility patch (see `patches/`).
- `assets/capnp-include/capnp/c++.capnp`
- `assets/capnp-include/capnp/schema.capnp`
- `assets/capnp-include/capnp/stream.capnp`
- `assets/capnp-include/capnp/persistent.capnp`
- `assets/capnp-include/capnp/rpc.capnp`
- `assets/capnp-include/capnp/rpc-twoparty.capnp`

> Copyright (c) 2013-2014 Sandstorm Development Group, Inc. and contributors
>
> Licensed under the MIT License:
>
> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to deal
> in the Software without restriction, including without limitation the rights
> to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all
> copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
> SOFTWARE.

---

## capnproto-rust (rust.capnp)

The following bundled file is from capnproto-rust
(<https://github.com/capnproto/capnproto-rust>):

- `assets/capnp-include/capnp/rust.capnp` — the annotation schema recognized by
  the capnpc-rust code generator.

> Copyright (c) 2013-2016 Sandstorm Development Group, Inc.; David Renshaw; and
> other contributors
>
> Licensed under the MIT License:
>
> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to deal
> in the Software without restriction, including without limitation the rights
> to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all
> copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
> SOFTWARE.
