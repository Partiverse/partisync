//! 拒权探针 guest：尝试 FS / 网络 / 时钟三类宿主能力。
//! 在「零 import」默认拒权 linker 下该 component **实例化即失败**
//! （wasi:filesystem / wasi:sockets / wasi:clocks 无宿主实现可解析），
//! 由宿主侧 tests/deny_probes.rs [P13] 断言失败且错误不泄露宿主路径/环境。

fn main() {
    let _ = std::fs::read_to_string("/etc/passwd");
    let _ = std::net::TcpStream::connect("127.0.0.1:1");
    let _ = std::time::SystemTime::now();
}
