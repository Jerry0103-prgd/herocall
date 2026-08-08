import { useCallback, useEffect, useState } from "react";

import { loadHoldingNewsArticles, type NewsArticle } from "../services/news";

function sourceTypeLabel(sourceType: NewsArticle["sourceType"]) {
  if (sourceType === "COMMUNITY") return "社区观点 · COMMUNITY";
  if (sourceType === "OFFICIAL") return "官方信息 · OFFICIAL";
  return "媒体报道 · MEDIA";
}

export function NewsPage() {
  const [articles, setArticles] = useState<NewsArticle[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [message, setMessage] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setIsLoading(true);
    try {
      setArticles(await loadHoldingNewsArticles());
      setMessage(null);
    } catch {
      setMessage("本地资讯服务暂不可用");
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  return (
    <section className="page news-page" aria-labelledby="news-title">
      <header className="page-header">
        <div>
          <p className="eyebrow">Holding news</p>
          <h1 id="news-title">财经资讯</h1>
          <p>仅展示已保存且关联当前持仓的可追溯资讯；社区内容不作为客观事实。</p>
        </div>
      </header>

      {message ? <p className="notice" role="status">{message}</p> : null}
      {isLoading ? <p className="table-state news-state">正在读取持仓关联资讯…</p> : null}
      {!isLoading && articles.length === 0 ? <p className="table-state news-state">暂无与持仓关联的资讯</p> : null}
      {!isLoading && articles.length > 0 ? <div className="news-list">{articles.map((article) => (
        <article className="news-card" key={article.id}>
          <div className="news-card-heading"><div><span className={`news-source-type news-source-type--${article.sourceType.toLowerCase()}`}>{sourceTypeLabel(article.sourceType)}</span><h2>{article.title}</h2></div><span className="news-related-security">{article.relatedSecurity ?? "未关联证券"}</span></div>
          <p className="news-summary">{article.summary}</p>
          <footer className="news-meta"><span>来源：{article.source}</span><span>发布时间：{article.publishedAt}</span><a href={article.url} rel="noreferrer" target="_blank">查看原文</a></footer>
        </article>
      ))}</div> : null}
    </section>
  );
}
