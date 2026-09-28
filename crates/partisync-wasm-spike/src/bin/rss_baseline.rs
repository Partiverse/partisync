//! 空载内存基线：什么也不做的进程，供 `/usr/bin/time -l` 测 peak RSS。
//! 内存增量 = rss_loaded 峰值 − 本进程峰值（SPEC §2.3 指标 3）。
fn main() {}
