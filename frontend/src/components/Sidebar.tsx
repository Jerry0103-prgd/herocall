export type PageId =
  | "dashboard"
  | "news"
  | "review"
  | "holdings"
  | "calendar"
  | "settings";

type NavigationItem = {
  id: PageId;
  label: string;
  symbol: string;
};

const navigationItems: NavigationItem[] = [
  { id: "dashboard", label: "今日总览", symbol: "◈" },
  { id: "news", label: "财经资讯", symbol: "◌" },
  { id: "review", label: "仓位复盘", symbol: "◒" },
  { id: "holdings", label: "我的持仓", symbol: "◫" },
  { id: "calendar", label: "事件日历", symbol: "◷" },
  { id: "settings", label: "设置", symbol: "◉" },
];

type SidebarProps = {
  activePage: PageId;
  onNavigate: (page: PageId) => void;
};

export function Sidebar({ activePage, onNavigate }: SidebarProps) {
  return (
    <aside className="sidebar" aria-label="主导航">
      <div className="brand">
        <span className="brand-mark" aria-hidden="true">A</span>
        <div>
          <strong>AStock</strong>
          <span>AI Workbench</span>
        </div>
      </div>

      <nav className="navigation">
        {navigationItems.map((item) => (
          <button
            className={`navigation-item ${activePage === item.id ? "is-active" : ""}`}
            key={item.id}
            onClick={() => onNavigate(item.id)}
            type="button"
          >
            <span aria-hidden="true" className="navigation-symbol">{item.symbol}</span>
            {item.label}
          </button>
        ))}
      </nav>

      <p className="sidebar-note">本地只读 · 不提供交易能力</p>
    </aside>
  );
}
