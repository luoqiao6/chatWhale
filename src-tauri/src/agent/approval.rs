use crate::agent::types::{
    AgentSettings, ApprovalPolicy, BrowserContentPolicy, WhitelistEntry,
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tauri::AppHandle;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use super::emit_agent_event;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalOutcome {
    Granted,
    Rejected(String),
    Timeout,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalResult {
    pub outcome: ApprovalOutcome,
    pub level: Option<String>,
}

#[derive(Clone)]
struct ApprovalReply {
    approved: bool,
    level: Option<String>,
}

#[derive(Serialize, Clone)]
struct ApprovalChoice {
    level: String,
    label: String,
}

#[derive(Serialize, Clone)]
struct ApprovalPayload<'a> {
    id: &'a str,
    tool_name: &'a str,
    command: &'a str,
    policy: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    choices: Option<Vec<ApprovalChoice>>,
}

#[derive(Default)]
pub struct ApprovalManager {
    pending: Mutex<HashMap<String, oneshot::Sender<ApprovalReply>>>,
}

impl ApprovalManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// 发起审批：emit agent-approval-request 后等待用户回执，三路 select（回执/超时/取消）。
    pub async fn request(
        &self,
        app: &AppHandle,
        window_label: Option<&str>,
        tool_name: &str,
        command: &str,
        policy: &str,
        timeout: std::time::Duration,
        cancellation: CancellationToken,
    ) -> ApprovalOutcome {
        self.request_with_choices(app, window_label, tool_name, command, policy, timeout, cancellation, &[])
            .await
            .outcome
    }

    pub async fn request_with_choices(
        &self,
        app: &AppHandle,
        window_label: Option<&str>,
        tool_name: &str,
        command: &str,
        policy: &str,
        timeout: std::time::Duration,
        cancellation: CancellationToken,
        choices: &[BrowserContentPolicy],
    ) -> ApprovalResult {
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        let choices_payload = if choices.is_empty() {
            None
        } else {
            Some(
                choices
                    .iter()
                    .map(|c| ApprovalChoice {
                        level: c.as_str().to_string(),
                        label: match c {
                            BrowserContentPolicy::Normal => "允许并放宽到 normal".to_string(),
                            BrowserContentPolicy::Trusted => "允许并放宽到 trusted".to_string(),
                            BrowserContentPolicy::Strict => "允许（strict）".to_string(),
                        },
                    })
                    .collect(),
            )
        };
        self.pending
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(id.clone(), tx);
        let _ = emit_agent_event(
            app,
            window_label,
            super::EVENT_APPROVAL_REQUEST,
            ApprovalPayload {
                id: &id,
                tool_name,
                command,
                policy,
                choices: choices_payload,
            },
        );

        tokio::select! {
            _ = cancellation.cancelled() => {
                self.pending.lock().unwrap_or_else(|p| p.into_inner()).remove(&id);
                ApprovalResult { outcome: ApprovalOutcome::Cancelled, level: None }
            }
            _ = tokio::time::sleep(timeout) => {
                self.pending.lock().unwrap_or_else(|p| p.into_inner()).remove(&id);
                ApprovalResult { outcome: ApprovalOutcome::Timeout, level: None }
            }
            v = rx => match v {
                Ok(reply) => ApprovalResult {
                    outcome: if reply.approved {
                        ApprovalOutcome::Granted
                    } else {
                        ApprovalOutcome::Rejected("用户拒绝".into())
                    },
                    level: reply.level,
                },
                Err(_) => ApprovalResult { outcome: ApprovalOutcome::Timeout, level: None },
            }
        }
    }

    pub fn resolve_with_level(&self, id: &str, approved: bool, level: Option<String>) -> bool {
        let sender = self
            .pending
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(id);
        match sender {
            Some(tx) => {
                let _ = tx.send(ApprovalReply { approved, level });
                true
            }
            None => false,
        }
    }

    pub fn resolve(&self, id: &str, approved: bool) -> bool {
        self.resolve_with_level(id, approved, None)
    }
}

/// 全局审批注册表：lib.rs 的 agent_approve command 与运行中的 Agent 解耦。
pub fn global_manager() -> &'static ApprovalManager {
    static GLOBAL: OnceLock<ApprovalManager> = OnceLock::new();
    GLOBAL.get_or_init(ApprovalManager::new)
}

