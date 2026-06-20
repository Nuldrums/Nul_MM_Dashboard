import { useEffect, useState } from 'react';
import { QueryClient, QueryClientProvider, useQueryClient } from '@tanstack/react-query';
import { BrowserRouter, Routes, Route, NavLink } from 'react-router-dom';
import {
  LayoutDashboard,
  BarChart3,
  Brain,
  Settings,
  PlusCircle,
  Flame,
  Minus,
  Square,
  X,
} from 'lucide-react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { getVersion } from '@tauri-apps/api/app';
import { onOpenUrl, getCurrent as getCurrentDeepLinks } from '@tauri-apps/plugin-deep-link';
import { ThemeProvider } from './theme/ThemeProvider';
import { ActiveProfileProvider } from './hooks/useActiveProfile';
import ProfileSelector from './components/ProfileSelector';
import Dashboard from './pages/Dashboard';
import CampaignDetail from './pages/CampaignDetail';
import PostComposer from './pages/PostComposer';
import Analytics from './pages/Analytics';
import AIInsights from './pages/AIInsights';
import SettingsPage from './pages/Settings';
import { apiFetch } from './hooks/useApi';
import './App.css';

const appWindow = getCurrentWindow();

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      retry: 1,
    },
  },
});

function TitleBar() {
  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="titlebar-title" data-tauri-drag-region>MEEM Marketing</div>
      <div className="titlebar-controls">
        <button className="titlebar-btn" onClick={() => appWindow.minimize()}>
          <Minus size={14} />
        </button>
        <button className="titlebar-btn" onClick={() => appWindow.toggleMaximize()}>
          <Square size={11} />
        </button>
        <button className="titlebar-btn titlebar-btn-close" onClick={() => appWindow.close()}>
          <X size={14} />
        </button>
      </div>
    </div>
  );
}

// Kick off a metric fetch on app open. The backend handles concurrency (returns
// "already running" if another fetch is in flight) and only API-trackable posts
// are touched, so this is cheap when nothing's connected.
function useStartupMetricFetch() {
  const queryClient = useQueryClient();
  useEffect(() => {
    let cancelled = false;
    let pollTimer: ReturnType<typeof setTimeout> | null = null;

    const poll = async () => {
      if (cancelled) return;
      try {
        const status = await apiFetch<{ running: boolean }>('/metrics/fetch/status');
        if (cancelled) return;
        if (status.running) {
          pollTimer = setTimeout(poll, 3000);
        } else {
          queryClient.invalidateQueries({ queryKey: ['metrics'] });
          queryClient.invalidateQueries({ queryKey: ['campaigns'] });
          queryClient.invalidateQueries({ queryKey: ['analytics'] });
        }
      } catch {
        // backend not ready yet; back off
        pollTimer = setTimeout(poll, 3000);
      }
    };

    apiFetch('/metrics/fetch', { method: 'POST' })
      .then(() => { if (!cancelled) pollTimer = setTimeout(poll, 2000); })
      .catch(() => { /* backend unreachable on cold start — silently ignore */ });

    return () => {
      cancelled = true;
      if (pollTimer) clearTimeout(pollTimer);
    };
  }, [queryClient]);
}

// Handle inbound deep links (e.g. meem://oauth/tiktok/callback?code=...&state=...).
// The website relay redirects the browser to a meem:// URL; Tauri's deep-link plugin
// fires onOpenUrl with that URL, and we POST the params to the local backend to
// complete the OAuth exchange.
// Module-level dedupe set: prevents the same OAuth callback URL from being
// processed twice when getCurrent() + onOpenUrl() + React StrictMode all fire
// for the same incoming deep link. Lives outside the component so it survives
// effect cleanups and double-mounts.
const processedDeepLinks = new Set<string>();

function useOAuthDeepLinks() {
  const queryClient = useQueryClient();
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const handleUrl = async (url: string) => {
      if (processedDeepLinks.has(url)) return;
      processedDeepLinks.add(url);
      try {
        const parsed = new URL(url);
        if (parsed.protocol !== 'meem:') return;
        const path = parsed.hostname + parsed.pathname;
        // Tauri URL parser puts the first segment in hostname for meem://oauth/...
        const isTikTokCallback =
          path.includes('oauth/tiktok/callback') || path === 'oauth/tiktok/callback';
        if (!isTikTokCallback) return;

        const code = parsed.searchParams.get('code');
        const state = parsed.searchParams.get('state');
        const error = parsed.searchParams.get('error');

        if (error) {
          alert(`TikTok connection failed: ${error} ${parsed.searchParams.get('error_description') ?? ''}`);
          return;
        }
        if (!code || !state) {
          alert('TikTok callback missing code or state');
          return;
        }

        const resp = await apiFetch<{ display_name: string }>('/oauth/tiktok/exchange', {
          method: 'POST',
          body: JSON.stringify({ code, state }),
        });
        queryClient.invalidateQueries({ queryKey: ['accounts'] });
        alert(`Connected to TikTok as @${resp.display_name}`);
      } catch (e) {
        alert(`TikTok connection failed: ${(e as Error).message}`);
      }
    };

    // Handle cold-start case: app launched via deep link (Windows passes URL as CLI arg)
    getCurrentDeepLinks().then((urls) => {
      if (urls && urls.length > 0) {
        for (const u of urls) handleUrl(u);
      }
    }).catch(() => { /* not in Tauri context, ignore */ });

    // Handle runtime case: deep link fires while app is already running
    onOpenUrl((urls) => {
      for (const u of urls) handleUrl(u);
    }).then((un) => { unlisten = un; }).catch(() => { /* not in Tauri context */ });

    return () => { if (unlisten) unlisten(); };
  }, [queryClient]);
}

function AppLayout() {
  useStartupMetricFetch();
  useOAuthDeepLinks();
  const [appVersion, setAppVersion] = useState<string | null>(null);
  useEffect(() => {
    getVersion().then(setAppVersion).catch(() => { /* not in Tauri context */ });
  }, []);
  return (
    <div className="app-layout">
      <TitleBar />
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
        <div className="sidebar-footer">MEEM{appVersion ? ` v${appVersion}` : ''}</div>
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
