title: sqlx 0.9 动态 SQL 实战
filename: sqlx_dynamic_sql_guide.md
tags: [rust, sqlx, sql, database, async]
updated_ns: 1704412800000000000

# sqlx 0.9 动态 SQL 实战

## 问题

`sqlx::query` 只接受 `&'static str`；动态拼表名/列名时编译期检查失效。

## 解决：AssertSqlSafe

```rust
use sqlx::AssertSqlSafe;

let table = "content";
let sql = format!("SELECT * FROM {table} WHERE id = ?");
let q = sqlx::query_as::<_, ContentRow>(&*sqlx::AssertSqlSafe(sql));
q.bind(content_id).fetch_one(pool).await?;
```

`AssertSqlSafe(String)` 是 wrapper，**仅当你 100% 信任来源时使用**——
表/列名走白名单（不在用户输入路径上）。

## 白名单模式

```rust
const ALLOWED_TABLES: &[&str] = &["content", "entry", "tag"];
let table = pick_table(user_input)?;  // 返回 &'static str
```

## 编译期检查的边界

| 字段 | 编译期检查 | 备注 |
|---|---|---|
| 表名 | ❌ | 动态 |
| 列名 | ❌ | 动态 |
| 字面量 | ✅ | 写死在 SQL |
| 参数值 | ✅ | bind() |

## PartiSync 用法

- `sidecar_items` 查询：表名固定
- `entry_tag` JOIN：表名固定
- 仅审计日志动态 INSERT：白名单表名

## 经验教训

不要因为「动态 SQL 麻烦」就拼字符串——SQL injection 是 OWASP Top 1。