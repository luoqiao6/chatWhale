use chatwhale_lib::agent::browser::locator;
use chatwhale_lib::agent::browser::BrowserManager;
use chatwhale_lib::agent::types::AgentSettings;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn spawn_fixture_server() -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { continue };
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let _ = sock.read(&mut buf).await;
                let body = r#"<!doctype html><html><body>
                  <article><h1>Hello Browser</h1><p id="js">loading</p></article>
                  <a id="go" href="/clicked">Go</a>
                  <input id="name" name="name" value="preset">
                  <script>document.getElementById("js").textContent = "rendered by js";</script>
                </body></html>"#;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = sock.write_all(resp.as_bytes()).await;
            });
        }
    });
    (format!("http://{addr}"), handle)
}

#[tokio::test]
async fn browser_open_read_click_fill_scroll_screenshot_close() {
    let Some(exe) = locator::locate(None) else {
        eprintln!("未检测到 Chrome/Edge，跳过集成测试");
        return;
    };
    let (base, _server) = spawn_fixture_server().await;
    let tmp = std::env::temp_dir().join(format!("cw-browser-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp).unwrap();
    let bm = BrowserManager::new(tmp.clone());
    let settings = AgentSettings {
        browser_path: Some(exe.display().to_string()),
        ..Default::default()
    };

    let snap = bm.open("test", &settings, &format!("{base}/")).await.unwrap();
    assert!(snap.body_text.contains("rendered by js"), "JS 渲染内容未出现");
    assert!(snap.article_text.contains("Hello Browser"));

    let snap2 = bm.click("test", &settings, None, Some("Go")).await.unwrap();
    assert!(snap2.url.ends_with("/clicked"), "点击后 URL 未跳转: {}", snap2.url);

    let msg = bm.fill("test", &settings, "#name", "hello").await.unwrap();
    assert!(msg.contains("#name"));
    let snap3 = bm.read("test", &settings, None, None).await.unwrap();
    assert!(snap3.form_values.iter().any(|f| f.name == "name" && f.value == "hello"));

    let scroll_msg = bm.scroll("test", &settings, "bottom", None).await.unwrap();
    assert!(scroll_msg.contains("已滚动"));

    let shot = bm.screenshot("test", &settings, None).await.unwrap();
    assert!(std::fs::metadata(&shot).unwrap().len() > 100, "截图文件过小");

    bm.close("test").await.unwrap();
    bm.close("test").await.unwrap(); // 幂等
    let _ = std::fs::remove_dir_all(&tmp);
}