pub fn resolve_global(id: &str, approved: bool) -> bool {
    global_manager().resolve(id, approved)
}

pub fn resolve_global_with_level(id: &str, approved: bool, level: Option<String>) -> bool {
    global_manager().resolve_with_level(id, approved, level)
}

pub fn normalized_command(cmd: &str) -> String {
    cmd.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn entry_matches(entry: &WhitelistEntry, command: &str, cwd: Option<&str>) -> bool {
    let norm = normalized_command(command);
    let prefix = normalized_command(&entry.prefix);
    // 禁止仅按命令名匹配（防止 rm -rf 被 "rm" 前缀放行）：条目必须包含参数/子命令
    if prefix.split_whitespace().count() < 2 {
        return false;
    }
    let prefix_matches = norm == prefix || norm.starts_with(&format!("{prefix} "));
    if !prefix_matches {
        return false;
    }
    match (&entry.cwd, cwd) {
        (None, _) => true,
        (Some(ec), Some(cc)) => {
            let a = std::fs::canonicalize(ec).unwrap_or_else(|_| std::path::PathBuf::from(ec));
            let b = std::fs::canonicalize(cc).unwrap_or_else(|_| std::path::PathBuf::from(cc));
            a == b
        }
        _ => false,
    }
}

pub fn is_whitelisted(settings: &AgentSettings, command: &str, cwd: Option<&str>) -> bool {
    settings
        .command_whitelist
        .iter()
        .any(|e| entry_matches(e, command, cwd))
}

/// 返回命令是否需要审批：None = 直接执行（白名单命中）；Some(false) = 需审批；Some(true) = 禁用。
pub fn policy_allows(settings: &AgentSettings, command: &str, cwd: Option<&str>) -> Option<bool> {
    match settings.command_approval {
        ApprovalPolicy::Always => Some(false),
        ApprovalPolicy::Never => Some(true),
        ApprovalPolicy::Whitelist => {
            if is_whitelisted(settings, command, cwd) {
                None
            } else {
                Some(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::types::ApprovalPolicy;

    #[test]
    fn normalizes_command_string() {
        assert_eq!(normalized_command("  ls   -la   "), "ls -la");
    }

    #[test]
    fn whitelist_matches_normalized_prefix_with_cwd() {
        let settings = AgentSettings {
            command_approval: ApprovalPolicy::Whitelist,
            command_whitelist: vec![WhitelistEntry {
                prefix: "git status".into(),
                cwd: Some("/work".into()),
            }],
            ..Default::default()
        };
        assert!(is_whitelisted(&settings, "  git   status", Some("/work")));
        assert!(!is_whitelisted(&settings, "git status", Some("/other")));
        assert!(!is_whitelisted(&settings, "git push", Some("/work")));
    }

    #[test]
    fn whitelist_never_allows_different_prefix() {
        let settings = AgentSettings {
            command_approval: ApprovalPolicy::Whitelist,
            command_whitelist: vec![WhitelistEntry {
                prefix: "rm".into(),
                cwd: None,
            }],
            ..Default::default()
        };
        // 单 token 前缀（仅命令名）不得放行任何命令
        assert!(!is_whitelisted(&settings, "rm -rf /", None));
        assert!(!is_whitelisted(&settings, "rm file.txt", None));

        // 带参数的多 token 前缀按规范化字符串前缀匹配
        let s2 = AgentSettings {
            command_approval: ApprovalPolicy::Whitelist,
            command_whitelist: vec![WhitelistEntry {
                prefix: "rm -rf /tmp/cleanup".into(),
                cwd: None,
            }],
            ..Default::default()
        };
        assert!(is_whitelisted(&s2, "rm -rf /tmp/cleanup", None));
        assert!(!is_whitelisted(&s2, "rm -rf /tmp/other", None));
    }

    #[tokio::test]
    async fn resolve_with_level_returns_level() {
        let mgr = ApprovalManager::new();
        let (tx, rx) = tokio::sync::oneshot::channel();
        mgr.pending
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert("a1".into(), tx);
        assert!(mgr.resolve_with_level("a1", true, Some("trusted".into())));
        let reply = rx.await.unwrap();
        assert!(reply.approved);
        assert_eq!(reply.level.as_deref(), Some("trusted"));
        assert!(!mgr.resolve_with_level("missing", true, None));
    }
}
