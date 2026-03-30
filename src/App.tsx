import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { BrowserRouter, Routes, Route, NavLink } from 'react-router-dom';
import {
  LayoutDashboard,
  BarChart3,
  Brain,
  Settings,
  PlusCircle,
  Flame,
} from 'lucide-react';
import { ThemeProvider } from './theme/ThemeProvider';
import { ActiveProfileProvider } from './hooks/useActiveProfile';
import ProfileSelector from './components/ProfileSelector';
import Dashboard from './pages/Dashboard';
import CampaignDetail from './pages/CampaignDetail';
import PostComposer from './pages/PostComposer';
import Analytics from './pages/Analytics';
import AIInsights from './pages/AIInsights';
import SettingsPage from './pages/Settings';
import './App.css';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      retry: 1,
    },
  },
});

function AppLayout() {
  return (
    <div className="app-layout">
      <aside className="sidebar">
        <div className="sidebar-brand">
          <h1>
            <Flame size={18} style={{ display: 'inline', verticalAlign: 'middle', marginRight: 6 }} />
            MEEM MARKETING
          </h1>
        </div>
        <ProfileSelector />
        <nav className="sidebar-nav">
          <NavLink to="/" end>
            <LayoutDashboard /> Dashboard
          </NavLink>
          <NavLink to="/campaigns/new">
            <PlusCircle /> New Campaign
          </NavLink>
          <NavLink to="/analytics">
            <BarChart3 /> Analytics
          </NavLink>
          <NavLink to="/ai">
            <Brain /> AI Insights
          </NavLink>
          <NavLink to="/settings">
            <Settings /> Settings
          </NavLink>
        </nav>
        <div className="sidebar-footer">MEEM v0.1.0</div>
      </aside>
      <main className="main-content">
        <Routes>
          <Route path="/" element={<Dashboard />} />
          <Route path="/campaigns/new" element={<PostComposer />} />
          <Route path="/campaigns/:id" element={<CampaignDetail />} />
          <Route path="/analytics" element={<Analytics />} />
          <Route path="/ai" element={<AIInsights />} />
          <Route path="/settings" element={<SettingsPage />} />
        </Routes>
      </main>
    </div>
  );
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <ThemeProvider>
        <BrowserRouter>
          <ActiveProfileProvider>
            <AppLayout />
          </ActiveProfileProvider>
        </BrowserRouter>
      </ThemeProvider>
    </QueryClientProvider>
  );
}
