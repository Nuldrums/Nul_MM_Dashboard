import { createContext, useContext, useState, useEffect, type ReactNode } from 'react';
import { useProfiles } from './useProfiles';
import type { Profile } from '../lib/types';

const STORAGE_KEY = 'trikeri-active-profile';

interface ActiveProfileContextValue {
  activeProfileId: string | null;
  setActiveProfileId: (id: string | null) => void;
  activeProfile: Profile | null;
}

const ActiveProfileContext = createContext<ActiveProfileContextValue>({
  activeProfileId: null,
  setActiveProfileId: () => {},
  activeProfile: null,
});

export function ActiveProfileProvider({ children }: { children: ReactNode }) {
  const [activeProfileId, setActiveProfileIdState] = useState<string | null>(() => {
    try {
      return localStorage.getItem(STORAGE_KEY);
    } catch {
      return null;
    }
  });

  const { data: profiles } = useProfiles();

  const setActiveProfileId = (id: string | null) => {
    setActiveProfileIdState(id);
    try {
      if (id) {
        localStorage.setItem(STORAGE_KEY, id);
      } else {
        localStorage.removeItem(STORAGE_KEY);
      }
    } catch {
      // localStorage unavailable
    }
  };

  // Auto-select first profile if none selected but profiles exist
  useEffect(() => {
    if (!activeProfileId && profiles && profiles.length > 0) {
      setActiveProfileId(profiles[0].id);
    }
    // If stored ID no longer exists in profiles, clear it
    if (activeProfileId && profiles && profiles.length > 0) {
      const exists = profiles.some((p) => p.id === activeProfileId);
      if (!exists) {
        setActiveProfileId(profiles[0].id);
      }
    }
  }, [profiles, activeProfileId]);

  const activeProfile = profiles?.find((p) => p.id === activeProfileId) ?? null;

  return (
    <ActiveProfileContext.Provider value={{ activeProfileId, setActiveProfileId, activeProfile }}>
      {children}
    </ActiveProfileContext.Provider>
  );
}

export function useActiveProfile() {
  return useContext(ActiveProfileContext);
}
