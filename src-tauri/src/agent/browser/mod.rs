pub mod policy;
pub mod locator;
pub mod cdp;
pub mod extract;
pub mod tools;

use crate::agent::browser::cdp::{BrowserProcess, CdpSession};
use crate::agent::browser::extract::{PageSnapshot, SNAPSHOT_JS};
use crate::agent::tools::resolve_workspace_path;
use crate::agent::types::AgentSettings;
use anyhow::{anyhow, Context, Result};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::Mutex;

pub struct BrowserSession {
    process: BrowserProcess,
    cdp: CdpSession,
}

pub struct BrowserManager {
    base_dir: PathBuf,
    sessions: Mutex<HashMap<String, BrowserSession>>,
}

impl BrowserManager {
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    async fn ensure_session(&self, workspace_id: &str, browser_path: Option<&str>) -> Result<()> {
        let mut sessions = self.sessions.lock().await;
        if sessions.contains_key(workspace_id) {
            return Ok(());
        }
        let exe = locator::locate(browser_path)
            .ok_or_else(|| anyhow!("未找到 Chrome/Edge，请安装或配置 agent.browser_path"))?;
        let profile = self.base_dir.join("profiles").join(workspace_id);
        std::fs::create_dir_all(&profile).context("创建浏览器 profile 目录失败")?;
        let (process, cdp) = cdp::launch(&exe, &profile).await?;
        sessions.insert(
            workspace_id.to_string(),
            BrowserSession { process, cdp },
        );
        Ok(())
    }

    async fn snapshot(session: &mut BrowserSession) -> Result<PageSnapshot> {
        let v = session.cdp.evaluate(SNAPSHOT_JS).await?;
        serde_json::from_value(v).context("页面快照解析失败")
    }

    pub async fn open(
        &self,
        workspace_id: &str,
        settings: &AgentSettings,
        url: &str,
    ) -> Result<PageSnapshot> {
        self.ensure_session(workspace_id, settings.browser_path.as_deref()).await?;
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(workspace_id)
            .ok_or_else(|| anyhow!("浏览器会话不存在"))?;
        session.cdp.navigate(url).await?;
        Self::snapshot(session).await
    }

