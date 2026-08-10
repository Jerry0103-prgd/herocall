import { useCallback, useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

import { loadHoldingNewsArticles, type NewsArticle } from "../services/news";

function sourceTypeLabel(sourceType: NewsArticle["sourceType"]) {
  if (sourceType === "COMMUNITY") return "社区观点 · COMMUNITY";
  if (sourceType === "OFFICIAL") return "官方信息 · OFFICIAL";
  return "媒体报道 · MEDIA";
}

function formatBeijingTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  const parts = new Intl.DateTimeFormat("en-CA", {
    timeZone: "Asia/Shanghai", year: "numeric", month: "2-digit", day: "2-digit",
    hour: "2-digit", minute: "2-digit", hourCycle: "h23",
  }).formatToParts(date);
  const part = (type: Intl.DateTimeFormatPartTypes) => parts.find((item) => item.type === type)?.value ?? "";
  return `${part("year")}-${part("month")}-${part("day")} ${part("hour")}:${part("minute")}`;
}

export function NewsPage() {
  const [articles, setArticles] = useState<NewsArticle[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [message, setMessage] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setIsLoading(true);
    try {
      const result = await loadHoldingNewsArticles();
      setArticles(result.articles);
      setMessage(result.noDataReason);
    } catch {
      setMessage("本地资讯服务暂不可用");
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  async function openOriginalArticle(url: string) {
    try {
      await openUrl(url);
    } catch {
      setMessage("无法打开原文链接，请稍后重试。");
    }
  }

  return (
    <section className="page news-page" aria-labelledby="news-title">
      <header className="page-header">
        <div>
          <p className="eyebrow">Watchlist news</p>
          <h1 id="news-title">个股资讯</h1>
          <p>仅展示已保存且关联当前关注标的的可追溯资讯；社区内容不作为客观事实。</p>
        </div>
      </header>

      {message ? <p className="notice" role="status">{message}</p> : null}
      {isLoading ? <p className="table-state news-state">正在读取关注标的关联资讯…</p> : null}
      {!isLoading && articles.length === 0 ? <p className="table-state news-state">暂无与关注标的关联的资讯</p> : null}
      {!isLoading && articles.length > 0 ? <div className="news-list">{articles.map((article) => (
        <article className="news-card" key={article.id}>
          <div className="news-card-heading"><div><span className={`news-source-type news-source-type--${article.sourceType.toLowerCase()}`}>{sourceTypeLabel(article.sourceType)}</span><h2>{article.title}</h2></div></div>
          <div className="news-context-grid">
            <div><span>证券</span><strong>{article.relatedSecurity ?? "未关联证券"}</strong></div>
            <div><span>来源</span><strong>{article.source}</strong></div>
            <div><span>发布时间</span><time>{formatBeijingTime(article.publishedAt)}</time></div>
            <button className="news-link-button" onClick={() => void openOriginalArticle(article.url)} type="button">查看原文 <span aria-hidden="true">›</span></button>
          </div>
          <p className="news-summary">{article.summary}</p>
          <footer className="news-meta"><span>抓取时间：{formatBeijingTime(article.fetchTime)}</span></footer>
        </article>
      ))}</div> : null}
    </section>
  );
}
