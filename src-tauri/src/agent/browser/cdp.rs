use anyhow::{anyhow, Context, Result};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::page::Page;
use futures_util::StreamExt;
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

pub struct BrowserProcess {
    browser: Browser,
    _handler: tokio::task::JoinHandle<()>,
}

impl BrowserProcess {
    pub async fn stop(mut self) {
        // 关闭浏览器连接并回收事件排空任务；浏览器进程由 chromiumoxide 生命周期管理
        let _ = self.browser.close().await;
        self._handler.abort();
    }
}

pub async fn launch(executable: &Path, user_data_dir: &Path) -> Result<(BrowserProcess, CdpSession)> {
    let config = BrowserConfig::builder()
        .user_data_dir(user_data_dir)
        .chrome_executable(executable)
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--remote-allow-origins=*")
        .with_head()
        .build()
        .map_err(|e| anyhow!("构建浏览器配置失败: {e}"))?;
    let (browser, mut handler) = Browser::launch(config).await.context("启动浏览器失败")?;
    let handler_task = tokio::spawn(async move {
        while handler.next().await.is_some() {}
    });
    let page = browser
        .new_page("about:blank")
        .await
        .context("创建页面失败")?;
    Ok((
        BrowserProcess { browser, _handler: handler_task },
        CdpSession { page },
    ))
}

pub struct CdpSession {
    page: Page,
}

impl CdpSession {
    pub async fn evaluate(&mut self, expression: &str) -> Result<Value> {
        let js = self
            .page
            .evaluate_expression(expression)
            .await
            .context("页面 JS 执行失败")?;
        let v = js
            .into_value::<Value>()
            .map_err(|_| anyhow!("页面 JS 返回值解析失败"))?;
        Ok(v)
    }

    pub async fn navigate(&mut self, url: &str) -> Result<()> {
        // goto 在目标 URL 完全加载后返回；随后等一个宏任务窗口再轮询就绪态
        self.page.goto(url).await.context("导航失败")?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        self.wait_ready(Duration::from_secs(30)).await
    }

    pub async fn wait_ready(&mut self, timeout: Duration) -> Result<()> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut last_err: Option<anyhow::Error> = None;
        loop {
            if tokio::time::Instant::now() >= deadline {
                return Err(last_err.unwrap_or_else(|| anyhow!("页面加载超时")));
            }
            match self.evaluate("document.readyState").await {
                Ok(v) if v.as_str() == Some("complete") => {
                    // 给 SPA 首帧渲染留一个宏任务窗口
                    let _ = self.evaluate("new Promise(r => setTimeout(r, 300))").await;
                    return Ok(());
                }
                Ok(_) => {}
                Err(e) => {
                    // 导航切换执行上下文期间评估可能瞬时失败，视为未就绪继续轮询
                    last_err = Some(e);
                }
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    pub async fn screenshot(&mut self) -> Result<Vec<u8>> {
        let bytes = self
            .page
            .screenshot(chromiumoxide::page::ScreenshotParams::default())
            .await
            .context("截图失败")?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn launch_fails_with_missing_executable() {
        let dir = std::env::temp_dir().join(format!("cw-cdp-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = std::env::temp_dir().join("definitely-not-a-browser");
        let result = launch(&exe, &dir).await;
        assert!(result.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