    pub async fn read(
        &self,
        workspace_id: &str,
        settings: &AgentSettings,
        selector: Option<&str>,
        timeout_ms: Option<u64>,
    ) -> Result<PageSnapshot> {
        self.ensure_session(workspace_id, settings.browser_path.as_deref()).await?;
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(workspace_id)
            .ok_or_else(|| anyhow!("浏览器会话不存在"))?;
        if let Some(sel) = selector {
            let timeout = timeout_ms.unwrap_or(3000).min(30_000);
            let js = format!(
                r#"new Promise((resolve) => {{
  const sel = {};
  const t0 = Date.now();
  const iv = setInterval(() => {{
    const el = document.querySelector(sel);
    if (el || Date.now() - t0 > {}) {{
      clearInterval(iv);
      const found = document.querySelector(sel);
      resolve({{ ok: !!found, text: found ? found.innerText || "" : "", title: document.title || "", url: location.href }});
    }}
  }}, 200);
}})"#,
                serde_json::json!(sel),
                timeout
            );
            let v = session.cdp.evaluate(&js).await?;
            if v["ok"].as_bool() != Some(true) {
                return Err(anyhow!("选择器 {sel} 在 {timeout}ms 内未出现"));
            }
            return serde_json::from_value(json!({
                "title": v["title"],
                "url": v["url"],
                "articleText": v["text"],
                "bodyText": v["text"],
                "links": [],
                "alts": [],
                "formValues": [],
                "dataAttrs": [],
                "hiddenText": "",
                "scriptText": "",
            }))
            .context("选择器快照解析失败");
        }
        Self::snapshot(session).await
    }

    pub async fn click(
        &self,
        workspace_id: &str,
        settings: &AgentSettings,
        selector: Option<&str>,
        text: Option<&str>,
    ) -> Result<PageSnapshot> {
        self.ensure_session(workspace_id, settings.browser_path.as_deref()).await?;
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(workspace_id)
            .ok_or_else(|| anyhow!("浏览器会话不存在"))?;
        let js = format!(
            r#"(() => {{
  const SEL = {};
  const TXT = {};
  const target = SEL
    ? document.querySelector(SEL)
    : [...document.querySelectorAll("a,button,input[type=submit],input[type=button],[role=button]")]
        .find((el) => (el.innerText || "").trim().includes(TXT)) || null;
  if (!target) return {{ ok: false, reason: "not found" }};
  target.scrollIntoView({{ block: "center" }});
  target.click();
  return {{ ok: true }};
}})()"#,
            serde_json::json!(selector),
            serde_json::json!(text.unwrap_or(""))
        );
        let v = session.cdp.evaluate(&js).await?;
        if v["ok"].as_bool() != Some(true) {
            return Err(anyhow!("未找到可点击元素（selector={selector:?}, text={text:?}）"));
        }
        session.cdp.wait_ready(Duration::from_secs(15)).await?;
        Self::snapshot(session).await
    }

    pub async fn fill(
        &self,
        workspace_id: &str,
        settings: &AgentSettings,
        selector: &str,
        value: &str,
    ) -> Result<String> {
        self.ensure_session(workspace_id, settings.browser_path.as_deref()).await?;
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(workspace_id)
            .ok_or_else(|| anyhow!("浏览器会话不存在"))?;
        let js = format!(
            r#"(() => {{
  const el = document.querySelector({});
  if (!el) return {{ ok: false, reason: "not found" }};
  const type = (el.getAttribute("type") || "text").toLowerCase();
  if (type === "password") return {{ ok: false, reason: "password field forbidden" }};
  el.focus();
  el.value = {};
  el.dispatchEvent(new Event("input", {{ bubbles: true }}));
  el.dispatchEvent(new Event("change", {{ bubbles: true }}));
  return {{ ok: true }};
}})()"#,
            serde_json::json!(selector),
            serde_json::json!(value)
        );
        let v = session.cdp.evaluate(&js).await?;
        if v["ok"].as_bool() != Some(true) {
            let reason = v["reason"].as_str().unwrap_or("unknown");
            return Err(anyhow!("填表失败: {reason}"));
        }
        Ok(format!("已填写 {selector}"))
    }

    pub async fn scroll(
        &self,
        workspace_id: &str,
        settings: &AgentSettings,
        direction: &str,
        amount: Option<i64>,
    ) -> Result<String> {
        self.ensure_session(workspace_id, settings.browser_path.as_deref()).await?;
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(workspace_id)
            .ok_or_else(|| anyhow!("浏览器会话不存在"))?;
        let amt = amount.unwrap_or(600);
        let js = format!(
            r#"(() => {{
  const DIR = {};
  const AMT = {};
  if (DIR === "top") window.scrollTo(0, 0);
  else if (DIR === "bottom") window.scrollTo(0, document.body.scrollHeight);
  else if (DIR === "down") window.scrollBy(0, AMT);
  else if (DIR === "up") window.scrollBy(0, -AMT);
  return {{ x: window.scrollX, y: window.scrollY }};
}})()"#,
            serde_json::json!(direction),
            amt
        );
        let v = session.cdp.evaluate(&js).await?;
        Ok(format!("已滚动到 ({}, {})", v["x"], v["y"]))
    }

    pub async fn screenshot(
        &self,
        workspace_id: &str,
        settings: &AgentSettings,
        path: Option<&str>,
    ) -> Result<PathBuf> {
        self.ensure_session(workspace_id, settings.browser_path.as_deref()).await?;
        let bytes = {
            let mut sessions = self.sessions.lock().await;
            let session = sessions
                .get_mut(workspace_id)
                .ok_or_else(|| anyhow!("浏览器会话不存在"))?;
            session.cdp.screenshot().await?
        };
        let dir = self.base_dir.join("screenshots").join(workspace_id);
        std::fs::create_dir_all(&dir).context("创建截图目录失败")?;
        let name = format!("{}.png", uuid::Uuid::new_v4());
        let app_path = dir.join(&name);
        std::fs::write(&app_path, &bytes).context("写入截图失败")?;
        if let Some(p) = path {
            let resolved = resolve_workspace_path(settings, p)?;
            if let Some(parent) = resolved.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::copy(&app_path, &resolved).context("复制截图到 workspace 失败")?;
            Ok(resolved)
        } else {
            Ok(app_path)
        }
    }

    pub async fn close(&self, workspace_id: &str) -> Result<()> {
        let mut sessions = self.sessions.lock().await;
        if let Some(s) = sessions.remove(workspace_id) {
            s.process.stop().await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn close_missing_session_is_ok() {
        let dir = std::env::temp_dir().join(format!("cw-bm-{}", uuid::Uuid::new_v4()));
        let bm = BrowserManager::new(dir.clone());
        assert!(bm.close("nope").await.is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
