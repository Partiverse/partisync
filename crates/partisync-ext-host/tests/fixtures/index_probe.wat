;; index.read 注权探针（SPEC M7-WP01 §2.2 白名单冻结表 / T04 接线）。
;;
;; 生成路径（可复现，wasm-tools 1.259.0 / wit-component 0.259.0 ——
;; ADR-0025 锁定同线）：
;;   1. `ext.wit`（同目录）：package `partisync:ext@0.1.0` + interface
;;      `index { search: func(query: string) -> string }` + world
;;      `index-probe`（import index / export search）
;;   2. core module（canonical ABI 形态）：
;;      - import `partisync:ext/index@0.1.0` 的 `search`（canonical
;;        签名 (ptr,len,retptr) -> ()，结果字符串写入 retarea）
;;      - export `search` (ptr,len) -> retptr：分配 8 字节 retarea
;;        （canonical lift 的 (ptr,len) 对）后转发宿主
;;      - export `realloc` + **`cabi_realloc`**（canonical ABI 必需导出，
;;        缺则 component new 报 "module does not export cabi_realloc"）
;;   3. `wasm-tools component embed --world index-probe wit core.wasm`
;;      → `wasm-tools component new` → `wasm-tools print`
;;
;; 语义：注权 `index.read` 后可调用宿主注入的 `IndexRead::search`
;; （JSON 进 JSON 出）；未注权则 import 不可解析、实例化必拒。
;;
;; 复现注意（对抗审查 F-5 实测）：`search(ptr,len) -> retptr` 里 query 串
;; 的取用有两种语义等价写法——本文件用**调用方传入的 ptr**
;; （`local.get 0`），按上文配方重新生成会得到 realloc 后的 bump 指针
;; （`global.get 0`）。两者都产出合法 core module 且测试行为一致（retarea
;; 固定于 16，入参串由 wasmtime 经 `cabi_realloc` bump 到其之后，不重叠），
;; 但不能逐字节复现本文件——审查员已实测确认语义等价。
;; 另：`realloc` 导出冗余（仅 `cabi_realloc` 即足够，审查实测），保留无害。
(component
  (type $ty-partisync:ext/index@0.1.0 (;0;)
    (instance
      (type (;0;) (func (param "query" string) (result string)))
      (export (;0;) "search" (func (type 0)))
    )
  )
  (import "partisync:ext/index@0.1.0" (instance $partisync:ext/index@0.1.0 (;0;) (type $ty-partisync:ext/index@0.1.0)))
  (core module $main (;0;)
    (type (;0;) (func (param i32 i32 i32)))
    (type (;1;) (func (param i32 i32 i32 i32) (result i32)))
    (type (;2;) (func (param i32 i32) (result i32)))
    (import "partisync:ext/index@0.1.0" "search" (func (;0;) (type 0)))
    (memory (;0;) 1)
    (global (;0;) (mut i32) i32.const 16)
    (export "memory" (memory 0))
    (export "realloc" (func 1))
    (export "cabi_realloc" (func 1))
    (export "search" (func 2))
    (func (;1;) (type 1) (param i32 i32 i32 i32) (result i32)
      (local i32)
      global.get 0
      local.set 4
      global.get 0
      local.get 1
      i32.add
      global.set 0
      local.get 4
    )
    (func (;2;) (type 2) (param i32 i32) (result i32)
      (local i32)
      i32.const 0
      i32.const 8
      i32.const 1
      i32.const 8
      call 1
      local.set 2
      local.get 0
      local.get 1
      local.get 2
      call 0
      local.get 2
    )
    (@producers
      (processed-by "wit-component" "0.259.0")
    )
  )
  (core module $wit-component-shim-module (;1;)
    (type (;0;) (func (param i32 i32 i32)))
    (table (;0;) 1 1 funcref)
    (export "0" (func $indirect-partisync:ext/index@0.1.0-search))
    (export "$imports" (table 0))
    (func $indirect-partisync:ext/index@0.1.0-search (;0;) (type 0) (param i32 i32 i32)
      local.get 0
      local.get 1
      local.get 2
      i32.const 0
      call_indirect (type 0)
    )
    (@producers
      (processed-by "wit-component" "0.259.0")
    )
  )
  (core instance $wit-component-shim-instance (;0;) (instantiate $wit-component-shim-module))
  (alias core export $wit-component-shim-instance "0" (core func $indirect-partisync:ext/index@0.1.0-search (;0;)))
  (core instance $partisync:ext/index@0.1.0 (;1;)
    (export "search" (func $indirect-partisync:ext/index@0.1.0-search))
  )
  (core instance $main (;2;) (instantiate $main
      (with "partisync:ext/index@0.1.0" (instance $partisync:ext/index@0.1.0))
    )
  )
  (alias core export $main "memory" (core memory $memory (;0;)))
  (core module $wit-component-fixup (;2;)
    (type (;0;) (func (param i32 i32 i32)))
    (import "actual" "0" (func $0 (;0;) (type 0)))
    (import "shim" "$imports" (table (;0;) 1 1 funcref))
    (elem (;0;) (i32.const 0) func $0)
    (@producers
      (processed-by "wit-component" "0.259.0")
    )
  )
  (alias export $partisync:ext/index@0.1.0 "search" (func $search (;0;)))
  (alias core export $main "cabi_realloc" (core func $cabi_realloc (;1;)))
  (core func $"#core-func2 indirect-partisync:ext/index@0.1.0-search" (@name "indirect-partisync:ext/index@0.1.0-search") (;2;) (canon lower (func $search) (memory $memory) (realloc $cabi_realloc) string-encoding=utf8))
  (core instance $actual (;3;)
    (export "0" (func $"#core-func2 indirect-partisync:ext/index@0.1.0-search"))
  )
  (core instance $fixup (;4;) (instantiate $wit-component-fixup
      (with "actual" (instance $actual))
      (with "shim" (instance $wit-component-shim-instance))
    )
  )
  (type (;1;) (func (param "query" string) (result string)))
  (alias core export $main "search" (core func $search (;3;)))
  (func $"#func1 search" (@name "search") (;1;) (type 1) (canon lift (core func $search) (memory $memory) (realloc $cabi_realloc) string-encoding=utf8))
  (export $"#func2 search" (@name "search") (;2;) "search" (func $"#func1 search"))
  (@producers
    (processed-by "wit-component" "0.259.0")
  )
)
