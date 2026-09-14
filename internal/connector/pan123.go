package connector

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"math"
	"math/rand"
	"net/http"
	"net/url"
	"os"
	"path"
	"strconv"
	"strings"
	"sync"
	"time"
)

// Pan123Config 123pan 原生 API 配置。
type Pan123Config struct {
	Username string
	Password string
	RootID   string // 起始目录 FileId，默认 "0"（根目录）
	Timeout  time.Duration
}

// Pan123Client 123pan 原生 API 客户端。
type Pan123Client struct {
	config Pan123Config
	client *http.Client
	token  string
}

// API 端点
const pan123API      = "https://www.123pan.com"
const pan123LoginAPI = "https://login.123pan.com"
const pan123FileList = "/b/api/file/list/new"
const pan123DownloadInfo = "/b/api/file/download_info"
const pan123UserInfo = "/b/api/user/info"
const pan123SignIn   = "/api/user/sign_in"

// 响应结构
type loginResp struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
	Data    struct {
		Token string `json:"token"`
	} `json:"data"`
}

type userInfoResp struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
	Data    struct {
		UID       int64  `json:"UID"`
		Nickname  string `json:"Nickname"`
		SpaceUsed int64  `json:"SpaceUsed"`
		FileCount int    `json:"FileCount"`
	} `json:"data"`
}

type fileListResp struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
	Data    struct {
		InfoList []panFile `json:"InfoList"`
		Next     string    `json:"Next"`  // "-1" = 无更多
		Total    int       `json:"Total"`
	} `json:"data"`
}

type downloadInfoResp struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
	Data    struct {
		DownloadUrl string `json:"DownloadUrl"`
	} `json:"data"`
}

// panFile 文件对象（与 API JSON 字段对应）。
type panFile struct {
	FileID   int64  `json:"FileId"`
	FileName string `json:"FileName"`
	Size     int64  `json:"Size"`
	Type     int    `json:"Type"` // 0=文件，1=目录
	ParentID int64  `json:"ParentFileId"`
	UpdateAt int64  `json:"UpdateAt"`
	Etag     string `json:"Etag"`
}

// NewPan123Client 创建客户端。
func NewPan123Client(cfg Pan123Config) (*Pan123Client, error) {
	t := cfg.Timeout
	if t == 0 {
		t = 30 * time.Second
	}
	return &Pan123Client{
		config: cfg,
		client: &http.Client{Timeout: t},
	}, nil
}

// Probe 测试连接。
func (c *Pan123Client) Probe(ctx context.Context) error {
	if err := c.login(ctx); err != nil {
		return fmt.Errorf("login failed: %w", err)
	}
	_, err := c.getUserInfo(ctx)
	return err
}

// login 获取 token。
func (c *Pan123Client) login(ctx context.Context) error {
	if c.token != "" {
		return nil
	}
	loginURL := pan123LoginAPI + pan123SignIn
	var body map[string]any
	if strings.Contains(c.config.Username, "@") {
		body = map[string]any{"mail": c.config.Username, "password": c.config.Password, "type": 2}
	} else {
		body = map[string]any{"passport": c.config.Username, "password": c.config.Password, "remember": true}
	}
	b, _ := json.Marshal(body)
	req, err := http.NewRequestWithContext(ctx, "POST", loginURL, strings.NewReader(string(b)))
	if err != nil {
		return err
	}
	// 设置请求头
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("origin", pan123API)
	req.Header.Set("referer", pan123API+"/")
	req.Header.Set("platform", "web")
	req.Header.Set("app-version", "3")
	req.Header.Set("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) partisync/1.0")

	resp, err := c.client.Do(req)
	if err != nil {
		return fmt.Errorf("login request: %w", err)
	}
	defer resp.Body.Close()

	respB, _ := io.ReadAll(resp.Body)
	var lr loginResp
	if err := json.Unmarshal(respB, &lr); err != nil {
		return fmt.Errorf("parse login response: %w", err)
	}
	if lr.Code != 200 {
		return fmt.Errorf("login failed: %s", lr.Message)
	}
	c.token = lr.Data.Token
	return nil
}

// getUserInfo 获取用户信息。
func (c *Pan123Client) getUserInfo(ctx context.Context) (*userInfoResp, error) {
	apiURL := pan123API + pan123UserInfo
	req, err := http.NewRequestWithContext(ctx, "GET", c.signAPI(apiURL), nil)
	if err != nil {
		return nil, err
	}
	c.setHeader(req, "GET")

	var resp userInfoResp
	httpResp, err := c.client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("user info: %w", err)
	}
	defer httpResp.Body.Close()
	body, _ := io.ReadAll(httpResp.Body)
	if err := json.Unmarshal(body, &resp); err != nil {
		return nil, fmt.Errorf("parse user info: %w", err)
	}
	if resp.Code != 200 {
		return nil, fmt.Errorf("get user info: %s", resp.Message)
	}
	return &resp, nil
}

