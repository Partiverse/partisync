;; clock.read 注权探针（SPEC M7-WP01 §2.2 白名单冻结表 / §3 [P14] 验收）。
;;
;; 生成路径（可复现，wasm-tools 1.259.0 / wit-component 0.259.0 ——
;; ADR-0025 锁定同线）：
;;   1. wit/clock.wit：package `partisync:ext@0.1.0` + interface
;;      `clock { now-millis: func() -> u64 }` + world `clock-probe`
;;      （import clock / export now）
;;   2. core module（canonical ABI 形态：memory + realloc + export
;;      `now` 调 import `partisync:ext/clock@0.1.0` 的 `now-millis`）
;;   3. `wasm-tools component embed --world clock-probe wit core.wasm`
;;      嵌入 WIT 元数据 → `wasm-tools component new` → `wasm-tools print`
;;
;; 双向语义（T03 验收）：
;; - manifest **未声明** clock.read → linker 无 `partisync:ext/clock@0.1.0`
;;   instance → 实例化必拒（[P13] 拒绝面）
;; - manifest **声明** clock.read → 实例化 + 导出 `now()` 调用成功，
;;   返回值是合理毫秒时间戳（[P14] 注入面 = 声明面）
;;
;; 命名空间要点（本 fixture 实证纠正 T02 的 flat-name 假设）：
;; component 的 import 是 **interface 实例**（`import
;; "partisync:ext/clock@0.1.0" (instance ...)`），非扁平函数名。宿主侧
;; 必须 `linker.instance("partisync:ext/clock@0.1.0")` 后在该 instance 内
;; `func_wrap("now-millis", ...)`——根 instance 上注册扁平名永远匹配不上
;; （TypeChecker 要求 `TypeDef::ComponentInstance` 对应
;; `Definition::Instance`，见 wasmtime matching.rs `definition`）。
(component
  (type $ty-partisync:ext/clock@0.1.0 (;0;)
    (instance
      (type (;0;) (func (result u64)))
      (export (;0;) "now-millis" (func (type 0)))
    )
  )
  (import "partisync:ext/clock@0.1.0" (instance $partisync:ext/clock@0.1.0 (;0;) (type $ty-partisync:ext/clock@0.1.0)))
  (core module $main (;0;)
    (type (;0;) (func (result i64)))
    (type (;1;) (func (param i32 i32 i32 i32) (result i32)))
    (import "partisync:ext/clock@0.1.0" "now-millis" (func (;0;) (type 0)))
    (memory (;0;) 1)
    (global (;0;) (mut i32) i32.const 0)
    (export "memory" (memory 0))
    (export "realloc" (func 1))
    (export "now" (func 2))
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
    (func (;2;) (type 0) (result i64)
      call 0
    )
    (@producers
      (processed-by "wit-component" "0.259.0")
    )
  )
  (alias export $partisync:ext/clock@0.1.0 "now-millis" (func $now-millis (;0;)))
  (core func $now-millis (;0;) (canon lower (func $now-millis)))
  (core instance $partisync:ext/clock@0.1.0 (;0;)
    (export "now-millis" (func $now-millis))
  )
  (core instance $main (;1;) (instantiate $main
      (with "partisync:ext/clock@0.1.0" (instance $partisync:ext/clock@0.1.0))
    )
  )
  (alias core export $main "memory" (core memory $memory (;0;)))
  (type (;1;) (func (result u64)))
  (alias core export $main "now" (core func $now (;1;)))
  (func $now (;1;) (type 1) (canon lift (core func $now)))
  (export $"#func2 now" (@name "now") (;2;) "now" (func $now))
  (@producers
    (processed-by "wit-component" "0.259.0")
  )
)
