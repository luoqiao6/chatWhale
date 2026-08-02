use chatwhale_lib::agent::mcp::transport::McpTransport;
use chatwhale_lib::agent::mcp::types::{McpServerConfig, TransportKind};
use std::collections::HashMap;

#[tokio::test]
async fn fake_server_list_and_call_tools() {
    let fixture = std::env::current_dir()
        .unwrap()
        .join("tests/fixtures/fake_mcp_server.sh");
    let cfg = McpServerConfig {
        id: "fake".into(),
        workspace_id: "default".into(),
        name: "fake".into(),
        command: "bash".into(),
        args: vec![fixture.display().to_string()],
        env: HashMap::new(),
        cwd: None,
        timeout: 5,
        transport: TransportKind::Stdio,
        enabled: true,
    };
    let mut t = McpTransport::spawn(&cfg).await.unwrap();
    let info = t.initialize().await.unwrap();
    assert!(info.to_string().contains("2025-03-26"));
    t.notify_initialized().await.unwrap();
    let tools = t.list_tools().await.unwrap();
    assert!(tools.get("tools").and_then(|v| v.as_array()).is_some());
    let result = t
        .call_tool("echo", serde_json::json!({"text": "hi"}), std::time::Duration::from_secs(5))
        .await
        .unwrap();
    assert!(result.to_string().contains("hi"));
    t.shutdown().await;
}
