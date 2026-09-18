//! xtask — 开发自动化：追溯与里程碑报告（SPEC docs/specs/M-1-WP05.md）。
//!
//! 用法：
//!   cargo xtask trace <TASK-ID>     打印挂接该任务的全部提交（含 trailer 工件引用与触及文件）
//!   cargo xtask report <MILESTONE>  汇总该里程碑的提交/任务/AI 披露统计，
//!                                   并在 `docs/reports/<M>-report.md` 缺失时生成报告骨架
//!
//! 追溯数据源是 git 提交 trailer（Task-ID/Spec/AI-Assist/AI-Review/Reviewed-By），
//! 见 AGENTS.md「提交与追溯格式」。trailer 提取为启发式（每种取最后一次出现），
//! 对 squash 合并的线性历史足够可靠。

use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::{Command, Output};

/// 一条提交的追溯视图。
#[derive(Debug, Clone, PartialEq, Eq)]
struct CommitRecord {
    hash: String,
    subject: String,
    task_id: Option<String>,
    spec: Option<String>,
    ai_assist: Option<String>,
    ai_review: Option<String>,
    reviewed_by: Option<String>,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [cmd, arg] if cmd == "trace" => exit_with(trace(arg)),
        [cmd, arg] if cmd == "report" => exit_with(report(arg)),
        _ => {
            eprintln!("usage: cargo xtask <trace <TASK-ID> | report <MILESTONE>>");
            std::process::exit(2);
        }
    }
}

fn exit_with(ok: bool) {
    std::process::exit(i32::from(!ok));
}

fn trace(task_id: &str) -> bool {
    let Some(records) = git_log_records() else {
        eprintln!("error: git log 失败（非 git 仓库？）");
        return false;
    };
    let hits: Vec<&CommitRecord> = records
        .iter()
        .filter(|r| r.task_id.as_deref() == Some(task_id))
        .collect();
    if hits.is_empty() {
        eprintln!("no commits traced to {task_id}");
        return false;
    }
    println!("== trace: {task_id}（{} commits）==", hits.len());
    for r in hits {
        println!();
        println!("commit  {} {}", &r.hash[..8], r.subject);
        if let Some(s) = &r.spec {
            println!("spec    {s}");
        }
        if let Some(a) = &r.ai_assist {
            println!("ai      {a}");
        }
        if let Some(v) = &r.ai_review {
            println!("review  {v}");
        }
        if let Some(h) = &r.reviewed_by {
            println!("human   {h}");
        }
        println!("files:");
        for f in files_of_commit(&r.hash) {
            println!("  {f}");
        }
    }
    true
}

fn report(milestone: &str) -> bool {
    let Some(records) = git_log_records() else {
        eprintln!("error: git log 失败（非 git 仓库？）");
        return false;
    };
    let prefix = format!("{milestone}-");
    let scope: Vec<&CommitRecord> = records
        .iter()
        .filter(|r| r.task_id.as_deref().is_some_and(|t| t.starts_with(&prefix)))
        .collect();
    let total = scope.len();
    let mut tasks: Vec<&str> = scope.iter().filter_map(|r| r.task_id.as_deref()).collect();
    tasks.sort_unstable();
    tasks.dedup();
    let mut wps: Vec<String> = tasks
        .iter()
        .filter_map(|t| wp_of(t).map(String::from))
        .collect();
    wps.sort();
    wps.dedup();
    let ai = scope.iter().filter(|r| r.ai_assist.is_some()).count();
    let reviewed = scope.iter().filter(|r| r.reviewed_by.is_some()).count();

    let mut stats = String::new();
    let _ = writeln!(stats, "- 挂接 {prefix}* 任务的提交数：{total}");
    let _ = writeln!(stats, "- 任务数：{}", tasks.len());
    let _ = writeln!(stats, "- 工作包分布：{}", wps.join(", "));
    let _ = writeln!(stats, "- AI 辅助提交（AI-Assist）：{ai}/{total}");
    let _ = writeln!(stats, "- 人工终审提交（Reviewed-By）：{reviewed}/{total}");
    for t in &tasks {
        let n = scope
            .iter()
            .filter(|r| r.task_id.as_deref() == Some(t))
            .count();
        let _ = writeln!(stats, "  - {t}: {n} commit(s)");
    }
    print!("{stats}");

    let path = PathBuf::from("docs/reports").join(format!("{milestone}-report.md"));
    if path.exists() {
        println!("\nreport 已存在，未覆盖：{}", path.display());
        return true;
    }
    let skeleton = report_skeleton(milestone, &stats);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&path, skeleton) {
        eprintln!("error: 无法写入 {}: {e}", path.display());
        return false;
    }
    println!("\nreport 骨架已生成：{}", path.display());
    true
}

