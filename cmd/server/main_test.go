package main

import (
	"net/http"
	"testing"
	"time"
)

// A2-06：服务端级超时只服务常规请求；上传端点的长预算由 internal/api 自行放宽。
// 本测试锁定服务端侧口径——若有人把常规读超时放大以求"修好慢上传"，
// 等于把慢速攻击面重新打开，必须失败。
func TestHTTPServerTimeouts(t *testing.T) {
	srv := newHTTPServer(":0", http.NewServeMux())

	if srv.ReadHeaderTimeout != readHeaderTimeout {
		t.Fatalf("ReadHeaderTimeout = %v, want %v", srv.ReadHeaderTimeout, readHeaderTimeout)
	}
	if srv.ReadTimeout != readTimeout {
		t.Fatalf("ReadTimeout = %v, want %v", srv.ReadTimeout, readTimeout)
	}
	if srv.WriteTimeout != writeTimeout {
		t.Fatalf("WriteTimeout = %v, want %v", srv.WriteTimeout, writeTimeout)
	}
	if srv.IdleTimeout != idleTimeout {
		t.Fatalf("IdleTimeout = %v, want %v", srv.IdleTimeout, idleTimeout)
	}
	if srv.ReadTimeout > 15*time.Second || srv.ReadHeaderTimeout > 15*time.Second {
		t.Fatalf("regular-request timeouts must stay tight: header=%v read=%v",
			srv.ReadHeaderTimeout, srv.ReadTimeout)
	}
	if srv.Addr != ":0" || srv.Handler == nil {
		t.Fatalf("server not wired: %+v", srv)
	}
}

// A2-03：MAX_UPLOAD_BYTES 解析口径——未设置或非法时回退 0（由 storage 用默认上限）。
func TestUploadLimitFromEnv(t *testing.T) {
	cases := []struct {
		value string
		want  int64
	}{
		{"", 0},
		{"1048576", 1 << 20},
		{"  2048  ", 2048},
		{"0", 0},
		{"-5", 0},
		{"not-a-number", 0},
	}
	for _, tc := range cases {
		t.Setenv("MAX_UPLOAD_BYTES", tc.value)
		if got := uploadLimitFromEnv(); got != tc.want {
			t.Errorf("uploadLimitFromEnv(MAX_UPLOAD_BYTES=%q) = %d, want %d", tc.value, got, tc.want)
		}
	}
}
