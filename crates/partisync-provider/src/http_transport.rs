//! OpenDAL HTTP 传输层（ADR-0006 伴生）：reqwest 实现，绕过
//! opendal 0.59.2 打包缺陷（opendal-http-transport-reqwest 钉了不存在的 reqwest
//! ^0.13.4 的前置版本冲突窗口）。API 均经源码核验（HttpTransport trait、
//! HttpBody::new(stream, size)、reqwest bytes_stream）。

use std::time::Duration;

use futures::StreamExt;
use opendal::HttpBody;
use opendal::{HttpTransport, HttpTransporter};

/// 进程级安装（first-wins；幂等）。在 Provider 构造前调用。
pub fn install_default() {
    HttpTransporter::install_default(ReqwestTransport::new());
}

/// 基于 reqwest 的传输（支持 3xx 跟随——trait 契约要求）。
struct ReqwestTransport {
    client: reqwest::Client,
}

fn uerr(e: impl std::fmt::Display) -> opendal::Error {
    opendal::Error::new(opendal::ErrorKind::Unexpected, e.to_string())
}

impl ReqwestTransport {
    fn new() -> Self {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(8))
            .timeout(Duration::from_secs(300))
            .build()
            .expect("reqwest client 构造失败");
        ReqwestTransport { client }
    }
}

impl HttpTransport for ReqwestTransport {
    async fn fetch(
        &self,
        req: http::Request<opendal::Buffer>,
    ) -> opendal::Result<http::Response<HttpBody>> {
        let (parts, body) = req.into_parts();
        let url = parts.uri.to_string();
        let method = reqwest::Method::from_bytes(parts.method.as_str().as_bytes()).map_err(uerr)?;
        let mut rq = self.client.request(method, &url);
        for (k, v) in parts.headers.iter() {
            rq = rq.header(k, v);
        }
        // Buffer → 请求体（小对象内存态；大 body 流式归 M1-WP03 传输优化）
        let bytes = body.to_vec();
        if !bytes.is_empty() {
            rq = rq.body(bytes);
        }
        let resp = rq.send().await.map_err(uerr)?;
        let status = resp.status();
        let mut builder = http::Response::builder()
            .status(http::StatusCode::from_u16(status.as_u16()).map_err(uerr)?);
        for (k, v) in resp.headers().iter() {
            builder = builder.header(k, v);
        }
        let size = resp.content_length();
        let stream = resp
            .bytes_stream()
            .map(|chunk| chunk.map(opendal::Buffer::from).map_err(uerr));
        let http_body = HttpBody::new(stream, size);
        builder.body(http_body).map_err(uerr)
    }
}