// signAPI 给 URL 追加签名参数防频率限制。
func (c *Pan123Client) signAPI(rawURL string) string {
	k, v := panSign(rawURL)
	if strings.Contains(rawURL, "?") {
		rawURL += "&" + k + "=" + v
	} else {
		rawURL += "?" + k + "=" + v
	}
	return rawURL
}

// setHeader 设置通用请求头。
func (c *Pan123Client) setHeader(req *http.Request, method string) {
	if c.token != "" {
		req.Header.Set("authorization", "Bearer "+c.token)
	}
	req.Header.Set("origin", pan123API)
	req.Header.Set("referer", pan123API+"/")
	req.Header.Set("platform", "web")
	req.Header.Set("app-version", "3")
	req.Header.Set("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) partisync/1.0")
	if method == "POST" {
		req.Header.Set("Content-Type", "application/json")
	}
}

// listFilesAt 列出指定父目录下的内容（分页）。
func (c *Pan123Client) listFilesAt(ctx context.Context, parentID string, page int) (*fileListResp, error) {
	apiURL := pan123API + pan123FileList
	req, err := http.NewRequestWithContext(ctx, "GET", c.signAPI(apiURL), nil)
	if err != nil {
		return nil, err
	}
	c.setHeader(req, "GET")

	q := req.URL.Query()
	q.Set("driveId", "0")
	q.Set("limit", "100")
	q.Set("next", "0")
	q.Set("orderBy", "file_id")
	q.Set("orderDirection", "desc")
	q.Set("parentFileId", parentID)
	q.Set("trashed", "false")
	q.Set("Page", strconv.Itoa(page))
	q.Set("OnlyLookAbnormalFile", "0")
	q.Set("event", "homeListFile")
	q.Set("operateType", "4")
	q.Set("inDirectSpace", "false")
	req.URL.RawQuery = q.Encode()

	httpResp, err := c.client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("list request: %w", err)
	}
	defer httpResp.Body.Close()

	body, err := io.ReadAll(httpResp.Body)
	if err != nil {
		return nil, fmt.Errorf("read list response: %w", err)
	}

	var resp fileListResp
	if err := json.Unmarshal(body, &resp); err != nil {
		return nil, fmt.Errorf("parse list response: %w", err)
	}
	if resp.Code != 0 && resp.Code != 200 {
		return nil, fmt.Errorf("list files: %s", resp.Message)
	}
	return &resp, nil
}

// ListFiles 递归列出所有文件（并发加速）。
func (c *Pan123Client) ListFiles(ctx context.Context) ([]FileInfo, error) {
	rootID := c.config.RootID
	if rootID == "" {
		rootID = "0"
	}

	dirMap := sync.Map{} // dirID -> dirPath (带尾斜杠)
	fileCh := make(chan FileInfo, 50000)
	errCh := make(chan error, 100)
	var wg sync.WaitGroup

	dirMap.Store(rootID, "") // 根目录路径为空
	wg.Add(1)
	go c.recurseDir(ctx, rootID, "", &dirMap, fileCh, errCh, &wg)

	var allFiles []FileInfo
	var errs []error
	go func() {
		wg.Wait()
		close(fileCh)
		close(errCh)
	}()

	for f := range fileCh {
		allFiles = append(allFiles, f)
	}
	for err := range errCh {
		errs = append(errs, err)
	}
	if len(errs) > 0 {
		return allFiles, fmt.Errorf("list errors: %v", errs)
	}
	return allFiles, nil
}

// recurseDir 递归处理目录（并发）。
func (c *Pan123Client) recurseDir(ctx context.Context, dirID, dirPath string, dirMap *sync.Map, fileCh chan<- FileInfo, errCh chan<- error, wg *sync.WaitGroup) {
	defer wg.Done()
	defer func() {
		if r := recover(); r != nil {
			errCh <- fmt.Errorf("recover recurseDir: %v", r)
		}
	}()

	page := 1
	for {
		resp, err := c.listFilesAt(ctx, dirID, page)
		if err != nil {
			if strings.Contains(err.Error(), "401") || strings.Contains(err.Error(), "Unauthorized") {
				c.token = ""
				if loginErr := c.login(ctx); loginErr == nil {
					resp, err = c.listFilesAt(ctx, dirID, page)
				}
			}
			if err != nil {
				errCh <- fmt.Errorf("list dir=%s page=%d: %w", dirID, page, err)
				return
			}
		}

		for i := range resp.Data.InfoList {
			f := &resp.Data.InfoList[i]
			relPath := f.FileName
			if dirPath != "" {
				relPath = path.Join(dirPath, f.FileName)
			}

			if f.Type == 1 {
				// 目录：启动新的递归 goroutine
				childID := strconv.FormatInt(f.FileID, 10)
				childPath := relPath + "/"
				dirMap.Store(childID, childPath)
				wg.Add(1)
				go func(cid, cpath string) {
					c.recurseDir(ctx, cid, cpath, dirMap, fileCh, errCh, wg)
					wg.Done()
				}(childID, childPath)
			} else {
				fileCh <- FileInfo{
					Path:     relPath,
					Name:     f.FileName,
					Size:     f.Size,
					MimeType: MimeTypeFromName(f.FileName),
				}
			}
		}

		if resp.Data.Next == "-1" || len(resp.Data.InfoList) == 0 {
			break
		}
		page++

		// 频率限制
		time.Sleep(750 * time.Millisecond)
	}
}

