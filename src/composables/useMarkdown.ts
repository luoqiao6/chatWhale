import { marked, type Renderer } from "marked";
import hljs from "highlight.js";

// Create a custom renderer for chat-optimized markdown
function createRenderer(): Renderer {
  const renderer = new marked.Renderer();

  renderer.code = function({ text, lang }: { text: string; lang?: string }): string {
    let highlighted: string;
    if (lang && hljs.getLanguage(lang)) {
      try {
        highlighted = hljs.highlight(text, { language: lang }).value;
      } catch {
        highlighted = hljs.highlightAuto(text).value;
      }
    } else {
      highlighted = hljs.highlightAuto(text).value;
    }
    const langTag = lang ? `<span class="code-lang-tag">${lang}</span>` : "";
    const copyBtn = `<button class="code-copy-btn" title="复制代码" onclick="navigator.clipboard.writeText(this.dataset.code);this.textContent='✅';setTimeout(()=>this.textContent='📋',1500)" data-code="${escapeHtml(text)}">📋</button>`;
    return `<pre>${langTag}${copyBtn}<code>${highlighted}</code></pre>`;
  };

  return renderer;
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

marked.setOptions({
  renderer: createRenderer(),
  breaks: true,
  gfm: true,
});

export function renderMarkdown(text: string): string {
  return marked.parse(text) as string;
}
