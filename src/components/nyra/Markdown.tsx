import React from "react";

// Small Markdown renderer for Nyra's answers: headings, paragraphs, lists, tables,
// block quotes, fenced code, inline code, bold, italic and links (shown as text).
// It builds React elements, so model output can never inject HTML.

type Block =
  | { kind: "heading"; level: number; text: string }
  | { kind: "paragraph"; text: string }
  | { kind: "list"; ordered: boolean; items: string[] }
  | { kind: "table"; header: string[]; rows: string[][] }
  | { kind: "quote"; text: string }
  | { kind: "code"; text: string }
  | { kind: "rule" };

const splitRow = (line: string) =>
  line
    .trim()
    .replace(/^\|/, "")
    .replace(/\|$/, "")
    .split("|")
    .map((c) => c.trim());

function parse(source: string): Block[] {
  const lines = source.replace(/\r\n/g, "\n").split("\n");
  const blocks: Block[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    const trimmed = line.trim();
    if (!trimmed) {
      i++;
      continue;
    }
    if (trimmed.startsWith("```")) {
      const body: string[] = [];
      i++;
      while (i < lines.length && !lines[i].trim().startsWith("```")) body.push(lines[i++]);
      i++;
      blocks.push({ kind: "code", text: body.join("\n") });
      continue;
    }
    const heading = /^(#{1,4})\s+(.*)$/.exec(trimmed);
    if (heading) {
      blocks.push({ kind: "heading", level: heading[1].length, text: heading[2] });
      i++;
      continue;
    }
    if (/^(-{3,}|\*{3,}|_{3,})$/.test(trimmed)) {
      blocks.push({ kind: "rule" });
      i++;
      continue;
    }
    if (trimmed.startsWith("|") && i + 1 < lines.length && /^\s*\|?\s*:?-{2,}/.test(lines[i + 1])) {
      const header = splitRow(trimmed);
      const rows: string[][] = [];
      i += 2;
      while (i < lines.length && lines[i].trim().startsWith("|")) rows.push(splitRow(lines[i++]));
      blocks.push({ kind: "table", header, rows });
      continue;
    }
    if (/^[-*+]\s+/.test(trimmed) || /^\d+[.)]\s+/.test(trimmed)) {
      const ordered = /^\d+[.)]\s+/.test(trimmed);
      const items: string[] = [];
      while (i < lines.length) {
        const t = lines[i].trim();
        const m = ordered ? /^\d+[.)]\s+(.*)$/.exec(t) : /^[-*+]\s+(.*)$/.exec(t);
        if (m) {
          items.push(m[1]);
          i++;
        } else if (t && items.length && /^\s{2,}/.test(lines[i])) {
          items[items.length - 1] += ` ${t}`;
          i++;
        } else break;
      }
      blocks.push({ kind: "list", ordered, items });
      continue;
    }
    if (trimmed.startsWith(">")) {
      const body: string[] = [];
      while (i < lines.length && lines[i].trim().startsWith(">")) body.push(lines[i++].trim().replace(/^>\s?/, ""));
      blocks.push({ kind: "quote", text: body.join(" ") });
      continue;
    }
    const para: string[] = [];
    while (
      i < lines.length &&
      lines[i].trim() &&
      !/^(#{1,4}\s|```|[-*+]\s|\d+[.)]\s|>|\|)/.test(lines[i].trim())
    ) {
      para.push(lines[i++].trim());
    }
    if (para.length === 0) {
      para.push(trimmed);
      i++;
    }
    blocks.push({ kind: "paragraph", text: para.join(" ") });
  }
  return blocks;
}

const INLINE = /(`[^`]+`|\*\*[^*]+\*\*|__[^_]+__|\*[^*\s][^*]*\*|\[[^\]]+\]\([^)\s]+\))/g;

function inline(text: string): React.ReactNode[] {
  const out: React.ReactNode[] = [];
  let last = 0;
  let key = 0;
  for (const match of text.matchAll(INLINE)) {
    const token = match[0];
    const start = match.index ?? 0;
    if (start > last) out.push(text.slice(last, start));
    if (token.startsWith("`")) {
      out.push(
        <code key={key++} className="px-1 py-px rounded bg-lunar-surface-2 border border-lunar-border text-[12px] font-mono break-all">
          {token.slice(1, -1)}
        </code>
      );
    } else if (token.startsWith("**") || token.startsWith("__")) {
      out.push(<strong key={key++} className="font-semibold text-lunar-white">{inline(token.slice(2, -2))}</strong>);
    } else if (token.startsWith("*")) {
      out.push(<em key={key++}>{inline(token.slice(1, -1))}</em>);
    } else {
      const m = /^\[([^\]]+)\]\(([^)]+)\)$/.exec(token);
      out.push(
        <span key={key++} className="underline decoration-lunar-muted underline-offset-2" title={m?.[2]}>
          {m?.[1] ?? token}
        </span>
      );
    }
    last = start + token.length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

export const Markdown: React.FC<{ text: string }> = ({ text }) => {
  const blocks = React.useMemo(() => parse(text), [text]);
  return (
    <div className="space-y-3 text-sm leading-6 text-lunar-text select-text [&_*]:select-text">
      {blocks.map((b, i) => {
        switch (b.kind) {
          case "heading":
            return (
              <p key={i} className={`font-semibold text-lunar-white ${b.level <= 2 ? "text-[15px] pt-1" : "text-sm"}`}>
                {inline(b.text)}
              </p>
            );
          case "paragraph":
            return <p key={i}>{inline(b.text)}</p>;
          case "list": {
            const Tag = b.ordered ? "ol" : "ul";
            return (
              <Tag key={i} className={`pl-5 space-y-1 ${b.ordered ? "list-decimal" : "list-disc"} marker:text-lunar-muted`}>
                {b.items.map((item, j) => (
                  <li key={j}>{inline(item)}</li>
                ))}
              </Tag>
            );
          }
          case "table":
            return (
              <div key={i} className="overflow-x-auto rounded-lg border border-lunar-border">
                <table className="w-full text-xs border-collapse">
                  <thead className="bg-lunar-surface-2">
                    <tr>
                      {b.header.map((h, j) => (
                        <th key={j} className="text-left font-medium text-lunar-text-sec px-3 py-2 border-b border-lunar-border">
                          {inline(h)}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {b.rows.map((row, r) => (
                      <tr key={r} className="border-b border-lunar-border/60 last:border-0">
                        {row.map((cell, c) => (
                          <td key={c} className="px-3 py-1.5 align-top break-words">
                            {inline(cell)}
                          </td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            );
          case "quote":
            return (
              <blockquote key={i} className="border-l-2 border-lunar-border pl-3 text-lunar-text-sec">
                {inline(b.text)}
              </blockquote>
            );
          case "code":
            return (
              <pre key={i} className="rounded-lg bg-lunar-surface-2 border border-lunar-border p-3 text-xs overflow-x-auto font-mono">
                {b.text}
              </pre>
            );
          case "rule":
            return <hr key={i} className="border-lunar-border" />;
        }
      })}
    </div>
  );
};