fn report_skeleton(milestone: &str, stats: &str) -> String {
    format!(
        "# {milestone} 里程碑报告\n\
         \n\
         生成：`cargo xtask report {milestone}`（骨架，人工部分见各节注释）\n\
         \n\
         ## 1. 范围与结果（对照 SPEC 汇总；范围变更记录）\n\
         <!-- 人工填写：逐工作包对照 SPEC 验收标准 -->\n\
         \n\
         ## 2. KPI 达标表（基准报告链接）\n\
         <!-- 基准基建（M-1-WP04）落地后自动填充；此前手工附 criterion 输出 -->\n\
         \n\
         ## 3. 测试证据（覆盖率/属性测试/变异分数/模糊时长/混沌/互操作）\n\
         <!-- 附 CI run 链接与本地验证输出 -->\n\
         \n\
         ## 4. 安全（cargo audit / deny / unsafe 增量 / 外部审计）\n\
         <!-- -->\n\
         \n\
         ## 5. ADR 清单与债务登记\n\
         <!-- -->\n\
         \n\
         ## 6. AI 使用披露（自动统计）\n\
         {stats}\n\
         ## 7. 抽查审计记录（随机 5 任务，仅凭工件重建故事）\n\
         <!-- 审计演练记录见 docs/reports/M-1-audit-rehearsal.md -->\n\
         \n\
         ## 8. 下一阶段建议\n\
         <!-- -->\n\
         \n\
         ## 放行签字（G3）\n\
         \n\
         - [ ] 架构负责人：\n\
         - [ ] 评审人：\n\
         - [ ] 安全负责人（M2/M4）：\n"
    )
}

fn git(args: &[&str]) -> Option<String> {
    let Output { status, stdout, .. } = Command::new("git").args(args).output().ok()?;
    if status.success() {
        Some(String::from_utf8_lossy(&stdout).into_owned())
    } else {
        None
    }
}

fn git_log_records() -> Option<Vec<CommitRecord>> {
    let raw = git(&["log", "--all", "--pretty=format:%H%x1f%s%x1f%b%x1e"])?;
    Some(parse_commits(&raw))
}

fn files_of_commit(hash: &str) -> Vec<String> {
    git(&["show", "--pretty=format:", "--name-only", hash])
        .map(|s| {
            s.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

fn parse_commits(raw: &str) -> Vec<CommitRecord> {
    raw.split('\x1e')
        .filter(|rec| !rec.trim().is_empty())
        .map(|rec| {
            let mut fields = rec.trim_matches('\n').splitn(3, '\x1f');
            let hash = fields.next().unwrap_or_default().trim().to_string();
            let subject = fields.next().unwrap_or_default().trim().to_string();
            let body = fields.next().unwrap_or_default().to_string();
            CommitRecord {
                hash,
                subject,
                task_id: trailer(&body, "Task-ID"),
                spec: trailer(&body, "Spec"),
                ai_assist: trailer(&body, "AI-Assist"),
                ai_review: trailer(&body, "AI-Review"),
                reviewed_by: trailer(&body, "Reviewed-By"),
            }
        })
        .collect()
}

fn trailer(body: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    body.lines()
        .rev()
        .find_map(|l| l.strip_prefix(&prefix).map(|v| v.trim().to_string()))
}

fn wp_of(task_id: &str) -> Option<&str> {
    task_id.rsplit_once("-T").map(|(wp, _)| wp)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "abc123\x1fchore(repo): bootstrap [M-1-WP01-T01]\x1f\n\nTask-ID: M-1-WP01-T01\nSpec: docs/specs/M-1-WP01.md\nAI-Assist: zcode/GLM-5.3 (scaffold)\n\x1edeff456\x1ffix(core): ulid carry edge\x1f\n\nTask-ID: M-1-WP07-T01\n\x1e";

    #[test]
    fn parses_records_and_trailers() {
        let rs = parse_commits(SAMPLE);
        assert_eq!(rs.len(), 2);
        assert_eq!(rs[0].hash, "abc123");
        assert_eq!(rs[0].subject, "chore(repo): bootstrap [M-1-WP01-T01]");
        assert_eq!(rs[0].task_id.as_deref(), Some("M-1-WP01-T01"));
        assert_eq!(rs[0].spec.as_deref(), Some("docs/specs/M-1-WP01.md"));
        assert_eq!(rs[0].ai_assist.as_deref(), Some("zcode/GLM-5.3 (scaffold)"));
        assert_eq!(rs[1].task_id.as_deref(), Some("M-1-WP07-T01"));
        assert_eq!(rs[1].spec, None);
    }

    #[test]
    fn milestone_prefix_scoping() {
        let rs = parse_commits(SAMPLE);
        let n = rs
            .iter()
            .filter(|r| r.task_id.as_deref().is_some_and(|t| t.starts_with("M-1-")))
            .count();
        assert_eq!(n, 2);
    }

    #[test]
    fn wp_extraction() {
        assert_eq!(wp_of("M2-WP03-T07"), Some("M2-WP03"));
        assert_eq!(wp_of("M-1-WP07-T01"), Some("M-1-WP07"));
        assert_eq!(wp_of("bogus"), None);
    }

    #[test]
    fn trailer_takes_last_occurrence() {
        assert_eq!(trailer("x\nK: 1\ny\nK: 2", "K").as_deref(), Some("2"));
        assert_eq!(trailer("no trailers here", "K"), None);
    }
}
