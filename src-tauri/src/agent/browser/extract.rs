use crate::agent::types::BrowserContentPolicy;
use serde::Deserialize;

/// 页面内采集结构化快照；字段名与 PageSnapshot 的 camelCase 对应。
pub const SNAPSHOT_JS: &str = r#"
(() => {
  const collectVisible = (root) => (root && root.innerText) ? root.innerText : "";
  const main = document.querySelector("article") || document.querySelector("main");
  const links = [];
  document.querySelectorAll("a[href]").forEach((a) => {
    const text = (a.innerText || "").trim().slice(0, 200);
    const href = (a.getAttribute("href") || "").slice(0, 1000);
    if (text || href) links.push({ text, href });
  });
  const alts = [];
  document.querySelectorAll("img[alt]").forEach((img) => {
    const alt = (img.getAttribute("alt") || "").trim().slice(0, 500);
    if (alt) alts.push(alt);
  });
  const formValues = [];
  document.querySelectorAll("input, textarea, select").forEach((el) => {
    const type = (el.getAttribute("type") || "text").toLowerCase();
    if (type === "password" || type === "hidden") return;
    const name = el.getAttribute("name") || el.getAttribute("id") || "";
    const value = el.value;
    if (name && value) formValues.push({ name: name.slice(0, 100), value: String(value).slice(0, 500) });
  });
  const dataAttrs = [];
  document.querySelectorAll("[data-]").forEach((el) => {
    if (dataAttrs.length >= 200) return;
    for (const attr of el.attributes) {
      if (attr.name.startsWith("data-") && attr.value) {
        dataAttrs.push({ name: attr.name.slice(0, 100), value: attr.value.slice(0, 500) });
        if (dataAttrs.length >= 200) break;
      }
    }
  });
  let hiddenText = "";
  document.querySelectorAll("body *").forEach((el) => {
    if (hiddenText.length >= 20000) return;
    const st = getComputedStyle(el);
    if (st.display === "none" || st.visibility === "hidden") {
      const t = (el.textContent || "").trim();
      if (t) hiddenText += t + "\n";
    }
  });
  let scriptText = "";
  document.querySelectorAll("script, style").forEach((el) => {
    if (scriptText.length >= 20000) return;
    const t = (el.textContent || "").trim();
    if (t) scriptText += t + "\n";
  });
  return {
    title: document.title || "",
    url: location.href,
    articleText: collectVisible(main) || collectVisible(document.body),
    bodyText: collectVisible(document.body),
    links: links.slice(0, 200),
    alts: alts.slice(0, 200),
    formValues: formValues.slice(0, 200),
    dataAttrs,
    hiddenText,
    scriptText,
  };
})()
"#;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageSnapshot {
    pub title: String,
    pub url: String,
    pub article_text: String,
    pub body_text: String,
    pub links: Vec<LinkItem>,
    pub alts: Vec<String>,
    pub form_values: Vec<FormValue>,
    pub data_attrs: Vec<AttrItem>,
    pub hidden_text: String,
    pub script_text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LinkItem {
    pub text: String,
    pub href: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FormValue {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AttrItem {
    pub name: String,
    pub value: String,
}

pub fn render_snapshot(snap: &PageSnapshot, policy: BrowserContentPolicy, mode: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", snap.title));
    out.push_str(&format!("URL: {}\n\n", snap.url));
    match mode {
        "markdown" => render_markdown(snap, policy, &mut out),
        "links" => render_links(snap, policy, &mut out),
        _ => render_text(snap, policy, &mut out),
    }
    out
}

fn render_text(snap: &PageSnapshot, policy: BrowserContentPolicy, out: &mut String) {
    match policy {
        BrowserContentPolicy::Strict => out.push_str(&snap.article_text),
        _ => {
            out.push_str(&snap.body_text);
            if !snap.links.is_empty() {
                out.push_str("\n\n## 链接\n");
                for l in &snap.links {
                    out.push_str(&format!("- {}: {}\n", l.text, l.href));
                }
            }
            if !snap.alts.is_empty() {
                out.push_str("\n## 图片说明\n");
                for a in &snap.alts {
                    out.push_str(&format!("- {a}\n"));
                }
            }
            if policy == BrowserContentPolicy::Trusted {
                if !snap.form_values.is_empty() {
                    out.push_str("\n## 表单值\n");
                    for f in &snap.form_values {
                        out.push_str(&format!("- {} = {}\n", f.name, f.value));
                    }
                }
                if !snap.data_attrs.is_empty() {
                    out.push_str("\n## data 属性\n");
                    for d in &snap.data_attrs {
                        out.push_str(&format!("- {} = {}\n", d.name, d.value));
                    }
                }
                if !snap.hidden_text.is_empty() {
                    out.push_str("\n## 隐藏文本\n");
                    out.push_str(&snap.hidden_text);
                    out.push('\n');
                }
                if !snap.script_text.is_empty() {
                    out.push_str("\n## 脚本/样式文本\n");
                    out.push_str(&snap.script_text);
                }
            }
        }
    }
}

fn render_links(snap: &PageSnapshot, policy: BrowserContentPolicy, out: &mut String) {
    for l in &snap.links {
        if policy >= BrowserContentPolicy::Normal {
            out.push_str(&format!("- {}: {}\n", l.text, l.href));
        } else {
            out.push_str(&format!("- {}\n", l.text));
        }
    }
}

fn render_markdown(snap: &PageSnapshot, policy: BrowserContentPolicy, out: &mut String) {
    let body = match policy {
        BrowserContentPolicy::Strict => &snap.article_text,
        _ => &snap.body_text,
    };
    for para in body.split("\n\n") {
        let p = para.trim();
        if !p.is_empty() {
            out.push_str(p);
            out.push_str("\n\n");
        }
    }
    if !snap.links.is_empty() {
        out.push_str("## 链接\n");
        for l in &snap.links {
            if policy >= BrowserContentPolicy::Normal {
                out.push_str(&format!("- [{}]({})\n", l.text, l.href));
            } else {
                out.push_str(&format!("- {}\n", l.text));
            }
        }
    }
    if policy == BrowserContentPolicy::Trusted && !snap.form_values.is_empty() {
        out.push_str("\n## 表单值\n");
        for f in &snap.form_values {
            out.push_str(&format!("- {} = {}\n", f.name, f.value));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PageSnapshot {
        PageSnapshot {
            title: "Test".into(),
            url: "https://example.com/".into(),
            article_text: "正文".into(),
            body_text: "正文\n导航".into(),
            links: vec![LinkItem { text: "官网".into(), href: "https://example.com/".into() }],
            alts: vec!["示意图".into()],
            form_values: vec![FormValue { name: "token".into(), value: "abc123".into() }],
            data_attrs: vec![AttrItem { name: "data-id".into(), value: "42".into() }],
            hidden_text: "隐藏段".into(),
            script_text: "var x=1;".into(),
        }
    }

    #[test]
    fn strict_renders_article_text_without_sensitive_classes() {
        let out = render_snapshot(&fixture(), BrowserContentPolicy::Strict, "text");
        assert!(out.contains("正文"));
        assert!(!out.contains("abc123"));
        assert!(!out.contains("var x=1;"));
    }

    #[test]
    fn normal_adds_links_and_alts_but_not_form_values() {
        let out = render_snapshot(&fixture(), BrowserContentPolicy::Normal, "text");
        assert!(out.contains("官网"));
        assert!(out.contains("示意图"));
        assert!(!out.contains("abc123"));
    }

    #[test]
    fn trusted_adds_form_values_attributes_hidden_and_scripts() {
        let out = render_snapshot(&fixture(), BrowserContentPolicy::Trusted, "text");
        assert!(out.contains("abc123"));
        assert!(out.contains("data-id = 42"));
        assert!(out.contains("隐藏段"));
        assert!(out.contains("var x=1;"));
    }

    #[test]
    fn strict_links_mode_omits_urls() {
        let out = render_snapshot(&fixture(), BrowserContentPolicy::Strict, "links");
        assert!(out.contains("官网"));
        // 链接行不得带 URL（头部 URL 元信息允许保留）
        assert!(!out.contains("- 官网: https://"));
        assert!(!out.contains("[官网](https://"));
    }

    #[test]
    fn markdown_mode_formats_links_with_url_when_allowed() {
        let out = render_snapshot(&fixture(), BrowserContentPolicy::Normal, "markdown");
        assert!(out.contains("[官网](https://example.com/)"));
    }
}
