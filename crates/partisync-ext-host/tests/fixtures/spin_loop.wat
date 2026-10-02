;; spin_loop 死循环探针（SPEC M8-WP06 §3：epoch/fuel 终止验收 fixture）。
;;
;; 形态：world `partisync:demo/demo-tool` 同构（export
;; `call(input: string) -> string`），canonical ABI 手写——core `call`
;; 无条件 `loop (br)` 死循环（永不返回；尾随 unreachable 满足类型校验）。
;; 源码形态入仓（沿 clock_probe.wat 判例：可读可审可 diff）。
;;
;; 用途：
;; - epoch 探针：调用在预算内以 epoch deadline trap 终止（真终止），
;;   对照 M8-WP06 §1 timeout 假终止（线程永占）；
;; - 线程释放：trap 返回后 spawn_blocking 线程可复用。
;;
;; realloc：bump 分配器（clock_probe 同构）；死循环路径不触达输出。
(component
  (core module $main (;0;)
    (type (;0;) (func (param i32 i32 i32 i32) (result i32)))
    (type (;1;) (func (param i32 i32) (result i32)))
    (memory (;0;) 1)
    (global (;0;) (mut i32) i32.const 0)
    (export "memory" (memory 0))
    (export "realloc" (func 0))
    (export "call" (func 1))
    (func (;0;) (type 0) (param i32 i32 i32 i32) (result i32)
      (local i32)
      global.get 0
      local.set 4
      global.get 0
      local.get 1
      i32.add
      global.set 0
      local.get 4
    )
    (func (;1;) (type 1) (param i32 i32) (result i32)
      (loop $l
        (br $l)
      )
      unreachable
    )
    (@producers
      (processed-by "hand-written" "M8-WP06-T01")
    )
  )
  (core instance $main (;0;) (instantiate $main))
  (alias core export $main "memory" (core memory $memory (;0;)))
  (alias core export $main "realloc" (core func $realloc (;0;)))
  (alias core export $main "call" (core func $call (;1;)))
  (type (;0;) (func (param "input" string) (result string)))
  (func $call-export (;0;) (type 0) (canon lift
    (core func $call)
    (memory $memory)
    (realloc (core func $realloc))
    string-encoding=utf8
  ))
  (export $"#func0 call" (@name "call") (;1;) "call" (func $call-export))
  (@producers
    (processed-by "hand-written" "M8-WP06-T01")
  )
)
