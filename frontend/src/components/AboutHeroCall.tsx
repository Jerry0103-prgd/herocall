type ReleaseNote = {
  version: string;
  date: string;
  summary?: string;
  groups: Array<{ title: string; items: string[] }>;
};

const releaseNotes: ReleaseNote[] = [
  {
    version: "V1.0.8",
    date: "2026-08-10",
    summary: "市场快照与投研信息密度优化",
    groups: [
      { title: "更新", items: [
        "明确 Market Data Provider 行情服务边界",
        "总览指数卡展示真实当日点位、涨跌幅、今开、高低点和成交额",
        "我的关注展示已保存行情快照的当前价格与涨跌幅",
        "AI复盘强化行情、资讯、事件和市场环境的证据要求",
        "行情刷新失败时展示安全的具体原因",
      ] },
    ],
  },
  {
    version: "V1.0.7",
    date: "2026-08-10",
    summary: "关注标的自主录入与彻底删除",
    groups: [
      { title: "更新", items: [
        "新增关注不再依赖本地证券基础信息校验",
        "取消关注新增不可恢复的二次确认",
        "确认删除后清理该标的本地行情、资讯、事件与 AI 分析",
        "共享资讯与事件按关联关系保护其他关注标的",
        "腾讯混元切换为 TokenHub OpenAI 兼容服务并支持连接测试",
      ] },
    ],
  },
  {
    version: "V1.0.6",
    date: "2026-08-10",
    summary: "关注管理与 AI Provider 状态优化",
    groups: [
      { title: "更新", items: [
        "修复取消关注逻辑，保留全部历史研究与交易数据",
        "支持证券代码或名称匹配本地已验证基础信息",
        "今日总览显示实际会调用的 AI Provider",
        "统一个股资讯页面定位与文案",
        "明确展示 AI Provider 的配置、启用与当前使用状态",
      ] },
    ],
  },
  {
    version: "V1.0.5",
    date: "2026-08-09",
    summary: "指数收盘观察与 AI复盘信息密度优化",
    groups: [
      { title: "更新", items: [
        "指数卡展示最近交易日、近5日与近10日真实收盘观察",
        "AI复盘显示当前实际启用的 Provider",
        "隐藏历史综合复盘入口，聚焦关注标的分析",
      ] },
    ],
  },
  {
    version: "V1.0.4",
    date: "2026-08-09",
    summary: "AI复盘中心与多 Provider 支持",
    groups: [
      { title: "更新", items: [
        "仓位复盘升级为 AI复盘中心",
        "按关注标的生成独立 AI 投研报告",
        "新增腾讯混元、豆包 Provider 配置",
        "优化系统同步时间的北京时间展示",
      ] },
    ],
  },
  {
    version: "V1.0.3",
    date: "2026-08-09",
    summary: "关注管理与 AI 投研复盘升级",
    groups: [
      { title: "更新", items: [
        "优化关注标的管理",
        "支持完整取消关注",
        "升级AI复盘报告结构",
        "优化投研分析展示体验",
      ] },
    ],
  },
  {
    version: "V1.0.2",
    date: "2026-08-09",
    summary: "体验优化版本",
    groups: [
      { title: "优化", items: [
        "优化关注标的管理体验",
        "优化资讯展示",
        "优化AI复盘阅读体验",
        "优化首页操作入口",
      ] },
    ],
  },
  {
    version: "V1.0.1",
    date: "2026-08-09",
    summary: "体验优化与问题修复",
    groups: [
      { title: "更新", items: [
        "UI视觉优化",
        "关注标的定位调整",
        "导航优化",
        "数据展示优化",
        "问题修复",
      ] },
    ],
  },
  {
    version: "V1.0.0",
    date: "2026-08-09",
    summary: "正式版本发布",
    groups: [
      { title: "新增", items: [
        "手动市场快照",
        "东方财富公告资讯",
        "AI复盘真实链路",
        "DeepSeek AI分析",
        "数据状态监控",
      ] },
      { title: "优化", items: [
        "Hero Call品牌升级",
        "UI统一优化",
        "本地数据安全存储",
      ] },
    ],
  },
  {
    version: "V0.9.0",
    date: "2026-08",
    groups: [
      { title: "主要更新", items: [
        "完成 Release 构建",
        "完成 Keychain 配置",
        "完成 AI复盘基础能力",
      ] },
    ],
  },
];

type AboutHeroCallProps = {
  version: string | null;
};

export function AboutHeroCall({ version }: AboutHeroCallProps) {
  return (
    <section className="settings-section about-section" aria-labelledby="about-title">
      <div className="section-heading">
        <div><p className="section-kicker">About</p><h2 id="about-title">关于 Hero Call</h2></div>
      </div>
      <div className="settings-card about-card">
        <div className="about-product">
          <strong>Hero Call</strong>
          <p>AI Portfolio Assistant</p>
          <span>当前版本：{version ? `v${version}` : "暂无数据"}</span>
        </div>
        <div className="release-notes" aria-label="迭代记录">
          <h3>迭代记录</h3>
          {releaseNotes.map((release) => (
            <article className="release-note" key={release.version}>
              <header><strong>{release.version}</strong><time>{release.date}</time></header>
              {release.summary ? <p>{release.summary}</p> : null}
              {release.groups.map((group) => <div className="release-note-group" key={group.title}>
                <h4>{group.title}</h4>
                <ul>{group.items.map((item) => <li key={item}>{item}</li>)}</ul>
              </div>)}
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}
