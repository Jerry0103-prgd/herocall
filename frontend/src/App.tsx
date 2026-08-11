import { useEffect, useState } from "react";

import { FirstRunWizard } from "./components/FirstRunWizard";
import { Sidebar, type PageId } from "./components/Sidebar";
import { ComingSoonPage } from "./pages/ComingSoonPage";
import { DashboardPage } from "./pages/DashboardPage";
import { EventCalendarPage } from "./pages/EventCalendarPage";
import { NewsPage } from "./pages/NewsPage";
import { PortfolioPage } from "./pages/PortfolioPage";
import { ReviewPage } from "./pages/ReviewPage";
import { SettingsPage } from "./pages/SettingsPage";
import { loadInitializationStatus } from "./services/initialization";

const pageTitles: Record<Exclude<PageId, "dashboard">, string> = {
  news: "市场情报",
  review: "AI复盘",
  holdings: "我的关注",
  calendar: "市场雷达",
  settings: "设置",
};

function App() {
  const [activePage, setActivePage] = useState<PageId>("dashboard");
  const [initializationComplete, setInitializationComplete] = useState<boolean | null>(null);

  useEffect(() => {
    void loadInitializationStatus()
      .then((status) => setInitializationComplete(status.completed))
      // A service failure must not create an unfinishable onboarding flow. The existing pages
      // retain their own unavailable-state handling and the next healthy launch checks again.
      .catch(() => setInitializationComplete(true));
  }, []);

  return (
    <main className="app-shell">
      <Sidebar activePage={activePage} onNavigate={setActivePage} />
      {activePage === "dashboard" ? <DashboardPage onNavigate={setActivePage} /> : null}
      {activePage === "news" ? <NewsPage /> : null}
      {activePage === "review" ? <ReviewPage /> : null}
      {activePage === "holdings" ? <PortfolioPage /> : null}
      {activePage === "calendar" ? <EventCalendarPage /> : null}
      {activePage === "settings" ? <SettingsPage /> : null}
      {activePage !== "dashboard" && activePage !== "news" && activePage !== "review" && activePage !== "holdings" && activePage !== "calendar" && activePage !== "settings" ? <ComingSoonPage title={pageTitles[activePage]} /> : null}
      {initializationComplete === false ? <FirstRunWizard onCompleted={() => setInitializationComplete(true)} /> : null}
    </main>
  );
}

export default App;