// DownloadToTemp 下载文件到临时目录（暂不支持通过路径下载）。
func (c *Pan123Client) DownloadToTemp(ctx context.Context, filePath string) (string, error) {
	return "", errors.New("123pan download not implemented: scan only mode")
}

// —— 签名算法（与 OpenList 123pan driver 一致）——

func panSign(rawURL string) (key, value string) {
	table := []byte{'a', 'd', 'e', 'f', 'g', 'h', 'l', 'm', 'y', 'i', 'j', 'n', 'o', 'p', 'k', 'q', 'r', 's', 't', 'u', 'b', 'c', 'v', 'w', 's', 'z'}
	random := fmt.Sprintf("%.f", math.Round(1e7*rand.Float64()))
	now := time.Now().In(time.FixedZone("CST", 8*3600))
	timestamp := fmt.Sprint(now.Unix())
	nowStr := []byte(now.Format("200601021504"))
	for i := range nowStr {
		nowStr[i] = table[nowStr[i]-48]
	}

	u, _ := url.Parse(rawURL)
	timeSign := fmt.Sprint(crc32Checksum(nowStr))
	pathStr := u.Path
	data := strings.Join([]string{timestamp, random, pathStr, "web", "3", timeSign}, "|")
	dataSign := fmt.Sprint(crc32Checksum([]byte(data)))
	return timeSign, strings.Join([]string{timestamp, random, dataSign}, "-")
}

func crc32Checksum(data []byte) uint32 {
	const polynomial = 0xEDB88320
	table := [256]uint32{}
	for i := range table {
		crc := uint32(i)
		for j := 0; j < 8; j++ {
			if crc&1 == 1 {
				crc = (crc >> 1) ^ polynomial
			} else {
				crc >>= 1
			}
		}
		table[i] = crc
	}
	var crc uint32 = 0xFFFFFFFF
	for _, b := range data {
		crc = (crc >> 8) ^ table[(crc^uint32(b))&0xFF]
	}
	return crc ^ 0xFFFFFFFF
}

// downloadInfo 获取文件下载 URL。
func (c *Pan123Client) downloadInfo(ctx context.Context, fileID int64, fileName string, size int64, fileType int, etag string) (string, error) {
	apiURL := pan123API + pan123DownloadInfo
	b, _ := json.Marshal(map[string]any{
		"driveId": 0, "etag": etag, "fileId": fileID,
		"fileName": fileName, "s3keyFlag": "", "size": size, "type": fileType,
	})
	req, err := http.NewRequestWithContext(ctx, "POST", c.signAPI(apiURL), strings.NewReader(string(b)))
	if err != nil {
		return "", err
	}
	c.setHeader(req, "POST")

	var resp downloadInfoResp
	httpResp, err := c.client.Do(req)
	if err != nil {
		return "", fmt.Errorf("download info: %w", err)
	}
	defer httpResp.Body.Close()
	body, _ := io.ReadAll(httpResp.Body)
	if err := json.Unmarshal(body, &resp); err != nil {
		return "", fmt.Errorf("parse download info: %w", err)
	}
	if resp.Code != 0 && resp.Code != 200 {
		return "", fmt.Errorf("download info: %s", resp.Message)
	}
	return resp.Data.DownloadUrl, nil
}

// downloadFile 从 URL 下载文件到临时路径。
func (c *Pan123Client) downloadFile(ctx context.Context, downloadURL, tempPath string) error {
	req, err := http.NewRequestWithContext(ctx, "GET", downloadURL, nil)
	if err != nil {
		return err
	}
	req.Header.Set("Referer", pan123API+"/")

	httpResp, err := c.client.Do(req)
	if err != nil {
		return fmt.Errorf("download: %w", err)
	}
	defer httpResp.Body.Close()

	if httpResp.StatusCode != 200 {
		return fmt.Errorf("download status %d", httpResp.StatusCode)
	}

	f, err := os.Create(tempPath)
	if err != nil {
		return fmt.Errorf("create temp file: %w", err)
	}
	defer f.Close()

	_, err = io.Copy(f, httpResp.Body)
	return err
}

func tempFileName(relPath, token string) string {
	h := sha256.Sum256([]byte(relPath + token))
	return hex.EncodeToString(h[:16])
}
